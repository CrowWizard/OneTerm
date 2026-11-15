//! Autonomous native owner proof; no fixture-driven channel reads/writes.

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::task::{Context, Waker};
use std::time::Duration;

use oneterm_terminal::ghostty_bridge::State;
use russh::server::{Auth, Handler, Msg, Server, Session};
use russh::{Channel, ChannelId};

use super::*;
use crate::session::authenticate_with_password;
use crate::test_support::{connect_trusting_loopback, spawn_server};

#[derive(Debug, PartialEq, Eq)]
enum Peer {
    Bytes(Vec<u8>),
    Resize(u32, u32),
    Closed,
}

#[derive(Clone)]
struct Probe(async_channel::Sender<Peer>);

impl Server for Probe {
    type Handler = Self;
    fn new_client(&mut self, _address: Option<SocketAddr>) -> Self {
        self.clone()
    }
}

impl Handler for Probe {
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
        session.data(channel, b"ready".to_vec())?;
        Ok(())
    }
    async fn data(
        &mut self,
        channel: ChannelId,
        bytes: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if bytes == b"go" {
            session.data(channel, b"\x1b[6n\x1b]2;blocked\x07".to_vec())?;
        } else if bytes == b"color-query" {
            session.data(channel, b"\x1b]10;?\x07\x1b]4;1;?\x07".to_vec())?;
        } else if bytes == b"modes" {
            session.data(
                channel,
                b"\x1b[?1h\x1b[?1000h\x1b[?1006h\x1b]2;input\x07".to_vec(),
            )?;
        } else if bytes == b"mouse-off" {
            session.data(
                channel,
                b"\x1b[?1000l\x1b[?1006l\x1b]2;mouse-off\x07".to_vec(),
            )?;
        } else if bytes == b"wheel-alt" {
            session.data(
                channel,
                b"\x1b[?1000l\x1b[?1049;1007;1h\x1b]2;wheel-alt\x07".to_vec(),
            )?;
        } else if bytes == b"wheel-alt-off" {
            session.data(channel, b"\x1b[?1007l\x1b]2;wheel-alt-off\x07".to_vec())?;
        } else if bytes == b"fault" {
            session.data(channel, b"\x1b[6n".repeat(1000))?;
        } else if bytes == b"paste-on" {
            session.data(channel, b"\x1b[?2004h\x1b]2;paste-on\x07".to_vec())?;
        } else if bytes == b"paste-off" {
            session.data(channel, b"\x1b[?2004l\x1b]2;paste-off\x07".to_vec())?;
        } else if bytes == b"history" {
            session.data(
                channel,
                b"\x1b[2J\x1b[Hzero\r\none\r\ntwo\r\nthree\r\nfour".to_vec(),
            )?;
        } else if bytes == b"exit" {
            session.data(channel, b" goodbye".to_vec())?;
            session.exit_status_request(channel, 7)?;
            session.eof(channel)?;
            session.close(channel)?;
        } else if bytes == b"late-exit" {
            session.exit_status_request(channel, 9)?;
            session.data(channel, b" after\x1b]2;late\x07".to_vec())?;
            session.eof(channel)?;
            session.close(channel)?;
        } else if bytes == b"status" {
            session.exit_status_request(channel, 7)?;
        } else if bytes == b"eof" {
            session.eof(channel)?;
        } else if bytes == b"signal" {
            session.exit_status_request(channel, 7)?;
            session.exit_signal_request(channel, russh::Sig::TERM, false, "", "")?;
            session.eof(channel)?;
            session.close(channel)?;
        }
        self.0.try_send(Peer::Bytes(bytes.to_vec())).unwrap();
        Ok(())
    }
    async fn channel_close(
        &mut self,
        _channel: ChannelId,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.0.try_send(Peer::Closed).unwrap();
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
        self.0.try_send(Peer::Resize(cols, rows)).unwrap();
        Ok(())
    }
}

struct ServerGuard(tokio::task::JoinHandle<()>);

#[tokio::test]
async fn orderly_shutdown_drains_admitted_raw_bytes_and_delivers_closed() {
    bounded(async {
        let f = setup().await;
        f.endpoint.write(b"before-seal").unwrap();
        f.endpoint.request_shutdown().unwrap();
        assert!(matches!(
            f.endpoint.write(b"after-seal"),
            Err(TerminalError::Closed)
        ));
        let consumer = tokio::spawn(async move {
            let mut seen = vec![];
            while let Ok(event) = f.events.recv().await {
                seen.push(event);
            }
            seen
        });
        drop(f.event_tx);
        let report = f.endpoint.wait().await.unwrap();
        assert_eq!(report.stop, Stop::Requested);
        assert_eq!(report.lifecycle, LifecycleDelivery::Complete);
        assert_eq!(
            f.peer.recv().await.unwrap(),
            Peer::Bytes(b"before-seal".to_vec())
        );
        assert_eq!(f.peer.recv().await.unwrap(), Peer::Closed);
        assert_eq!(consumer.await.unwrap().last(), Some(&SessionEvent::Closed));
    })
    .await;
}

#[tokio::test]
async fn hard_close_interrupts_orderly_shutdown_while_reliable_delivery_is_blocked() {
    bounded(async {
        let f = setup().await;
        blocked(&f).await;
        f.endpoint.request_shutdown().unwrap();
        f.endpoint.close().unwrap();
        let report = f.endpoint.wait().await.unwrap();
        assert_eq!(report.stop, Stop::Requested);
        assert_eq!(report.lifecycle, LifecycleDelivery::Cancelled);
        assert_eq!(f.peer.recv().await.unwrap(), Peer::Closed);
    })
    .await;
}
impl Drop for ServerGuard {
    fn drop(&mut self) {
        self.0.abort();
    }
}

async fn bounded<T>(future: impl Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(20), future)
        .await
        .expect("native owner deadline")
}

struct Fixture {
    endpoint: NativeChannel,
    events: async_channel::Receiver<SessionEvent>,
    event_tx: async_channel::Sender<SessionEvent>,
    peer: async_channel::Receiver<Peer>,
    _server: ServerGuard,
    _trust: crate::test_support::TempKnownHosts,
}

async fn setup() -> Fixture {
    let (tx, peer) = async_channel::bounded(32);
    let (address, server) = spawn_server(Probe(tx)).await;
    let server = ServerGuard(server);
    let (mut handle, trust) = connect_trusting_loopback(address).await;
    authenticate_with_password(&mut handle, "user", "secret")
        .await
        .unwrap();
    let channel = handle.channel_open_session().await.unwrap();
    channel.request_shell(false).await.unwrap();
    let (event_tx, events) = async_channel::bounded(1);
    let endpoint = NativeChannel::spawn(handle, channel, 16, 3, event_tx.clone()).unwrap();
    assert_eq!(events.recv().await.unwrap(), SessionEvent::Output);
    let model = endpoint.model();
    let frame = model.reader().poll().unwrap().frame.unwrap();
    let text: String = frame.source().rows[0]
        .cells
        .iter()
        .map(|cell| cell.text.as_str())
        .collect();
    assert!(text.starts_with("ready"), "{text:?}");
    Fixture {
        endpoint,
        events,
        event_tx,
        peer,
        _server: server,
        _trust: trust,
    }
}

#[tokio::test]
async fn native_raw_visible_origin_bytes_not_paste_and_queue_cancellation() {
    bounded(async {
        let f = setup().await;
        f.endpoint.write(b"paste-on").unwrap();
        assert_eq!(
            f.peer.recv().await.unwrap(),
            Peer::Bytes(b"paste-on".to_vec())
        );
        while f.events.recv().await.unwrap() != SessionEvent::Title("paste-on".into()) {}
        let surface: &dyn oneterm_terminal::ghostty_session::SurfaceSession = &f.endpoint;
        let raw = b"\x1bOA\0\xff\r\n\x03";
        surface.write_visible(raw).await.unwrap();
        assert_eq!(f.peer.recv().await.unwrap(), Peer::Bytes(raw.to_vec()));
        surface.paste_visible("paste").await.unwrap();
        assert_eq!(
            f.peer.recv().await.unwrap(),
            Peer::Bytes(b"\x1b[200~paste\x1b[201~".to_vec())
        );
        blocked(&f).await;
        let mut inputs: Vec<_> = (0..64)
            .map(|_| Box::pin(f.endpoint.write_visible(b"queued")))
            .collect();
        for input in &mut inputs {
            assert!(
                input
                    .as_mut()
                    .poll(&mut Context::from_waker(Waker::noop()))
                    .is_pending()
            );
        }
        assert!(matches!(
            f.endpoint.write_visible(b"full").await,
            Err(AsyncGhosttyPumpError::Transport(TerminalError::QueueFull))
        ));
        assert!(!f.endpoint.close.is_cancelled());
        drop(inputs.remove(0));
        assert!(f.endpoint.close.is_cancelled());
        while !f.endpoint.inputs.is_empty() {
            tokio::task::yield_now().await;
        }
        for input in inputs {
            assert!(matches!(input.await, Err(AsyncGhosttyPumpError::Closed)));
        }
        assert_eq!(
            f.endpoint.wait().await.unwrap().lifecycle,
            LifecycleDelivery::Cancelled
        );
        assert_eq!(f.peer.recv().await.unwrap(), Peer::Closed);
    })
    .await;
}

#[tokio::test]
async fn native_key_receipt_origin_bytes_modes_and_no_duplicate_delivery() {
    use oneterm_terminal::ghostty::input::{KeyAction, KeyCode, MAX_KEY_TEXT_BYTES};
    bounded(async {
        let f = setup().await;
        let other = setup().await;
        let surface: &dyn oneterm_terminal::ghostty_session::SurfaceSession = &f.endpoint;
        let mut invalid = KeyEvent::new(KeyCode::UNIDENTIFIED);
        invalid.text = "x".repeat(MAX_KEY_TEXT_BYTES + 1);
        assert!(surface.key_receipt(invalid).await.is_err());
        assert!(!f.endpoint.close.is_cancelled());
        assert_eq!(
            surface
                .key_receipt(KeyEvent::new(KeyCode::ARROW_UP))
                .await
                .unwrap(),
            b"\x1b[A"
        );
        assert_eq!(
            f.peer.recv().await.unwrap(),
            Peer::Bytes(b"\x1b[A".to_vec())
        );
        f.endpoint.write(b"modes").unwrap();
        assert_eq!(f.peer.recv().await.unwrap(), Peer::Bytes(b"modes".to_vec()));
        while f.events.recv().await.unwrap() != SessionEvent::Title("input".into()) {}
        assert_eq!(
            surface
                .key_receipt(KeyEvent::new(KeyCode::ARROW_UP))
                .await
                .unwrap(),
            b"\x1bOA"
        );
        assert_eq!(
            f.peer.recv().await.unwrap(),
            Peer::Bytes(b"\x1bOA".to_vec())
        );
        let mut release = KeyEvent::new(KeyCode::ARROW_UP);
        release.action = KeyAction::Release;
        assert!(surface.key_receipt(release).await.unwrap().is_empty());
        // A subsequent marker proves there was no hidden duplicate key write.
        f.endpoint.write(b"receipt-marker").unwrap();
        assert_eq!(
            f.peer.recv().await.unwrap(),
            Peer::Bytes(b"receipt-marker".to_vec())
        );
        assert!(other.peer.is_empty());
        f.endpoint.close().unwrap();
        assert!(matches!(
            surface.key_receipt(KeyEvent::new(KeyCode::ENTER)).await,
            Err(AsyncGhosttyPumpError::Transport(TerminalError::Closed))
        ));
        f.endpoint.wait().await.unwrap();
        other.endpoint.close().unwrap();
        other.endpoint.wait().await.unwrap();
    })
    .await;
}

