//! Real channel + production command FIFO, not the shipping ssh_main_task.
//! The native stream never constructs a legacy Terminal or TerminalPump.

use std::future::{Future, poll_fn};
use std::net::SocketAddr;
use std::sync::Arc;
use std::task::Poll;
use std::time::Duration;

use oneterm_terminal::backend::AsyncGhosttyPump;
use oneterm_terminal::ghostty_bridge::State;
use oneterm_terminal::{
    ClipboardOrigin, OscRouter, PtyTransport, SessionEvent, SessionEventSink, SharedSessionState,
};
use russh::server::{Auth, Handler, Msg, Server, Session};
use russh::{Channel, ChannelId, ChannelMsg};

use crate::session::authenticate_with_password;
use crate::test_support::{connect_trusting_loopback, spawn_server};
use crate::transport::{Cmd, SSH_COMMAND_QUEUE_CAPACITY, SshTransport};

const OUTPUT: &[u8] = b"abc\x1b[6n\x1b]2;remote\x07";

#[derive(Debug, PartialEq, Eq)]
enum PeerEvent {
    Data(Vec<u8>),
    Resize(u32, u32),
    Closed,
}

#[derive(Clone)]
struct ProbeServer(async_channel::Sender<PeerEvent>);

impl Server for ProbeServer {
    type Handler = Self;

    fn new_client(&mut self, _peer_addr: Option<SocketAddr>) -> Self::Handler {
        self.clone()
    }
}

impl Handler for ProbeServer {
    type Error = russh::Error;

    async fn auth_password(&mut self, _user: &str, _password: &str) -> Result<Auth, Self::Error> {
        Ok(Auth::Accept)
    }

    async fn channel_open_session(
        &mut self,
        _channel: Channel<Msg>,
        reply: russh::server::ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        reply.accept().await;
        Ok(())
    }

    async fn shell_request(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        // Separate SSH packets split the CSI; transport packetization is not a
        // parser boundary. The fixture consumes however channel.wait groups it.
        session.data(channel, OUTPUT[..5].to_vec())?;
        session.data(channel, OUTPUT[5..].to_vec())?;
        Ok(())
    }

    async fn data(
        &mut self,
        _channel: ChannelId,
        data: &[u8],
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.0.try_send(PeerEvent::Data(data.to_vec())).unwrap();
        Ok(())
    }

    async fn window_change_request(
        &mut self,
        _channel: ChannelId,
        cols: u32,
        rows: u32,
        _width: u32,
        _height: u32,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.0.try_send(PeerEvent::Resize(cols, rows)).unwrap();
        Ok(())
    }

    async fn channel_close(
        &mut self,
        _channel: ChannelId,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.0.try_send(PeerEvent::Closed).unwrap();
        Ok(())
    }
}

// Abort the listening task on success, panic or deadline cancellation.
struct ServerGuard(tokio::task::JoinHandle<()>);
impl Drop for ServerGuard {
    fn drop(&mut self) {
        self.0.abort();
    }
}

async fn deadline<T>(future: impl Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(20), future)
        .await
        .expect("native SSH fixture exceeded its deadline")
}