#[tokio::test]
async fn native_key_receipt_queue_rejection_and_admitted_cancellation() {
    use oneterm_terminal::ghostty::input::KeyCode;
    bounded(async {
        let f = setup().await;
        drop(f.endpoint.key_receipt(KeyEvent::new(KeyCode::ENTER)));
        assert!(f.endpoint.inputs.is_empty());
        blocked(&f).await;
        let mut receipts: Vec<_> = (0..64)
            .map(|_| Box::pin(f.endpoint.key_receipt(KeyEvent::new(KeyCode::ENTER))))
            .collect();
        for receipt in &mut receipts {
            assert!(
                receipt
                    .as_mut()
                    .poll(&mut Context::from_waker(Waker::noop()))
                    .is_pending()
            );
        }
        assert!(matches!(
            f.endpoint.key_receipt(KeyEvent::new(KeyCode::ENTER)).await,
            Err(AsyncGhosttyPumpError::Transport(TerminalError::QueueFull))
        ));
        assert!(!f.endpoint.close.is_cancelled());
        drop(receipts.remove(0));
        assert!(f.endpoint.close.is_cancelled());
        while !f.endpoint.inputs.is_empty() {
            tokio::task::yield_now().await;
        }
        for receipt in receipts {
            assert!(matches!(receipt.await, Err(AsyncGhosttyPumpError::Closed)));
        }
        assert_eq!(
            f.endpoint.wait().await.unwrap().lifecycle,
            LifecycleDelivery::Cancelled
        );
        assert_eq!(f.peer.recv().await.unwrap(), Peer::Closed);
    })
    .await;
}

#[tokio::test]
async fn native_clear_command_exact_bytes_and_origin_isolation() {
    bounded(async {
        let f = setup().await;
        let other = setup().await;
        f.endpoint.write(b"paste-on").unwrap();
        assert_eq!(
            f.peer.recv().await.unwrap(),
            Peer::Bytes(b"paste-on".to_vec())
        );
        loop {
            if f.events.recv().await.unwrap() == SessionEvent::Title("paste-on".into()) {
                break;
            }
        }
        let consumer = tokio::spawn({
            let events = f.events.clone();
            async move { while events.recv().await.is_ok() {} }
        });
        let surface: &dyn oneterm_terminal::ghostty_session::SurfaceSession = &f.endpoint;
        surface.selection(SelectionUpdate::All).await.unwrap();
        assert_eq!(
            f.endpoint
                .model()
                .selection_text()
                .await
                .unwrap()
                .as_deref(),
            Some("ready")
        );
        surface.clear_command().await.unwrap();
        assert_eq!(
            f.peer.recv().await.unwrap(),
            Peer::Bytes(b"clear\r".to_vec())
        );
        assert_eq!(f.endpoint.model().selection_text().await.unwrap(), None);
        assert!(other.peer.is_empty());
        surface.selection(SelectionUpdate::All).await.unwrap();
        assert_eq!(
            f.endpoint
                .model()
                .selection_text()
                .await
                .unwrap()
                .as_deref(),
            Some("ready")
        );
        f.endpoint.close().unwrap();
        assert!(matches!(
            surface.clear_command().await,
            Err(AsyncGhosttyPumpError::Transport(TerminalError::Closed))
        ));
        f.endpoint.wait().await.unwrap();
        other.endpoint.close().unwrap();
        other.endpoint.wait().await.unwrap();
        consumer.abort();
    })
    .await;
}

#[tokio::test]
async fn native_owner_clipboard_reply_reaches_only_originating_peer() {
    bounded(async {
        let f = setup().await;
        let other = setup().await;
        f.endpoint.reply_clipboard("hi").unwrap();
        assert_eq!(
            f.peer.recv().await.unwrap(),
            Peer::Bytes(b"\x1b]52;c;aGk=\x07".to_vec())
        );
        assert!(other.peer.is_empty());
        assert!(
            f.endpoint
                .reply_clipboard(&"x".repeat(256 * 1024 + 1))
                .is_err()
        );
        assert!(f.peer.is_empty());
        f.endpoint.close().unwrap();
        assert_eq!(
            f.endpoint.reply_clipboard("late"),
            Err(TerminalError::Closed)
        );
        f.endpoint.wait().await.unwrap();
        other.endpoint.close().unwrap();
        other.endpoint.wait().await.unwrap();
    })
    .await;
}

async fn blocked(f: &Fixture) {
    blocked_with_reply(f, b"\x1b[1;6R").await;
}

async fn blocked_with_reply(f: &Fixture, reply: &[u8]) {
    f.event_tx.try_send(SessionEvent::Bell).unwrap();
    f.endpoint.write(b"go").unwrap();
    assert_eq!(f.peer.recv().await.unwrap(), Peer::Bytes(b"go".to_vec()));
    // The owner must drain native replies even when finish_batch awaits UI.
    assert_eq!(f.peer.recv().await.unwrap(), Peer::Bytes(reply.to_vec()));
    while f.endpoint.state().title().as_deref() != Some("blocked") {
        tokio::task::yield_now().await;
    }
}

#[tokio::test]
async fn native_owner_ime_preedit_exact_commit_and_closed_admission() {
    use oneterm_terminal::ghostty_ime::{Error as ImeError, MAX_TEXT_BYTES};
    bounded(async {
        let f = setup().await;
        f.endpoint.paste("").await.unwrap();
        let model = f.endpoint.model();
        let before = model.reader().poll().unwrap().frame.unwrap();
        f.endpoint.set_marked_text("e\u{301}日本語".into()).unwrap();
        let saved = f.endpoint.marked_text().unwrap();
        f.endpoint.clear_marked_text();
        assert_eq!(f.endpoint.marked_text(), None);
        f.endpoint.set_marked_text("pending".into()).unwrap();
        assert_eq!(
            f.endpoint.commit_text(&"a".repeat(MAX_TEXT_BYTES + 1)),
            Err(ImeError::TextLimit)
        );
        assert_eq!(f.endpoint.marked_text().as_deref(), Some("pending"));
        f.endpoint.paste("").await.unwrap();
        assert!(f.peer.is_empty());
        assert!(Arc::ptr_eq(
            &before,
            model.reader().poll().unwrap().frame.as_ref().unwrap()
        ));
        let input = "e\u{301}日本語\n\r\x1b\x03";
        f.endpoint.commit_text(input).unwrap();
        assert_eq!(f.endpoint.marked_text(), None);
        assert_eq!(
            f.peer.recv().await.unwrap(),
            Peer::Bytes(input.as_bytes().to_vec())
        );
        assert_eq!(saved, "e\u{301}日本語");
        f.endpoint.close().unwrap();
        f.endpoint.set_marked_text("closed".into()).unwrap();
        assert_eq!(
            f.endpoint.commit_text("not-sent"),
            Err(ImeError::Input(TerminalError::Closed))
        );
        assert_eq!(f.endpoint.marked_text(), None);
        f.endpoint.wait().await.unwrap();
    })
    .await;
}

#[tokio::test]
async fn native_owner_surface_trait_encodes_resizes_and_reports_admission() {
    use oneterm_terminal::ghostty::input::{KeyCode, KeyEvent};
    use oneterm_terminal::ghostty_session::{InputError as SurfaceError, Size, SurfaceSession};
    bounded(async {
        let f = setup().await;
        let surface: &dyn SurfaceSession = &f.endpoint;
        assert_eq!(surface.query().viewport.unwrap().columns(), 16);
        surface
            .resize(Size {
                cols: 40,
                rows: 6,
                cell_width_px: 10,
                cell_height_px: 20,
            })
            .unwrap();
        f.endpoint.paste("").await.unwrap();
        assert_eq!(f.peer.recv().await.unwrap(), Peer::Resize(40, 6));
        surface.key(KeyEvent::new(KeyCode::ENTER)).unwrap();
        assert_eq!(f.peer.recv().await.unwrap(), Peer::Bytes(b"\r".to_vec()));
        surface.set_marked_text("pending".into()).unwrap();
        surface.commit_text("e\u{301}日本語").unwrap();
        assert_eq!(surface.marked_text(), None);
        assert_eq!(
            f.peer.recv().await.unwrap(),
            Peer::Bytes("e\u{301}日本語".as_bytes().to_vec())
        );
        let mut invalid = KeyEvent::new(KeyCode::UNIDENTIFIED);
        invalid.text = "\x03".into();
        assert!(matches!(
            surface.key(invalid),
            Err(SurfaceError::Invalid(_))
        ));
        f.endpoint.close().unwrap();
        assert!(matches!(
            surface.key(KeyEvent::new(KeyCode::ENTER)),
            Err(SurfaceError::Admission(TerminalError::Closed))
        ));
        f.endpoint.wait().await.unwrap();
    })
    .await;
}

#[tokio::test]
async fn native_owner_surface_mouse_uses_live_modes_and_typed_errors() {
    use oneterm_terminal::ghostty::input::{
        Geometry, Modifiers, MouseAction, MouseButton, MouseEvent,
    };
    use oneterm_terminal::ghostty_session::{InputError as SurfaceError, SurfaceSession};
    bounded(async {
        let f = setup().await;
        let surface: &dyn SurfaceSession = &f.endpoint;
        let mut mouse = MouseEvent {
            action: MouseAction::Press,
            button: Some(MouseButton::Left),
            modifiers: Modifiers::default(),
            x: 12.5,
            y: 25.0,
            any_button_pressed: true,
            geometry: Geometry {
                width: 160,
                height: 60,
                cell_width: 10,
                cell_height: 20,
                padding_top: 0,
                padding_bottom: 0,
                padding_left: 0,
                padding_right: 0,
            },
        };
        surface.mouse(mouse).unwrap();
        f.endpoint.paste("").await.unwrap();
        assert!(f.peer.is_empty(), "disabled reporting encodes nothing");
        f.endpoint.write(b"modes").unwrap();
        assert_eq!(f.peer.recv().await.unwrap(), Peer::Bytes(b"modes".to_vec()));
        while f.endpoint.state().title().as_deref() != Some("input") {
            tokio::task::yield_now().await;
        }
        while let Ok(event) = f.events.try_recv() {
            assert!(matches!(
                event,
                SessionEvent::Output | SessionEvent::Title(_)
            ));
        }
        f.endpoint.state().release_repaint_hint();
        surface.mouse(mouse).unwrap();
        assert_eq!(
            f.peer.recv().await.unwrap(),
            Peer::Bytes(b"\x1b[<0;2;2M".to_vec())
        );
        mouse.action = MouseAction::Release;
        mouse.any_button_pressed = false;
        surface.mouse(mouse).unwrap();
        assert_eq!(
            f.peer.recv().await.unwrap(),
            Peer::Bytes(b"\x1b[<0;2;2m".to_vec())
        );
        f.endpoint.write(b"mouse-off").unwrap();
        assert_eq!(
            f.peer.recv().await.unwrap(),
            Peer::Bytes(b"mouse-off".to_vec())
        );
        while f.endpoint.state().title().as_deref() != Some("mouse-off") {
            tokio::task::yield_now().await;
        }
        while f.events.try_recv().is_ok() {}
        surface.mouse(mouse).unwrap();
        f.endpoint.paste("").await.unwrap();
        assert!(f.peer.is_empty(), "latest native mode disables reporting");
        mouse.x = f32::NAN;
        assert!(matches!(
            surface.mouse(mouse),
            Err(SurfaceError::Invalid(_))
        ));
        mouse.x = 12.5;
        f.endpoint.close().unwrap();
        assert!(matches!(
            surface.mouse(mouse),
            Err(SurfaceError::Admission(TerminalError::Closed))
        ));
        f.endpoint.wait().await.unwrap();
    })
    .await;
}

#[tokio::test]
async fn native_owner_default_colors_ack_publication_and_real_peer_queries() {
    use oneterm_terminal::ghostty::render::Rgb;
    use oneterm_terminal::ghostty_content::{Content, RenderSource};
    bounded(async {
        let f = setup().await;
        let model = f.endpoint.model();
        f.endpoint.paste("").await.unwrap();
        f.endpoint.state().release_repaint_hint();
        let colors = DefaultColors {
            foreground: Rgb {
                r: 17,
                g: 34,
                b: 51,
            },
            background: Rgb { r: 4, g: 5, b: 6 },
            cursor: Some(Rgb { r: 7, g: 8, b: 9 }),
            palette: [Rgb {
                r: 10,
                g: 11,
                b: 12,
            }; 256],
        };
        drop(f.endpoint.set_default_colors(colors.clone()));
        assert!(f.endpoint.inputs.is_empty());
        let surface: &dyn oneterm_terminal::ghostty_session::SurfaceSession = &f.endpoint;
        surface.set_default_colors(colors.clone()).await.unwrap();
        let source: &dyn RenderSource = &f.endpoint;
        let mut content = Content::new(source);
        let mut independent = Content::new(source);
        content.poll().unwrap();
        independent.poll().unwrap();
        assert_eq!(
            content.damage(),
            &oneterm_terminal::ghostty::owner::Damage::Full
        );
        assert_eq!(independent.damage(), content.damage());
        let viewport = content.viewport().unwrap().clone();
        assert_eq!(viewport.foreground(), colors.foreground);
        assert_eq!(viewport.background(), colors.background);
        assert_eq!(viewport.cursor_color(), colors.cursor);
        assert_eq!(viewport.palette(), &colors.palette);
        assert_eq!(viewport.rows(0, usize::MAX).len(), viewport.lines());
        content.poll().unwrap();
        assert_eq!(
            content.damage(),
            &oneterm_terminal::ghostty::owner::Damage::None
        );
        let frame = model.reader().poll().unwrap().frame.unwrap();
        assert_eq!(frame.source().foreground, colors.foreground);
        assert_eq!(frame.source().background, colors.background);
        assert_eq!(frame.source().cursor_color, colors.cursor);
        assert_eq!(frame.source().palette, colors.palette);
        assert_eq!(f.events.try_recv().unwrap(), SessionEvent::Output);
        assert!(
            f.peer.is_empty(),
            "default setters do not synthesize parser/peer bytes"
        );
        f.endpoint.write(b"color-query").unwrap();
        assert_eq!(
            f.peer.recv().await.unwrap(),
            Peer::Bytes(b"color-query".to_vec())
        );
        let mut bytes = Vec::new();
        let expected = b"\x1b]10;rgb:1111/2222/3333\x07\x1b]4;1;rgb:0a0a/0b0b/0c0c\x07";
        while bytes.len() < expected.len() {
            let Peer::Bytes(chunk) = f.peer.recv().await.unwrap() else {
                panic!("unexpected close/resize during color reply")
            };
            bytes.extend(chunk);
        }
        assert_eq!(bytes, expected);
        f.endpoint.close().unwrap();
        assert!(matches!(
            f.endpoint.set_default_colors(colors).await,
            Err(AsyncGhosttyPumpError::Transport(TerminalError::Closed))
        ));
        f.endpoint.wait().await.unwrap();
    })
    .await;
}

#[tokio::test]
async fn native_owner_surface_wheel_live_routing_and_history_publication() {
    use oneterm_terminal::ghostty::{
        input::{Geometry, Modifiers, MouseAction, MouseEvent},
        wheel::WheelEvent,
    };
    use oneterm_terminal::ghostty_session::{InputError as SurfaceError, SurfaceSession};
    bounded(async {
        let f = setup().await;
        let surface: &dyn SurfaceSession = &f.endpoint;
        let mut wheel = WheelEvent {
            steps: 2,
            pointer: MouseEvent {
                action: MouseAction::Motion,
                button: None,
                modifiers: Modifiers::default(),
                x: 12.5,
                y: 25.0,
                any_button_pressed: false,
                geometry: Geometry {
                    width: 160,
                    height: 60,
                    cell_width: 10,
                    cell_height: 20,
                    padding_top: 0,
                    padding_bottom: 0,
                    padding_left: 0,
                    padding_right: 0,
                },
            },
        };
        surface.wheel(wheel).unwrap();
        f.endpoint.paste("").await.unwrap();
        assert!(f.peer.is_empty());
        assert!(matches!(
            surface.wheel(WheelEvent { steps: 0, ..wheel }),
            Err(SurfaceError::Invalid(_))
        ));
        for (command, title, expected) in [
            ("modes", "input", b"\x1b[<64;2;2M\x1b[<64;2;2M".as_slice()),
            ("wheel-alt", "wheel-alt", b"\x1bOA\x1bOA".as_slice()),
            ("wheel-alt-off", "wheel-alt-off", b"".as_slice()),
        ] {
            f.endpoint.write(command.as_bytes()).unwrap();
            assert_eq!(
                f.peer.recv().await.unwrap(),
                Peer::Bytes(command.as_bytes().to_vec())
            );
            while f.endpoint.state().title().as_deref() != Some(title) {
                tokio::task::yield_now().await;
            }
            while f.events.try_recv().is_ok() {}
            f.endpoint.state().release_repaint_hint();
            surface.wheel(wheel).unwrap();
            f.endpoint.paste("").await.unwrap();
            if expected.is_empty() {
                assert!(f.peer.is_empty());
            } else {
                assert_eq!(f.peer.recv().await.unwrap(), Peer::Bytes(expected.to_vec()));
            }
        }
        f.endpoint.close().unwrap();
        assert!(matches!(
            surface.wheel(wheel),
            Err(SurfaceError::Admission(TerminalError::Closed))
        ));
        f.endpoint.wait().await.unwrap();

        let f = setup().await;
        let surface: &dyn SurfaceSession = &f.endpoint;
        f.endpoint.write(b"history").unwrap();
        assert_eq!(
            f.peer.recv().await.unwrap(),
            Peer::Bytes(b"history".to_vec())
        );
        while f
            .endpoint
            .model()
            .reader()
            .poll()
            .unwrap()
            .frame
            .unwrap()
            .source()
            .metadata
            .scrollbar
            .total
            <= 3
        {
            tokio::task::yield_now().await;
        }
        while f.events.try_recv().is_ok() {}
        f.endpoint.state().release_repaint_hint();
        wheel.steps = 10;
        surface.wheel(wheel).unwrap();
        f.endpoint.paste("").await.unwrap();
        let frame = f.endpoint.model().reader().poll().unwrap().frame.unwrap();
        assert_eq!(frame.source().metadata.scrollbar.offset, 0);
        assert_eq!(frame.source().rows[0].cells[0].text, "z");
        assert_eq!(f.events.try_recv().unwrap(), SessionEvent::Output);
        assert!(f.peer.is_empty(), "history sends no peer bytes");
        f.endpoint.close().unwrap();
        f.endpoint.wait().await.unwrap();
    })
    .await;
}

#[tokio::test]
async fn native_owner_default_colors_full_queue_cancel_and_pending_ack_cleanup() {
    use oneterm_terminal::ghostty::render::Rgb;
    bounded(async {
        let f = setup().await;
        let model = f.endpoint.model();
        let good = model.reader().poll().unwrap().frame.unwrap();
        blocked(&f).await;
        let colors = DefaultColors {
            foreground: Rgb::default(),
            background: Rgb::default(),
            cursor: None,
            palette: [Rgb::default(); 256],
        };
        let mut updates: Vec<_> = (0..64)
            .map(|_| Box::pin(f.endpoint.set_default_colors(colors.clone())))
            .collect();
        for update in &mut updates {
            assert!(
                update
                    .as_mut()
                    .poll(&mut Context::from_waker(Waker::noop()))
                    .is_pending()
            );
        }
        assert!(matches!(
            f.endpoint.set_default_colors(colors).await,
            Err(AsyncGhosttyPumpError::Transport(TerminalError::QueueFull))
        ));
        assert!(!f.endpoint.close.is_cancelled());
        assert_eq!(f.endpoint.inputs.len(), 64);
        drop(updates.remove(0));
        assert!(f.endpoint.close.is_cancelled());
        let inputs = f.endpoint.inputs.clone();
        while !inputs.is_empty() {
            tokio::task::yield_now().await;
        }
        for update in updates {
            assert!(matches!(update.await, Err(AsyncGhosttyPumpError::Closed)));
        }
        let report = f.endpoint.wait().await.unwrap();
        assert_eq!(report.lifecycle, LifecycleDelivery::Cancelled);
        assert!(Arc::ptr_eq(
            &good,
            model.reader().poll().unwrap().frame.as_ref().unwrap()
        ));
        assert_eq!(f.peer.recv().await.unwrap(), Peer::Closed);
    })
    .await;
}

#[tokio::test]
async fn native_search_trait_progress_limits_reveal_and_clear() {
    use oneterm_terminal::ghostty::search::{MAX_NEEDLE_BYTES, Options, Status};
    use oneterm_terminal::ghostty_session::SurfaceSession;
    bounded(async {
        let f = setup().await;
        f.endpoint.paste("").await.unwrap();
        let surface: &dyn SurfaceSession = &f.endpoint;
        assert!(
            surface
                .search("x".repeat(MAX_NEEDLE_BYTES + 1), Options::default())
                .await
                .unwrap_err()
                .is_search_rejection()
        );
        assert!(
            surface
                .search(
                    "x".into(),
                    Options {
                        case_sensitive: true,
                        whole_word: false
                    }
                )
                .await
                .unwrap_err()
                .is_search_rejection()
        );
        let mut found = surface
            .search("READY".into(), Options::default())
            .await
            .unwrap();
        for _ in 0..64 {
            if found.status == Status::Complete {
                break;
            }
            found = surface.search_step().await.unwrap();
        }
        assert_eq!(found.status, Status::Complete);
        assert_eq!(found.matches.len(), 1);
        assert!(surface.query().matches_search(&found));
        let stale = found.navigation(0).unwrap();
        let revealed = surface
            .reveal_search(found.navigation(0).unwrap())
            .await
            .unwrap();
        assert!(surface.query().matches_search(&revealed));
        assert!(
            surface
                .reveal_search(stale)
                .await
                .unwrap_err()
                .is_search_rejection()
        );
        assert!(!f.endpoint.close.is_cancelled());
        assert!(
            surface
                .search(String::new(), Options::default())
                .await
                .unwrap()
                .matches
                .is_empty()
        );
        f.endpoint.close().unwrap();
        f.endpoint.wait().await.unwrap();
    })
    .await;
}