async fn roundtrip(cancel: bool) {
    let (peer_tx, peer_rx) = async_channel::bounded(16);
    let (address, server_task) = spawn_server(ProbeServer(peer_tx)).await;
    let _server = ServerGuard(server_task);
    let (mut handle, _known_hosts) = connect_trusting_loopback(address).await;
    authenticate_with_password(&mut handle, "user", "secret")
        .await
        .unwrap();
    let mut channel = handle.channel_open_session().await.unwrap();
    let (cmd_tx, cmd_rx) = async_channel::bounded(SSH_COMMAND_QUEUE_CAPACITY);
    let transport = SshTransport::new(cmd_tx);
    let (event_tx, event_rx) = async_channel::bounded(1);
    let router = OscRouter::new(
        transport.clone(),
        SessionEventSink::new(event_tx.clone()),
        SharedSessionState::new_alive(),
        ClipboardOrigin::Remote,
    );
    let mut pump = AsyncGhosttyPump::spawn(router, 16, 3).unwrap();
    let model = pump.model();
    let good = pump.finish_batch().await.unwrap();
    assert_eq!(event_rx.recv().await.unwrap(), SessionEvent::Output);
    event_tx.try_send(SessionEvent::Bell).unwrap();

    transport.pty_write(b"typed").unwrap();
    channel.request_shell(false).await.unwrap();
    let mut received = Vec::new();
    while received.len() < OUTPUT.len() {
        match channel.wait().await {
            Some(ChannelMsg::Data { data }) => {
                received.extend_from_slice(&data);
                pump.process_chunk(&data).await.unwrap();
            }
            other => panic!("expected remote output, got {other:?}"),
        }
    }
    assert_eq!(received, OUTPUT);
    assert!(Arc::ptr_eq(
        model.reader().poll().unwrap().frame.as_ref().unwrap(),
        &good
    ));

    // Both prior input and native reply use the originating transport's actual
    // FIFO. Queue admission alone does not satisfy the peer assertion below.
    for expected in [b"typed".as_slice(), b"\x1b[1;4R".as_slice()] {
        let Cmd::Write(bytes) = cmd_rx.recv().await.unwrap() else {
            panic!("expected FIFO write")
        };
        assert_eq!(bytes, expected);
        let result = channel.data(&bytes[..]).await;
        transport.release_write_bytes(bytes.len());
        result.unwrap();
    }
    let mut peer_bytes = Vec::new();
    while peer_bytes.len() < b"typed\x1b[1;4R".len() {
        let PeerEvent::Data(bytes) = peer_rx.recv().await.unwrap() else {
            panic!("expected peer receipt")
        };
        peer_bytes.extend(bytes);
    }
    assert_eq!(peer_bytes, b"typed\x1b[1;4R");
    assert_eq!(transport.diagnostics().queued_write_bytes, 0);

    // A real peer has received the reply, but reliable UI delivery is a distinct
    // barrier: no new capture can commit while the one-slot queue is full.
    let mut finish = Box::pin(pump.finish_batch());
    poll_fn(|cx| {
        assert!(finish.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
    assert!(Arc::ptr_eq(
        model.reader().poll().unwrap().frame.as_ref().unwrap(),
        &good
    ));

    if cancel {
        drop(finish);
        assert!(transport.is_closing());
        assert!(matches!(
            model.reader().poll().unwrap().state,
            State::Failed(_)
        ));
        assert!(pump.shutdown().await.is_err());
        assert!(Arc::ptr_eq(
            model.reader().poll().unwrap().frame.as_ref().unwrap(),
            &good
        ));
        assert_eq!(event_rx.recv().await.unwrap(), SessionEvent::Bell);
        assert!(event_rx.is_empty());
    } else {
        assert_eq!(event_rx.recv().await.unwrap(), SessionEvent::Bell);
        let frame = finish.await.unwrap();
        let text: String = frame.source().rows[0]
            .cells
            .iter()
            .map(|cell| cell.text.as_str())
            .collect();
        assert!(text.starts_with("abc"), "remote text missing: {text:?}");
        assert_eq!(
            event_rx.recv().await.unwrap(),
            SessionEvent::Title("remote".into())
        );
        pump.resize(20, 4, 10, 20).await.unwrap();
        pump.resize(24, 5, 10, 20).await.unwrap();
        assert!(matches!(cmd_rx.recv().await.unwrap(), Cmd::Resize));
        let (rows, cols) = transport.take_pending_resize().unwrap();
        channel
            .window_change(cols.into(), rows.into(), 0, 0)
            .await
            .unwrap();
        assert_eq!(peer_rx.recv().await.unwrap(), PeerEvent::Resize(24, 5));
        let frame = pump.finish_batch().await.unwrap();
        assert_eq!((frame.source().cols, frame.source().rows.len()), (24, 5));
        pump.shutdown().await.unwrap();
        assert!(matches!(
            model.reader().poll().unwrap().state,
            State::Closed
        ));
    }
    assert!(transport.is_closing());
    // The transport only signals shutdown. This fixture explicitly performs
    // physical channel close; it does not claim shipping task cancellation.
    channel.close().await.unwrap();
    assert_eq!(peer_rx.recv().await.unwrap(), PeerEvent::Closed);
    assert!(cmd_rx.try_recv().is_ok_and(|cmd| matches!(cmd, Cmd::Close)));
    assert!(cmd_rx.is_empty());
    drop(channel);
    drop(handle);
}

#[tokio::test]
async fn native_ssh_reply_fifo_resize_and_reliable_publication() {
    deadline(roundtrip(false)).await;
}

#[tokio::test]
async fn native_ssh_cancelled_publication_retains_frame_and_closes_channel() {
    deadline(roundtrip(true)).await;
}