#[tokio::test]
async fn native_owner_selection_ack_highlight_copy_and_rejection_recovery() {
    use oneterm_terminal::ghostty::anchors::{Coordinate, Point, Space};
    bounded(async {
        let f = setup().await;
        let model = f.endpoint.model();
        f.endpoint.paste("").await.unwrap();
        f.endpoint.state().release_repaint_hint();
        let start = model
            .create_anchor(Point {
                space: Space::Active,
                coordinate: Coordinate { column: 1, row: 0 },
            })
            .await
            .unwrap();
        let end = model
            .create_anchor(Point {
                space: Space::Active,
                coordinate: Coordinate { column: 3, row: 0 },
            })
            .await
            .unwrap();
        let update = SelectionUpdate::Set {
            start: start.clone(),
            end: end.clone(),
            rectangle: false,
        };
        drop(f.endpoint.update_selection(update.clone()));
        assert!(f.endpoint.inputs.is_empty());
        f.endpoint.update_selection(update.clone()).await.unwrap();
        let mut content = oneterm_terminal::ghostty_content::Content::new(&f.endpoint);
        content.poll().unwrap();
        let selected = content.viewport().unwrap().clone();
        assert_eq!(selected.selection_spans().len(), 1);
        assert_eq!(selected.selection_spans()[0].columns, 1..4);
        let good = model.reader().poll().unwrap().frame.unwrap();
        assert_eq!(
            good.source()
                .rows
                .iter()
                .flat_map(|row| &row.cells)
                .filter(|cell| cell.selected)
                .count(),
            3
        );
        assert_eq!(
            model.selection_text().await.unwrap().as_deref(),
            Some("ead")
        );
        assert_eq!(f.events.try_recv().unwrap(), SessionEvent::Output);
        model.release_anchor(&end).await.unwrap();
        assert!(
            f.endpoint
                .update_selection(update)
                .await
                .unwrap_err()
                .is_selection_rejection()
        );
        assert!(!f.endpoint.close.is_cancelled());
        assert!(Arc::ptr_eq(
            &good,
            model.reader().poll().unwrap().frame.as_ref().unwrap()
        ));
        assert_eq!(
            model.selection_text().await.unwrap().as_deref(),
            Some("ead")
        );
        f.endpoint.state().release_repaint_hint();
        let native_surface: &dyn oneterm_terminal::ghostty_session::SurfaceSession = &f.endpoint;
        native_surface
            .selection(SelectionUpdate::All)
            .await
            .unwrap();
        assert_eq!(
            model.selection_text().await.unwrap().as_deref(),
            Some("ready")
        );
        f.endpoint
            .update_selection(SelectionUpdate::Clear)
            .await
            .unwrap();
        assert_eq!(model.selection_text().await.unwrap(), None);
        content.poll().unwrap();
        assert!(content.viewport().unwrap().selection_spans().is_empty());
        assert_eq!(selected.selection_spans().len(), 1);
        assert!(
            model
                .reader()
                .poll()
                .unwrap()
                .frame
                .unwrap()
                .source()
                .rows
                .iter()
                .flat_map(|row| &row.cells)
                .all(|cell| !cell.selected)
        );
        assert_eq!(f.events.try_recv().unwrap(), SessionEvent::Output);
        assert!(f.peer.is_empty());
        model.release_anchor(&start).await.unwrap();
        use oneterm_terminal::ghostty::selection::{Gesture, GestureId};
        let surface: &dyn oneterm_terminal::ghostty_session::SurfaceSession = &f.endpoint;
        let id = GestureId::default();
        let viewport = model.query().viewport.unwrap();
        surface
            .selection(SelectionUpdate::Gesture(Gesture::Begin {
                id: id.clone(),
                viewport: viewport.clone(),
                coordinate: Coordinate { column: 1, row: 0 },
            }))
            .await
            .unwrap();
        surface
            .selection(SelectionUpdate::Gesture(Gesture::Move {
                id: id.clone(),
                viewport,
                coordinate: Coordinate { column: 3, row: 0 },
            }))
            .await
            .unwrap();
        surface
            .selection(SelectionUpdate::Gesture(Gesture::Finish(id)))
            .await
            .unwrap();
        assert_eq!(
            model.selection_text().await.unwrap().as_deref(),
            Some("ead")
        );
        f.endpoint.close().unwrap();
        assert!(matches!(
            f.endpoint.update_selection(SelectionUpdate::Clear).await,
            Err(AsyncGhosttyPumpError::Transport(TerminalError::Closed))
        ));
        f.endpoint.wait().await.unwrap();
    })
    .await;
}

#[tokio::test]
async fn native_owner_publication_waits_for_ui_and_resize_fifo_before_ack() {
    use oneterm_terminal::ghostty::anchors::{Coordinate, Point, Space};
    use oneterm_terminal::ghostty::render::Rgb;
    bounded(async {
        let f = setup().await;
        let model = f.endpoint.model();
        let start = model
            .create_anchor(Point {
                space: Space::Active,
                coordinate: Coordinate { column: 1, row: 0 },
            })
            .await
            .unwrap();
        let good = model.reader().poll().unwrap().frame.unwrap();
        blocked(&f).await;
        f.endpoint.resize(40, 6, 10, 20).unwrap();
        let mut selection = Box::pin(f.endpoint.update_selection(SelectionUpdate::Set {
            start: start.clone(),
            end: start,
            rectangle: false,
        }));
        assert!(
            selection
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending()
        );
        let colors = DefaultColors {
            foreground: Rgb {
                r: 17,
                g: 34,
                b: 51,
            },
            background: Rgb::default(),
            cursor: None,
            palette: [Rgb::default(); 256],
        };
        let mut update = Box::pin(f.endpoint.set_default_colors(colors.clone()));
        assert!(
            update
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending()
        );
        assert!(Arc::ptr_eq(
            &good,
            model.reader().poll().unwrap().frame.as_ref().unwrap()
        ));
        assert_eq!(f.endpoint.inputs.len(), 3);
        f.endpoint.write(b"probe").unwrap();
        assert_eq!(f.peer.recv().await.unwrap(), Peer::Bytes(b"probe".to_vec()));
        assert_eq!(f.events.recv().await.unwrap(), SessionEvent::Bell);
        assert_eq!(
            f.events.recv().await.unwrap(),
            SessionEvent::Title("blocked".into())
        );
        selection.await.unwrap();
        update.await.unwrap();
        assert_eq!(f.peer.recv().await.unwrap(), Peer::Resize(40, 6));
        let frame = model.reader().poll().unwrap().frame.unwrap();
        assert_eq!(frame.source().cols, 40);
        assert_eq!(frame.source().foreground, colors.foreground);
        assert!(frame.source().rows[0].cells[1].selected);
        assert_eq!(model.selection_text().await.unwrap().as_deref(), Some("e"));
        f.endpoint.close().unwrap();
        f.endpoint.wait().await.unwrap();
    })
    .await;
}

#[tokio::test]
async fn native_owner_publication_fatal_delivery_ack_precedes_teardown() {
    use oneterm_terminal::ghostty::render::Rgb;
    bounded(async {
        for colors_update in [false, true] {
            let (tx, _peer) = async_channel::bounded(32);
            let (address, server) = spawn_server(Probe(tx)).await;
            let _server = ServerGuard(server);
            let (mut handle, _trust) = connect_trusting_loopback(address).await;
            authenticate_with_password(&mut handle, "user", "secret")
                .await
                .unwrap();
            let channel = handle.channel_open_session().await.unwrap();
            let (_reader, writer) = channel.split();
            let (commands_tx, commands) = async_channel::bounded(64);
            let transport = SshTransport::new(commands_tx);
            let (_inputs_tx, inputs) = async_channel::bounded(64);
            let (events_tx, events_rx) = async_channel::bounded(16);
            let events = SessionEventSink::new(events_tx);
            let state = SharedSessionState::new_alive();
            let router = OscRouter::new(
                transport.clone(),
                events.clone(),
                state.clone(),
                ClipboardOrigin::Remote,
            );
            let mut pump = AsyncGhosttyPump::spawn(router, 16, 3).unwrap();
            let good = pump.finish_batch().await.unwrap();
            let model = pump.model();
            pump.process_chunk(b"\x1b]2;pending-selection\x07")
                .await
                .unwrap();
            drop(events_rx);
            let io = OwnerIo {
                transport: transport.clone(),
                commands,
                inputs,
                close: CancellationToken::new(),
                state,
                events,
            };
            let (reply, ack) = async_channel::bounded(1);
            let result = if colors_update {
                let colors = Box::new(DefaultColors {
                    foreground: Rgb::default(),
                    background: Rgb::default(),
                    cursor: None,
                    palette: [Rgb::default(); 256],
                });
                io.default_colors(&mut pump, &writer, colors, reply).await
            } else {
                io.selection(&mut pump, &writer, SelectionUpdate::Clear, reply)
                    .await
            };
            assert!(matches!(
                result,
                Err(Error::Native(AsyncGhosttyPumpError::EventsClosed))
            ));
            assert!(matches!(
                ack.recv().await.unwrap(),
                Err(AsyncGhosttyPumpError::EventsClosed)
            ));
            assert!(transport.is_closing());
            assert!(Arc::ptr_eq(
                &good,
                model.reader().poll().unwrap().frame.as_ref().unwrap()
            ));
            assert!(pump.shutdown().await.is_err());
            writer.close().await.unwrap();
        }
    })
    .await;
}

#[tokio::test]
async fn native_owner_selection_full_queue_cancel_and_pending_ack_cleanup() {
    bounded(async {
        for clear in [false, true] {
            let f = setup().await;
            drop(f.endpoint.clear_command());
            assert!(f.endpoint.inputs.is_empty());
            let model = f.endpoint.model();
            let good = model.reader().poll().unwrap().frame.unwrap();
            blocked(&f).await;
            let mut selections: Vec<_> = (0..64)
                .map(|_| {
                    Box::pin(async {
                        if clear {
                            f.endpoint.clear_command().await
                        } else {
                            f.endpoint.update_selection(SelectionUpdate::Clear).await
                        }
                    })
                })
                .collect();
            for selection in &mut selections {
                assert!(
                    selection
                        .as_mut()
                        .poll(&mut Context::from_waker(Waker::noop()))
                        .is_pending()
                );
            }
            assert!(matches!(
                async {
                    if clear {
                        f.endpoint.clear_command().await
                    } else {
                        f.endpoint.update_selection(SelectionUpdate::Clear).await
                    }
                }
                .await,
                Err(AsyncGhosttyPumpError::Transport(TerminalError::QueueFull))
            ));
            assert!(!f.endpoint.close.is_cancelled());
            assert_eq!(f.endpoint.inputs.len(), 64);
            drop(selections.remove(0));
            assert!(f.endpoint.close.is_cancelled());
            let inputs = f.endpoint.inputs.clone();
            // Teardown must drop every queued sender even while callers keep waiting.
            while !inputs.is_empty() {
                tokio::task::yield_now().await;
            }
            for selection in selections {
                assert!(matches!(
                    selection.await,
                    Err(AsyncGhosttyPumpError::Closed)
                ));
            }
            let report = f.endpoint.wait().await.unwrap();
            assert_eq!(report.lifecycle, LifecycleDelivery::Cancelled);
            assert!(Arc::ptr_eq(
                &good,
                model.reader().poll().unwrap().frame.as_ref().unwrap()
            ));
            assert_eq!(f.peer.recv().await.unwrap(), Peer::Closed);
        }
    })
    .await;
}

#[tokio::test]
async fn native_owner_scroll_publishes_history_without_peer_output() {
    bounded(async {
        let f = setup().await;
        let model = f.endpoint.model();
        f.endpoint.write(b"history").unwrap();
        assert_eq!(
            f.peer.recv().await.unwrap(),
            Peer::Bytes(b"history".to_vec())
        );
        let bottom = loop {
            let frame = model.reader().poll().unwrap().frame.unwrap();
            if frame.source().metadata.scrollbar.total > 3 {
                break frame;
            }
            tokio::task::yield_now().await;
        };
        // Empty plain paste is a no-wire control barrier: the preceding output
        // batch must finish posting its repaint before this consumer releases it.
        f.endpoint.paste("").await.unwrap();
        while f.events.try_recv().is_ok() {}
        f.endpoint.state().release_repaint_hint();
        f.endpoint.scroll(Scroll::Top).unwrap();
        assert_eq!(f.events.recv().await.unwrap(), SessionEvent::Output);
        let top = model.reader().poll().unwrap().frame.unwrap();
        assert_eq!(top.source().metadata.scrollbar.offset, 0);
        assert_eq!(top.source().rows[0].cells[0].text, "z");
        assert!(!Arc::ptr_eq(&bottom, &top));
        for (scroll, offset) in [
            (Scroll::Delta(1), 1),
            (Scroll::Row(0), 0),
            (Scroll::Delta(1), 1),
            (Scroll::Delta(-1), 0),
        ] {
            f.endpoint.state().release_repaint_hint();
            f.endpoint.scroll(scroll).unwrap();
            assert_eq!(f.events.recv().await.unwrap(), SessionEvent::Output);
            assert_eq!(
                model
                    .reader()
                    .poll()
                    .unwrap()
                    .frame
                    .unwrap()
                    .source()
                    .metadata
                    .scrollbar
                    .offset,
                offset
            );
        }
        f.endpoint.state().release_repaint_hint();
        f.endpoint.scroll(Scroll::Bottom).unwrap();
        assert_eq!(f.events.recv().await.unwrap(), SessionEvent::Output);
        assert_eq!(
            model.reader().poll().unwrap().frame.unwrap().source(),
            bottom.source()
        );
        assert!(f.peer.is_empty(), "viewport changes send no peer bytes");
        f.endpoint.close().unwrap();
        assert!(matches!(
            f.endpoint.scroll(Scroll::Top),
            Err(InputError::Admission(TerminalError::Closed))
        ));
        f.endpoint.wait().await.unwrap();
    })
    .await;
}

#[tokio::test]
async fn native_search_queue_pressure_and_post_admission_cancellation() {
    use oneterm_terminal::ghostty::search::Request;
    bounded(async {
        let f = setup().await;
        blocked(&f).await;
        drop(f.endpoint.search_request(Request::Step));
        assert!(f.endpoint.inputs.is_empty());
        let mut searches: Vec<_> = (0..64)
            .map(|_| Box::pin(f.endpoint.search_request(Request::Step)))
            .collect();
        for search in &mut searches {
            assert!(
                search
                    .as_mut()
                    .poll(&mut Context::from_waker(Waker::noop()))
                    .is_pending()
            );
        }
        assert!(matches!(
            f.endpoint.search_request(Request::Step).await,
            Err(AsyncGhosttyPumpError::Transport(TerminalError::QueueFull))
        ));
        assert!(!f.endpoint.close.is_cancelled());
        assert_eq!(f.endpoint.inputs.len(), 64);
        drop(searches.remove(0));
        assert!(f.endpoint.close.is_cancelled());
        for search in searches {
            assert!(matches!(search.await, Err(AsyncGhosttyPumpError::Closed)));
        }
        let inputs = f.endpoint.inputs.clone();
        assert_eq!(
            f.endpoint.wait().await.unwrap().lifecycle,
            LifecycleDelivery::Cancelled
        );
        assert!(inputs.is_empty());
        assert_eq!(f.peer.recv().await.unwrap(), Peer::Closed);
    })
    .await;
}

#[tokio::test]
async fn native_owner_scroll_waits_for_ui_then_publishes_in_control_order() {
    bounded(async {
        let f = setup().await;
        let model = f.endpoint.model();
        f.endpoint.write(b"history").unwrap();
        assert_eq!(
            f.peer.recv().await.unwrap(),
            Peer::Bytes(b"history".to_vec())
        );
        loop {
            if model
                .reader()
                .poll()
                .unwrap()
                .frame
                .unwrap()
                .source()
                .metadata
                .scrollbar
                .total
                > 3
            {
                break;
            }
            tokio::task::yield_now().await;
        }
        let good = model.reader().poll().unwrap().frame.unwrap();
        blocked_with_reply(&f, b"\x1b[3;5R").await;
        f.endpoint.scroll(Scroll::Top).unwrap();
        f.endpoint.scroll(Scroll::Delta(1)).unwrap();
        f.endpoint.write(b"probe").unwrap();
        assert_eq!(f.peer.recv().await.unwrap(), Peer::Bytes(b"probe".to_vec()));
        assert_eq!(f.endpoint.inputs.len(), 2);
        assert!(Arc::ptr_eq(
            model.reader().poll().unwrap().frame.as_ref().unwrap(),
            &good
        ));
        assert_eq!(f.events.recv().await.unwrap(), SessionEvent::Bell);
        assert_eq!(
            f.events.recv().await.unwrap(),
            SessionEvent::Title("blocked".into())
        );
        loop {
            if model
                .reader()
                .poll()
                .unwrap()
                .frame
                .unwrap()
                .source()
                .metadata
                .scrollbar
                .offset
                == 1
            {
                break;
            }
            tokio::task::yield_now().await;
        }
        f.endpoint.close().unwrap();
        f.endpoint.wait().await.unwrap();
    })
    .await;
}

#[tokio::test]
async fn native_owner_paste_remote_modes_filter_policy_and_no_capture() {
    bounded(async {
        let f = setup().await;
        let model = f.endpoint.model();
        let good = model.reader().poll().unwrap().frame.unwrap();
        drop(f.endpoint.paste("unpolled"));
        assert!(f.endpoint.inputs.is_empty());
        let oversized = "x".repeat(1024 * 1024 + 1);
        assert!(matches!(
            f.endpoint.paste(&oversized).await,
            Err(AsyncGhosttyPumpError::Paste(_))
        ));
        assert_eq!(f.endpoint.paste_budget.load(Ordering::Acquire), 0);
        f.endpoint.paste("").await.unwrap();
        f.endpoint.paste("a\r\nb\nc\x1b\x03").await.unwrap();
        assert_eq!(
            f.peer.recv().await.unwrap(),
            Peer::Bytes(b"a\rb\rc\x1b\x03".to_vec())
        );
        assert!(Arc::ptr_eq(
            model.reader().poll().unwrap().frame.as_ref().unwrap(),
            &good
        ));
        f.endpoint.write(b"paste-on").unwrap();
        assert_eq!(
            f.peer.recv().await.unwrap(),
            Peer::Bytes(b"paste-on".to_vec())
        );
        assert_eq!(
            f.events.recv().await.unwrap(),
            SessionEvent::Title("paste-on".into())
        );
        let enabled = loop {
            let frame = model.reader().poll().unwrap().frame.unwrap();
            if frame.source().metadata.modes.bracketed_paste {
                break frame;
            }
            tokio::task::yield_now().await;
        };
        f.endpoint
            .paste("a\x1b[201~b\x1b[200~c\x1b[31m\x03\n")
            .await
            .unwrap();
        assert_eq!(
            f.peer.recv().await.unwrap(),
            Peer::Bytes(b"\x1b[200~abc[31m\n\x1b[201~".to_vec())
        );
        f.endpoint.paste("").await.unwrap();
        assert_eq!(
            f.peer.recv().await.unwrap(),
            Peer::Bytes(b"\x1b[200~\x1b[201~".to_vec())
        );
        assert!(Arc::ptr_eq(
            model.reader().poll().unwrap().frame.as_ref().unwrap(),
            &enabled
        ));
        assert!(f.events.is_empty());
        f.endpoint.write(b"paste-off").unwrap();
        assert_eq!(
            f.peer.recv().await.unwrap(),
            Peer::Bytes(b"paste-off".to_vec())
        );
        assert_eq!(
            f.events.recv().await.unwrap(),
            SessionEvent::Title("paste-off".into())
        );
        f.endpoint.paste("plain\n").await.unwrap();
        assert_eq!(
            f.peer.recv().await.unwrap(),
            Peer::Bytes(b"plain\r".to_vec())
        );
        assert_eq!(f.endpoint.paste_budget.load(Ordering::Acquire), 0);
        f.endpoint.close().unwrap();
        assert!(matches!(
            f.endpoint.paste("closed").await,
            Err(AsyncGhosttyPumpError::Transport(TerminalError::Closed))
        ));
        assert_eq!(f.endpoint.paste_budget.load(Ordering::Acquire), 0);
        f.endpoint.wait().await.unwrap();
    })
    .await;
}

#[tokio::test]
async fn native_owner_visible_paste_trait_uses_original_peer_and_live_filter() {
    use oneterm_terminal::ghostty_session::SurfaceSession;
    bounded(async {
        let f = setup().await;
        let surface: &dyn SurfaceSession = &f.endpoint;
        let model = f.endpoint.model();
        let good = model.reader().poll().unwrap().frame.unwrap();
        surface.paste_visible("").await.unwrap();
        assert!(Arc::ptr_eq(&good, &model.reader().poll().unwrap().frame.unwrap()));
        assert!(matches!(surface.paste_visible(&"x".repeat(1024 * 1024 + 1)).await, Err(AsyncGhosttyPumpError::Paste(_))));
        surface.paste_visible("plain\n").await.unwrap();
        assert_eq!(f.peer.recv().await.unwrap(), Peer::Bytes(b"plain\r".to_vec()));
        f.endpoint.write(b"paste-on").unwrap();
        assert_eq!(f.peer.recv().await.unwrap(), Peer::Bytes(b"paste-on".to_vec()));
        loop {
            if matches!(f.events.recv().await.unwrap(), SessionEvent::Title(title) if title == "paste-on") { break; }
        }
        surface.paste_visible("a\x1b[201~b\x1b\x03\n").await.unwrap();
        assert_eq!(f.peer.recv().await.unwrap(), Peer::Bytes(b"\x1b[200~ab\n\x1b[201~".to_vec()));
        assert_eq!(f.endpoint.paste_budget.load(Ordering::Acquire), 0);
        f.endpoint.close().unwrap();
        f.endpoint.wait().await.unwrap();
    }).await;
}

#[tokio::test]
async fn native_owner_paste_transport_failure_ack_releases_active_text() {
    bounded(async {
        let (tx, _peer) = async_channel::bounded(32);
        let (address, server) = spawn_server(Probe(tx)).await;
        let _server = ServerGuard(server);
        let (mut handle, _trust) = connect_trusting_loopback(address).await;
        authenticate_with_password(&mut handle, "user", "secret")
            .await
            .unwrap();
        let channel = handle.channel_open_session().await.unwrap();
        let (_reader, writer) = channel.split();
        // Isolate the paste acknowledgment boundary: keep the original
        // transport receiver full, while drive's injected command source stays
        // empty. Thus lookup cannot race a normal drain and mask QueueFull.
        let (command_tx, command_rx) = async_channel::bounded(1);
        let transport = SshTransport::new(command_tx);
        transport.pty_write(b"occupied").unwrap();
        let (_unused_tx, commands) = async_channel::bounded(1);
        let (_input_tx, inputs) = async_channel::bounded(64);
        let (events, _events_rx) = async_channel::bounded(16);
        let state = SharedSessionState::new_alive();
        let events = SessionEventSink::new(events);
        let router = OscRouter::new(
            transport.clone(),
            events.clone(),
            state.clone(),
            ClipboardOrigin::Remote,
        );
        let mut pump = AsyncGhosttyPump::spawn(router, 16, 3).unwrap();
        let good = pump.finish_batch().await.unwrap();
        let model = pump.model();
        let io = OwnerIo {
            transport: transport.clone(),
            commands,
            inputs,
            close: CancellationToken::new(),
            state,
            events,
        };
        let budget = Arc::new(ByteBudget::<PASTE_BYTE_BUDGET>::default());
        let text = "rejected".to_owned();
        assert!(budget.reserve(text.len()));
        let (reply, ack) = async_channel::bounded(1);
        let paste = Paste {
            reveal: false,
            _reservation: PasteReservation {
                budget: budget.clone(),
                bytes: text.len(),
            },
            text,
            result: reply,
        };
        assert!(matches!(
            io.paste(&mut pump, &writer, paste).await,
            Err(Error::Native(AsyncGhosttyPumpError::Transport(
                TerminalError::QueueFull
            )))
        ));
        assert!(matches!(
            ack.recv().await.unwrap(),
            Err(AsyncGhosttyPumpError::Transport(TerminalError::QueueFull))
        ));
        assert_eq!(budget.load(Ordering::Acquire), 0);
        assert!(transport.is_closing());
        assert!(
            matches!(command_rx.try_recv().unwrap(), Cmd::Write(bytes) if bytes == b"occupied")
        );
        transport.release_write_bytes(b"occupied".len());
        assert!(command_rx.is_empty());
        assert_eq!(transport.diagnostics().queued_write_bytes, 0);
        assert!(Arc::ptr_eq(
            model.reader().poll().unwrap().frame.as_ref().unwrap(),
            &good
        ));
        assert!(pump.shutdown().await.is_err());
        writer.close().await.unwrap();
    })
    .await;
}

#[tokio::test]
async fn native_owner_paste_budget_cancel_wakes_full_ui_and_commands() {
    bounded(async {
        let f = setup().await;
        let model = f.endpoint.model();
        let good = model.reader().poll().unwrap().frame.unwrap();
        blocked(&f).await;
        let text = "x".repeat(1024 * 1024);
        let mut pastes: Vec<_> = (0..4).map(|_| Box::pin(f.endpoint.paste(&text))).collect();
        for paste in &mut pastes {
            assert!(
                paste
                    .as_mut()
                    .poll(&mut Context::from_waker(Waker::noop()))
                    .is_pending()
            );
        }
        assert_eq!(
            f.endpoint.paste_budget.load(Ordering::Acquire),
            PASTE_BYTE_BUDGET
        );
        f.endpoint.scroll(Scroll::Top).unwrap();
        assert!(matches!(
            f.endpoint.paste("x").await,
            Err(AsyncGhosttyPumpError::Transport(TerminalError::QueueFull))
        ));
        assert!(!f.endpoint.transport.is_closing());
        for _ in 0..SSH_COMMAND_QUEUE_CAPACITY {
            f.endpoint.write(b"never").unwrap();
        }
        assert_eq!(f.endpoint.write(b"full"), Err(TerminalError::QueueFull));
        drop(pastes.remove(0));
        assert!(f.endpoint.close.is_cancelled());
        drop(pastes);
        let budget = f.endpoint.paste_budget.clone();
        let transport = f.endpoint.transport.clone();
        let report = f.endpoint.wait().await.unwrap();
        assert_eq!(report.stop, Stop::Requested);
        assert_eq!(report.lifecycle, LifecycleDelivery::Cancelled);
        assert_eq!(budget.load(Ordering::Acquire), 0);
        assert_eq!(transport.diagnostics().queued_write_bytes, 0);
        assert_eq!(f.peer.recv().await.unwrap(), Peer::Closed);
        assert!(f.peer.is_empty());
        assert!(Arc::ptr_eq(
            model.reader().poll().unwrap().frame.as_ref().unwrap(),
            &good
        ));
    })
    .await;
}

#[tokio::test]
async fn native_owner_paste_orders_resize_paste_key_after_ui_release() {
    bounded(async {
        use oneterm_terminal::ghostty::input::KeyCode;
        let f = setup().await;
        blocked(&f).await;
        f.endpoint.resize(40, 6, 10, 20).unwrap();
        let mut paste = Box::pin(f.endpoint.paste("queued"));
        assert!(
            paste
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending()
        );
        f.endpoint.key(KeyEvent::new(KeyCode::ENTER)).unwrap();
        f.endpoint.resize(80, 12, 10, 20).unwrap();
        f.endpoint.write(b"probe").unwrap();
        assert_eq!(f.peer.recv().await.unwrap(), Peer::Bytes(b"probe".to_vec()));
        assert_eq!(f.endpoint.inputs.len(), 4);
        assert_eq!(f.events.recv().await.unwrap(), SessionEvent::Bell);
        assert_eq!(
            f.events.recv().await.unwrap(),
            SessionEvent::Title("blocked".into())
        );
        paste.await.unwrap();
        assert_eq!(f.endpoint.paste_budget.load(Ordering::Acquire), 0);
        assert_eq!(f.peer.recv().await.unwrap(), Peer::Resize(40, 6));
        let mut bytes = Vec::new();
        while bytes.len() < b"queued\r".len() {
            let Peer::Bytes(chunk) = f.peer.recv().await.unwrap() else {
                panic!("unexpected resize/close before input");
            };
            bytes.extend(chunk);
        }
        assert_eq!(bytes, b"queued\r");
        assert_eq!(f.peer.recv().await.unwrap(), Peer::Resize(80, 12));
        f.endpoint.close().unwrap();
        f.endpoint.wait().await.unwrap();
    })
    .await;
}

#[tokio::test]
async fn native_owner_close_wakes_full_ui_and_command_queues() {
    bounded(async {
        let f = setup().await;
        let model = f.endpoint.model();
        let good = model.reader().poll().unwrap().frame.unwrap();
        blocked(&f).await;
        for _ in 0..SSH_COMMAND_QUEUE_CAPACITY {
            f.endpoint.write(b"never").unwrap();
        }
        assert_eq!(f.endpoint.write(b"full"), Err(TerminalError::QueueFull));
        f.endpoint.set_marked_text("pending".into()).unwrap();
        assert_eq!(
            f.endpoint.commit_text("full"),
            Err(oneterm_terminal::ghostty_ime::Error::Input(
                TerminalError::QueueFull
            ))
        );
        assert_eq!(f.endpoint.marked_text(), None);
        assert!(!f.endpoint.close.is_cancelled());
        let transport = f.endpoint.transport.clone();
        f.endpoint.close().unwrap();
        let report = f.endpoint.wait().await.unwrap();
        assert_eq!(report.stop, Stop::Requested);
        assert_eq!(report.lifecycle, LifecycleDelivery::Cancelled);
        assert_eq!(f.peer.recv().await.unwrap(), Peer::Closed);
        assert_eq!(transport.diagnostics().queued_write_bytes, 0);
        assert_eq!(f.events.recv().await.unwrap(), SessionEvent::Bell);
        assert!(f.events.is_empty());
        let update = model.reader().poll().unwrap();
        assert!(Arc::ptr_eq(update.frame.as_ref().unwrap(), &good));
        assert!(matches!(update.state, State::Failed(_)));
    })
    .await;
}

#[tokio::test]
async fn native_owner_release_ui_publishes_then_peer_exit_tears_down() {
    bounded(async {
        let f = setup().await;
        let model = f.endpoint.model();
        let good = model.reader().poll().unwrap().frame.unwrap();
        blocked(&f).await;
        assert_eq!(f.events.recv().await.unwrap(), SessionEvent::Bell);
        assert_eq!(
            f.events.recv().await.unwrap(),
            SessionEvent::Title("blocked".into())
        );
        loop {
            let frame = model.reader().poll().unwrap().frame.unwrap();
            if !Arc::ptr_eq(&frame, &good) {
                break;
            }
            tokio::task::yield_now().await;
        }
        f.endpoint.write(b"exit").unwrap();
        let state = f.endpoint.state();
        loop {
            match f.events.recv().await.unwrap() {
                SessionEvent::Output => {}
                SessionEvent::Exited(Some(7)) => break,
                event => panic!("unexpected lifecycle event: {event:?}"),
            }
        }
        assert_eq!(f.events.recv().await.unwrap(), SessionEvent::Closed);
        let report = f.endpoint.wait().await.unwrap();
        assert_eq!(
            report,
            Report {
                stop: Stop::Eof,
                exit_code: Some(7),
                lifecycle: LifecycleDelivery::Complete,
            }
        );
        assert!(!state.alive());
        assert!(matches!(
            model.reader().poll().unwrap().state,
            State::Closed
        ));
        let frame = model.reader().poll().unwrap().frame.unwrap();
        let text: String = frame.source().rows[0]
            .cells
            .iter()
            .map(|cell| cell.text.as_str())
            .collect();
        assert!(text.contains("goodbye"), "{text:?}");
    })
    .await;
}

#[tokio::test]
async fn native_owner_event_loss_tears_down_without_replacing_last_good() {
    bounded(async {
        let f = setup().await;
        let model = f.endpoint.model();
        let good = model.reader().poll().unwrap().frame.unwrap();
        f.events.close();
        f.endpoint.write(b"go").unwrap();
        assert!(matches!(
            f.endpoint.wait().await,
            Err(Error::Native(AsyncGhosttyPumpError::EventsClosed))
        ));
        let update = model.reader().poll().unwrap();
        assert!(Arc::ptr_eq(update.frame.as_ref().unwrap(), &good));
        assert!(matches!(update.state, State::Failed(_)));
        loop {
            if f.peer.recv().await.unwrap() == Peer::Closed {
                break;
            }
        }
    })
    .await;
}

#[tokio::test]
async fn native_owner_drop_closes_while_ui_is_backpressured() {
    bounded(async {
        let f = setup().await;
        blocked(&f).await;
        let state = f.endpoint.state();
        let model = f.endpoint.model();
        drop(f.endpoint);
        assert_eq!(f.peer.recv().await.unwrap(), Peer::Closed);
        while !matches!(model.reader().poll().unwrap().state, State::Failed(_)) || state.alive() {
            tokio::task::yield_now().await;
        }
    })
    .await;
}

#[tokio::test]
async fn native_owner_unpolled_write_releases_consumed_reservation() {
    bounded(async {
        let (tx, _peer) = async_channel::bounded(32);
        let (address, server) = spawn_server(Probe(tx)).await;
        let _server = ServerGuard(server);
        let (mut handle, _trust) = connect_trusting_loopback(address).await;
        authenticate_with_password(&mut handle, "user", "secret")
            .await
            .unwrap();
        let channel = handle.channel_open_session().await.unwrap();
        let (_reader, writer) = channel.split();
        let (tx, commands) = async_channel::bounded(1);
        let transport = SshTransport::new(tx);
        transport.pty_write(b"not sent").unwrap();
        let command = commands.recv().await;
        let io = OwnerIo {
            transport,
            commands,
            inputs: async_channel::bounded(1).1,
            close: CancellationToken::new(),
            state: SharedSessionState::new_alive(),
            events: SessionEventSink::new(async_channel::bounded(1).0),
        };
        assert_eq!(io.transport.diagnostics().queued_write_bytes, 8);
        // A biased close branch can win before this future's first poll.
        drop(io.command(command, &writer));
        assert_eq!(io.transport.diagnostics().queued_write_bytes, 0);
        writer.close().await.unwrap();
    })
    .await;
}

#[tokio::test]
async fn native_owner_idle_close_destroys_native_without_delivery_failure() {
    bounded(async {
        let f = setup().await;
        let model = f.endpoint.model();
        let state = f.endpoint.state();
        f.endpoint.close().unwrap();
        let report = f.endpoint.wait().await.unwrap();
        assert_eq!(report.stop, Stop::Requested);
        assert_eq!(report.lifecycle, LifecycleDelivery::Complete);
        assert_eq!(f.events.recv().await.unwrap(), SessionEvent::Closed);
        assert_eq!(f.peer.recv().await.unwrap(), Peer::Closed);
        assert!(matches!(
            model.reader().poll().unwrap().state,
            State::Closed
        ));
        assert!(!state.alive());
    })
    .await;
}

#[tokio::test]
async fn native_owner_cancelled_wait_requests_close_and_keeps_last_good() {
    bounded(async {
        let f = setup().await;
        let model = f.endpoint.model();
        let state = f.endpoint.state();
        let good = model.reader().poll().unwrap().frame.unwrap();
        blocked(&f).await;
        let mut wait = Box::pin(f.endpoint.wait());
        std::future::poll_fn(|cx| {
            assert!(wait.as_mut().poll(cx).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        drop(wait);
        assert_eq!(f.peer.recv().await.unwrap(), Peer::Closed);
        while state.alive() {
            tokio::task::yield_now().await;
        }
        assert!(Arc::ptr_eq(
            model.reader().poll().unwrap().frame.as_ref().unwrap(),
            &good
        ));
    })
    .await;
}

#[tokio::test]
async fn native_owner_native_failure_keeps_cause_and_last_good() {
    bounded(async {
        let f = setup().await;
        let model = f.endpoint.model();
        let good = model.reader().poll().unwrap().frame.unwrap();
        let transport = f.endpoint.transport.clone();
        // Final Closed also fails; the original native cause must win.
        f.events.close();
        f.endpoint.write(b"fault").unwrap();
        assert!(matches!(
            f.endpoint.wait().await,
            Err(Error::Native(AsyncGhosttyPumpError::Engine(
                oneterm_terminal::ghostty_bridge::Error::Owner(_)
            )))
        ));
        let update = model.reader().poll().unwrap();
        assert!(matches!(
            update.state,
            State::Failed(oneterm_terminal::ghostty_bridge::Error::Owner(_))
        ));
        assert!(Arc::ptr_eq(update.frame.as_ref().unwrap(), &good));
        assert_eq!(transport.diagnostics().queued_write_bytes, 0);
        loop {
            if f.peer.recv().await.unwrap() == Peer::Closed {
                break;
            }
        }
    })
    .await;
}

fn mouse_press() -> MouseEvent {
    use oneterm_terminal::ghostty::input::{Geometry, Modifiers, MouseAction, MouseButton};
    MouseEvent {
        action: MouseAction::Press,
        button: Some(MouseButton::Left),
        modifiers: Modifiers::default(),
        x: 15.0,
        y: 25.0,
        any_button_pressed: true,
        geometry: Geometry {
            width: 160,
            height: 60,
            cell_width: 10,
            cell_height: 20,
            padding_top: 0,
            padding_bottom: 0,
            padding_left: 0,
            padding_right: 0,
        },
    }
}

#[tokio::test]
async fn native_owner_physical_input_uses_remote_modes_and_fifo() {
    bounded(async {
        use oneterm_terminal::ghostty::input::KeyCode;
        let f = setup().await;
        f.endpoint.write(b"modes").unwrap();
        assert_eq!(f.peer.recv().await.unwrap(), Peer::Bytes(b"modes".to_vec()));
        assert_eq!(
            f.events.recv().await.unwrap(),
            SessionEvent::Title("input".into())
        );
        f.endpoint.key(KeyEvent::new(KeyCode::ARROW_UP)).unwrap();
        f.endpoint.mouse(mouse_press()).unwrap();
        f.endpoint.key(KeyEvent::new(KeyCode::ENTER)).unwrap();
        // Compare the stream, not russh message boundaries.
        let expected = b"\x1bOA\x1b[<0;2;2M\r";
        let mut bytes = Vec::new();
        while bytes.len() < expected.len() {
            let Peer::Bytes(chunk) = f.peer.recv().await.unwrap() else {
                panic!("unexpected peer close")
            };
            bytes.extend(chunk);
        }
        assert_eq!(bytes, expected);
        f.endpoint.close().unwrap();
        assert_eq!(f.endpoint.wait().await.unwrap().stop, Stop::Requested);
    })
    .await;
}

#[tokio::test]
async fn native_owner_invalid_input_is_rejected_without_stopping_owner() {
    bounded(async {
        use oneterm_terminal::ghostty::input::{KeyCode, MAX_KEY_TEXT_BYTES};
        let f = setup().await;
        let mut key = KeyEvent::new(KeyCode::UNIDENTIFIED);
        key.text = "x".repeat(MAX_KEY_TEXT_BYTES + 1);
        assert!(matches!(f.endpoint.key(key), Err(InputError::Invalid(_))));
        let mut key = KeyEvent::new(KeyCode::UNIDENTIFIED);
        key.text = "\x1b".into();
        assert!(matches!(f.endpoint.key(key), Err(InputError::Invalid(_))));
        let mut mouse = mouse_press();
        mouse.x = f32::NAN;
        assert!(matches!(
            f.endpoint.mouse(mouse),
            Err(InputError::Invalid(_))
        ));
        assert!(f.endpoint.state().alive());
        f.endpoint.key(KeyEvent::new(KeyCode::ENTER)).unwrap();
        assert_eq!(f.peer.recv().await.unwrap(), Peer::Bytes(b"\r".to_vec()));
        f.endpoint.close().unwrap();
        assert!(matches!(
            f.endpoint.key(KeyEvent::new(KeyCode::ENTER)),
            Err(InputError::Admission(TerminalError::Closed))
        ));
        f.endpoint.wait().await.unwrap();
    })
    .await;
}

#[tokio::test]
async fn native_owner_close_discards_full_input_queue_under_ui_pressure() {
    bounded(async {
        use oneterm_terminal::ghostty::input::KeyCode;
        let f = setup().await;
        let model = f.endpoint.model();
        let good = model.reader().poll().unwrap().frame.unwrap();
        blocked(&f).await;
        for _ in 0..INPUT_QUEUE_CAPACITY {
            f.endpoint.key(KeyEvent::new(KeyCode::ENTER)).unwrap();
        }
        assert!(matches!(
            f.endpoint.mouse(mouse_press()),
            Err(InputError::Admission(TerminalError::QueueFull))
        ));
        let inputs = f.endpoint.inputs.clone();
        assert!(matches!(
            f.endpoint.scroll(Scroll::Top),
            Err(InputError::Admission(TerminalError::QueueFull))
        ));
        assert!(matches!(
            f.endpoint.paste("queue full").await,
            Err(AsyncGhosttyPumpError::Transport(TerminalError::QueueFull))
        ));
        assert_eq!(f.endpoint.paste_budget.load(Ordering::Acquire), 0);
        assert!(!f.endpoint.transport.is_closing());
        f.endpoint.close().unwrap();
        f.endpoint.wait().await.unwrap();
        assert!(inputs.is_closed());
        assert!(inputs.is_empty());
        assert_eq!(f.peer.recv().await.unwrap(), Peer::Closed);
        assert!(Arc::ptr_eq(
            model.reader().poll().unwrap().frame.as_ref().unwrap(),
            &good
        ));
    })
    .await;
}

#[tokio::test]
async fn native_owner_queued_input_waits_for_batch_and_resumes_after_ui_release() {
    bounded(async {
        use oneterm_terminal::ghostty::input::KeyCode;
        let f = setup().await;
        blocked(&f).await;
        f.endpoint.key(KeyEvent::new(KeyCode::ENTER)).unwrap();
        f.endpoint.write(b"probe").unwrap();
        // Raw transport work still progresses; physical input must not reenter
        // the active pump while reliable event delivery holds the batch.
        assert_eq!(f.peer.recv().await.unwrap(), Peer::Bytes(b"probe".to_vec()));
        assert_eq!(f.endpoint.inputs.len(), 1);
        assert_eq!(f.events.recv().await.unwrap(), SessionEvent::Bell);
        assert_eq!(
            f.events.recv().await.unwrap(),
            SessionEvent::Title("blocked".into())
        );
        assert_eq!(f.peer.recv().await.unwrap(), Peer::Bytes(b"\r".to_vec()));
        f.endpoint.close().unwrap();
        f.endpoint.wait().await.unwrap();
    })
    .await;
}

async fn frame_size(model: &SharedModel, cols: u16, rows: u16) {
    loop {
        let frame = model.reader().poll().unwrap().frame.unwrap();
        if frame.source().cols == cols && frame.source().rows.len() == usize::from(rows) {
            break;
        }
        tokio::task::yield_now().await;
    }
}

#[tokio::test]
async fn native_owner_resize_coordinates_peer_frame_and_input_order() {
    bounded(async {
        use oneterm_terminal::ghostty::input::KeyCode;
        let f = setup().await;
        f.endpoint.resize(40, 6, 10, 20).unwrap();
        f.endpoint.key(KeyEvent::new(KeyCode::ENTER)).unwrap();
        f.endpoint.resize(80, 12, 10, 20).unwrap();
        assert_eq!(f.peer.recv().await.unwrap(), Peer::Resize(40, 6));
        assert_eq!(f.peer.recv().await.unwrap(), Peer::Bytes(b"\r".to_vec()));
        assert_eq!(f.peer.recv().await.unwrap(), Peer::Resize(80, 12));
        frame_size(&f.endpoint.model(), 80, 12).await;
        f.endpoint.close().unwrap();
        f.endpoint.wait().await.unwrap();
    })
    .await;
}

#[tokio::test]
async fn native_owner_invalid_resize_preserves_live_session_and_frame() {
    bounded(async {
        let f = setup().await;
        let model = f.endpoint.model();
        let good = model.reader().poll().unwrap().frame.unwrap();
        for (cols, rows) in [(0, 3), (16, 0), (u16::MAX, u16::MAX)] {
            assert!(matches!(
                f.endpoint.resize(cols, rows, 0, 0),
                Err(InputError::Invalid(_))
            ));
        }
        f.endpoint.write(b"probe").unwrap();
        assert_eq!(f.peer.recv().await.unwrap(), Peer::Bytes(b"probe".to_vec()));
        assert!(Arc::ptr_eq(
            model.reader().poll().unwrap().frame.as_ref().unwrap(),
            &good
        ));
        assert!(f.endpoint.state().alive());
        f.endpoint.close().unwrap();
        assert!(matches!(
            f.endpoint.resize(20, 4, 0, 0),
            Err(InputError::Admission(TerminalError::Closed))
        ));
        f.endpoint.wait().await.unwrap();
    })
    .await;
}

#[tokio::test]
async fn native_owner_resize_waits_for_ui_release_before_peer_and_capture() {
    bounded(async {
        let f = setup().await;
        let model = f.endpoint.model();
        let good = model.reader().poll().unwrap().frame.unwrap();
        blocked(&f).await;
        f.endpoint.resize(40, 6, 10, 20).unwrap();
        f.endpoint.write(b"probe").unwrap();
        assert_eq!(f.peer.recv().await.unwrap(), Peer::Bytes(b"probe".to_vec()));
        assert_eq!(f.endpoint.inputs.len(), 1);
        assert!(Arc::ptr_eq(
            model.reader().poll().unwrap().frame.as_ref().unwrap(),
            &good
        ));
        assert_eq!(f.events.recv().await.unwrap(), SessionEvent::Bell);
        assert_eq!(
            f.events.recv().await.unwrap(),
            SessionEvent::Title("blocked".into())
        );
        assert_eq!(f.peer.recv().await.unwrap(), Peer::Resize(40, 6));
        frame_size(&model, 40, 6).await;
        f.endpoint.close().unwrap();
        f.endpoint.wait().await.unwrap();
    })
    .await;
}

#[tokio::test]
async fn native_owner_close_discards_full_resize_queue_under_ui_pressure() {
    bounded(async {
        let f = setup().await;
        let model = f.endpoint.model();
        let good = model.reader().poll().unwrap().frame.unwrap();
        blocked(&f).await;
        for _ in 0..INPUT_QUEUE_CAPACITY {
            f.endpoint.resize(40, 6, 10, 20).unwrap();
        }
        assert!(matches!(
            f.endpoint.resize(80, 12, 10, 20),
            Err(InputError::Admission(TerminalError::QueueFull))
        ));
        let inputs = f.endpoint.inputs.clone();
        f.endpoint.close().unwrap();
        f.endpoint.wait().await.unwrap();
        assert!(inputs.is_closed() && inputs.is_empty());
        assert_eq!(f.peer.recv().await.unwrap(), Peer::Closed);
        assert!(Arc::ptr_eq(
            model.reader().poll().unwrap().frame.as_ref().unwrap(),
            &good
        ));
    })
    .await;
}

#[tokio::test]
async fn native_owner_physical_resize_failure_stops_before_capture() {
    bounded(async {
        let (tx, _peer) = async_channel::bounded(32);
        let (address, server) = spawn_server(Probe(tx)).await;
        let _server = ServerGuard(server);
        let (mut handle, _trust) = connect_trusting_loopback(address).await;
        authenticate_with_password(&mut handle, "user", "secret")
            .await
            .unwrap();
        let channel = handle.channel_open_session().await.unwrap();
        handle
            .disconnect(
                russh::Disconnect::ByApplication,
                "resize failure fixture",
                "",
            )
            .await
            .unwrap();
        // The task has completed and its physical command receiver is gone.
        // russh reports the intentional disconnect as an error on this handle.
        assert!(handle.await.is_err());
        let (tx, commands) = async_channel::bounded(SSH_COMMAND_QUEUE_CAPACITY);
        let transport = SshTransport::new(tx);
        let (inputs, input_rx) = async_channel::bounded(INPUT_QUEUE_CAPACITY);
        let state = SharedSessionState::new_alive();
        let (event_tx, _events) = async_channel::bounded(8);
        let router = OscRouter::new(
            transport.clone(),
            SessionEventSink::new(event_tx),
            state.clone(),
            ClipboardOrigin::Remote,
        );
        let mut pump = AsyncGhosttyPump::spawn(router, 16, 3).unwrap();
        let good = pump.finish_batch().await.unwrap();
        let model = pump.model();
        let io = OwnerIo {
            transport: transport.clone(),
            commands,
            inputs: input_rx,
            close: CancellationToken::new(),
            state: state.clone(),
            events: SessionEventSink::new(async_channel::bounded(1).0),
        };
        inputs
            .try_send(Input::Resize(Resize {
                cols: 40,
                rows: 6,
                cell_width_px: 10,
                cell_height_px: 20,
            }))
            .ok()
            .unwrap();
        // Input wins before the already disconnected reader. Native resize can
        // succeed, but the dead physical writer must prevent resized capture.
        assert!(matches!(run(channel, pump, io).await, Err(Error::Ssh(_))));
        assert!(!state.alive());
        assert!(inputs.is_closed() && inputs.is_empty());
        assert!(transport.is_closing());
        assert_eq!(transport.diagnostics().queued_write_bytes, 0);
        assert!(Arc::ptr_eq(
            model.reader().poll().unwrap().frame.as_ref().unwrap(),
            &good
        ));
    })
    .await;
}

#[tokio::test]
async fn native_owner_resize_barrier_preserves_fifo_with_or_without_marker() {
    bounded(async {
        for capacity in [1, 2] {
            let (tx, peer) = async_channel::bounded(32);
            let (address, server) = spawn_server(Probe(tx)).await;
            let _server = ServerGuard(server);
            let (mut handle, _trust) = connect_trusting_loopback(address).await;
            authenticate_with_password(&mut handle, "user", "secret")
                .await
                .unwrap();
            let channel = handle.channel_open_session().await.unwrap();
            let (_reader, writer) = channel.split();
            let (tx, commands) = async_channel::bounded(capacity);
            let transport = SshTransport::new(tx);
            transport.pty_write(b"before").unwrap();
            // Capacity 1 suppresses the marker; capacity 2 queues it behind
            // prior input. Both must deliver prior bytes before window-change.
            transport.pty_resize(6, 40).unwrap();
            let io = OwnerIo {
                transport: transport.clone(),
                commands,
                inputs: async_channel::bounded(1).1,
                close: CancellationToken::new(),
                state: SharedSessionState::new_alive(),
                events: SessionEventSink::new(async_channel::bounded(1).0),
            };
            assert!(matches!(
                io.flush_resize(&writer).await.unwrap(),
                Driver::Done(())
            ));
            assert_eq!(peer.recv().await.unwrap(), Peer::Bytes(b"before".to_vec()));
            assert_eq!(peer.recv().await.unwrap(), Peer::Resize(40, 6));
            assert_eq!(transport.diagnostics().queued_write_bytes, 0);
            assert!(transport.take_pending_resize().is_none());
            // A stale wake marker is harmless, not another remote resize.
            io.command(Ok(Cmd::Resize), &writer).await.unwrap();
            writer.data(&b"after"[..]).await.unwrap();
            assert_eq!(peer.recv().await.unwrap(), Peer::Bytes(b"after".to_vec()));
            writer.close().await.unwrap();
            drop(handle);
        }
    })
    .await;
}

#[tokio::test]
async fn native_owner_exit_event_precedes_late_output_and_closed() {
    bounded(async {
        let f = setup().await;
        f.endpoint.write(b"late-exit").unwrap();
        assert_eq!(
            f.peer.recv().await.unwrap(),
            Peer::Bytes(b"late-exit".to_vec())
        );
        assert_eq!(
            f.events.recv().await.unwrap(),
            SessionEvent::Exited(Some(9))
        );
        assert!(!f.endpoint.state().alive());
        assert_eq!(f.endpoint.state().exit_code(), Some(9));
        assert_eq!(
            f.events.recv().await.unwrap(),
            SessionEvent::Title("late".into())
        );
        loop {
            match f.events.recv().await.unwrap() {
                SessionEvent::Output => {}
                SessionEvent::Closed => break,
                event => panic!("unexpected lifecycle event: {event:?}"),
            }
        }
        let model = f.endpoint.model();
        assert_eq!(
            f.endpoint.wait().await.unwrap(),
            Report {
                stop: Stop::Eof,
                exit_code: Some(9),
                lifecycle: LifecycleDelivery::Complete
            }
        );
        let frame = model.reader().poll().unwrap().frame.unwrap();
        let text: String = frame.source().rows[0]
            .cells
            .iter()
            .map(|cell| cell.text.as_str())
            .collect();
        assert!(text.starts_with("ready after"), "{text:?}");
        assert!(matches!(
            model.reader().poll().unwrap().state,
            State::Closed
        ));
    })
    .await;
}

async fn final_lifecycle_blocked(f: &Fixture) {
    f.event_tx.try_send(SessionEvent::Bell).unwrap();
    f.endpoint.write(b"eof").unwrap();
    assert_eq!(f.peer.recv().await.unwrap(), Peer::Bytes(b"eof".to_vec()));
    assert_eq!(f.peer.recv().await.unwrap(), Peer::Closed);
    while !matches!(
        f.endpoint.model().reader().poll().unwrap().state,
        State::Closed
    ) {
        tokio::task::yield_now().await;
    }
    assert!(!f.endpoint.state().alive());
    assert_eq!(f.endpoint.write(b"never"), Err(TerminalError::Closed));
    assert!(matches!(
        f.endpoint.resize(20, 4, 0, 0),
        Err(InputError::Admission(TerminalError::Closed))
    ));
}

#[tokio::test]
async fn native_owner_final_closed_waits_after_physical_and_native_teardown() {
    bounded(async {
        let f = setup().await;
        final_lifecycle_blocked(&f).await;
        let mut wait = Box::pin(f.endpoint.wait());
        std::future::poll_fn(|cx| {
            assert!(wait.as_mut().poll(cx).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        assert_eq!(f.events.recv().await.unwrap(), SessionEvent::Bell);
        assert_eq!(f.events.recv().await.unwrap(), SessionEvent::Closed);
        assert_eq!(
            wait.await.unwrap(),
            Report {
                stop: Stop::Eof,
                exit_code: None,
                lifecycle: LifecycleDelivery::Complete
            }
        );
        assert!(f.events.is_empty());
    })
    .await;
}

#[tokio::test]
async fn native_owner_close_cancels_blocked_exit_delivery_without_replay() {
    bounded(async {
        let f = setup().await;
        let model = f.endpoint.model();
        let good = model.reader().poll().unwrap().frame.unwrap();
        f.event_tx.try_send(SessionEvent::Bell).unwrap();
        f.endpoint.write(b"status").unwrap();
        assert_eq!(
            f.peer.recv().await.unwrap(),
            Peer::Bytes(b"status".to_vec())
        );
        while f.endpoint.state().exit_code() != Some(7) {
            tokio::task::yield_now().await;
        }
        assert!(!f.endpoint.state().alive());
        f.endpoint.write(b"probe").unwrap();
        assert_eq!(f.peer.recv().await.unwrap(), Peer::Bytes(b"probe".to_vec()));
        f.endpoint.close().unwrap();
        assert_eq!(
            f.endpoint.wait().await.unwrap(),
            Report {
                stop: Stop::Requested,
                exit_code: Some(7),
                lifecycle: LifecycleDelivery::Cancelled
            }
        );
        assert_eq!(f.peer.recv().await.unwrap(), Peer::Closed);
        assert_eq!(f.events.recv().await.unwrap(), SessionEvent::Bell);
        assert!(f.events.is_empty());
        assert!(Arc::ptr_eq(
            model.reader().poll().unwrap().frame.as_ref().unwrap(),
            &good
        ));
        assert!(matches!(
            model.reader().poll().unwrap().state,
            State::Failed(_)
        ));
    })
    .await;
}

#[tokio::test]
async fn native_owner_close_cancels_final_lifecycle_delivery_explicitly() {
    bounded(async {
        let f = setup().await;
        let model = f.endpoint.model();
        let good = model.reader().poll().unwrap().frame.unwrap();
        final_lifecycle_blocked(&f).await;
        f.endpoint.close().unwrap();
        assert_eq!(
            f.endpoint.wait().await.unwrap(),
            Report {
                stop: Stop::Eof,
                exit_code: None,
                lifecycle: LifecycleDelivery::Cancelled
            }
        );
        assert_eq!(f.events.recv().await.unwrap(), SessionEvent::Bell);
        assert!(f.events.is_empty());
        assert!(matches!(
            model.reader().poll().unwrap().state,
            State::Closed
        ));
        assert!(Arc::ptr_eq(
            model.reader().poll().unwrap().frame.as_ref().unwrap(),
            &good
        ));
    })
    .await;
}

#[tokio::test]
async fn native_owner_lost_final_lifecycle_event_returns_typed_failure() {
    bounded(async {
        let f = setup().await;
        let model = f.endpoint.model();
        final_lifecycle_blocked(&f).await;
        f.events.close();
        assert!(matches!(
            f.endpoint.wait().await,
            Err(Error::Native(AsyncGhosttyPumpError::EventsClosed))
        ));
        assert!(matches!(
            model.reader().poll().unwrap().state,
            State::Closed
        ));
    })
    .await;
}

#[tokio::test]
async fn native_owner_drop_and_cancelled_wait_wake_final_lifecycle_delivery() {
    bounded(async {
        for cancel_wait in [false, true] {
            let f = setup().await;
            final_lifecycle_blocked(&f).await;
            // Observe task completion without owning or aborting its JoinHandle.
            let observer = f.endpoint.task.abort_handle();
            if cancel_wait {
                let mut wait = Box::pin(f.endpoint.wait());
                std::future::poll_fn(|cx| {
                    assert!(wait.as_mut().poll(cx).is_pending());
                    std::task::Poll::Ready(())
                })
                .await;
                drop(wait);
            } else {
                drop(f.endpoint);
            }
            while !observer.is_finished() {
                tokio::task::yield_now().await;
            }
            assert_eq!(f.events.recv().await.unwrap(), SessionEvent::Bell);
            assert!(f.events.is_empty());
        }
    })
    .await;
}

#[tokio::test]
async fn native_owner_exit_signal_clears_cached_status_and_sends_none() {
    bounded(async {
        let f = setup().await;
        let state = f.endpoint.state();
        f.endpoint.write(b"signal").unwrap();
        assert_eq!(
            f.peer.recv().await.unwrap(),
            Peer::Bytes(b"signal".to_vec())
        );
        assert_eq!(
            f.events.recv().await.unwrap(),
            SessionEvent::Exited(Some(7))
        );
        assert_eq!(f.events.recv().await.unwrap(), SessionEvent::Exited(None));
        assert_eq!(f.events.recv().await.unwrap(), SessionEvent::Closed);
        assert!(!state.alive());
        assert_eq!(state.exit_code(), None);
        assert_eq!(
            f.endpoint.wait().await.unwrap(),
            Report {
                stop: Stop::Eof,
                exit_code: None,
                lifecycle: LifecycleDelivery::Complete
            }
        );
    })
    .await;
}
