//! Direct native application connection proof uses temporary loopback trust.

use super::*;
use crate::test_support::{TempKnownHosts, loopback_fingerprint, spawn_server};
use oneterm_core::{HostKeyPolicy, SecretString, SshAuthMethod};
use oneterm_terminal::{SessionEvent, SessionKind};
use russh::server::{Auth, Handler, Msg, Server, Session};
use russh::{Channel, ChannelId, Pty};
use std::net::SocketAddr;
use std::time::Duration;

#[derive(Debug, PartialEq, Eq)]
enum Peer {
    SftpPending,
    Pty(u32, u32, bool),
    Env(String, String),
    Bytes(Vec<u8>),
    Resize(u32, u32),
    Closed,
}

struct Probe {
    server_handle: Arc<std::sync::Mutex<Option<russh::server::Handle>>>,
    peer: async_channel::Sender<Peer>,
    sftp_ids: std::collections::HashSet<ChannelId>,
    channels: std::collections::HashMap<ChannelId, Channel<Msg>>,
    mode: SftpMode,
    authentications: Arc<std::sync::atomic::AtomicUsize>,
}

#[derive(Clone)]
enum SftpMode {
    Unsupported,
    Files(std::path::PathBuf),
    Hold,
}
impl Server for Probe {
    type Handler = Self;
    fn new_client(&mut self, _: Option<SocketAddr>) -> Self {
        Self {
            server_handle: self.server_handle.clone(),
            peer: self.peer.clone(),
            mode: self.mode.clone(),
            authentications: self.authentications.clone(),
            sftp_ids: Default::default(),
            channels: Default::default(),
        }
    }
}
impl Handler for Probe {
    type Error = russh::Error;
    async fn channel_open_direct_tcpip(
        &mut self,
        channel: Channel<Msg>,
        host: &str,
        port: u32,
        _: &str,
        _: u32,
        reply: russh::server::ChannelOpenHandle,
        _: &mut Session,
    ) -> Result<(), Self::Error> {
        let mut tcp = tokio::net::TcpStream::connect((host, port as u16)).await?;
        reply.accept().await;
        tokio::spawn(async move {
            let mut remote = channel.into_stream();
            if let Err(error) = tokio::io::copy_bidirectional(&mut remote, &mut tcp).await {
                log::debug!("Native fixture relay ended: {error}");
            }
        });
        Ok(())
    }
    async fn tcpip_forward(
        &mut self,
        _: &str,
        port: &mut u32,
        session: &mut Session,
    ) -> Result<bool, Self::Error> {
        if *port == 0 {
            *port = 40001;
        }
        *self.server_handle.lock().unwrap() = Some(session.handle());
        Ok(true)
    }
    async fn subsystem_request(
        &mut self,
        channel: ChannelId,
        _: &str,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.sftp_ids.insert(channel);
        match &self.mode {
            SftpMode::Unsupported => {
                session.channel_failure(channel)?;
                session.close(channel)?;
            }
            SftpMode::Files(root) => {
                session.channel_success(channel)?;
                let stream = self.channels.remove(&channel).unwrap().into_stream();
                crate::sftp_task::handle_limit_tests::serve_fixture(stream, root).await;
            }
            SftpMode::Hold => {
                session.channel_success(channel)?;
                self.peer.send(Peer::SftpPending).await.unwrap();
            }
        }
        Ok(())
    }
    async fn auth_password(&mut self, _: &str, password: &str) -> Result<Auth, Self::Error> {
        self.authentications
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if password == "hold" {
            self.peer
                .send(Peer::Bytes(b"auth-pending".to_vec()))
                .await
                .unwrap();
            std::future::pending::<()>().await;
        }
        Ok(if password == "secret" {
            Auth::Accept
        } else {
            Auth::reject()
        })
    }
    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        reply: russh::server::ChannelOpenHandle,
        _: &mut Session,
    ) -> Result<(), Self::Error> {
        self.channels.insert(channel.id(), channel);
        reply.accept().await;
        Ok(())
    }
    async fn pty_request(
        &mut self,
        _: ChannelId,
        term: &str,
        cols: u32,
        rows: u32,
        _: u32,
        _: u32,
        modes: &[(Pty, u32)],
        _: &mut Session,
    ) -> Result<(), Self::Error> {
        assert_eq!(term, "xterm-256color");
        self.peer
            .send(Peer::Pty(cols, rows, modes.contains(&(Pty::ECHO, 0))))
            .await
            .unwrap();
        Ok(())
    }
    async fn env_request(
        &mut self,
        _: ChannelId,
        name: &str,
        value: &str,
        _: &mut Session,
    ) -> Result<(), Self::Error> {
        self.peer
            .send(Peer::Env(name.into(), value.into()))
            .await
            .unwrap();
        Ok(())
    }
    async fn shell_request(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.channel_success(channel)?;
        session.data(
            channel,
            b"ready\x1b]2;native-ssh\x07\x1b]7;file://loopback/srv/native\x07".to_vec(),
        )?;
        Ok(())
    }
    async fn data(
        &mut self,
        channel: ChannelId,
        bytes: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if self.sftp_ids.contains(&channel) {
            return Ok(());
        }
        self.peer.send(Peer::Bytes(bytes.to_vec())).await.unwrap();
        if bytes == b"query" {
            session.data(
                channel,
                b"\x1b[6n\x1b]52;c;aGk=\x07\x1b]52;c;?\x07\x1b]2;policy-done\x07".to_vec(),
            )?;
        } else if bytes == b"exit" {
            session.exit_status_request(channel, 7)?;
            session.eof(channel)?;
            session.close(channel)?;
        }
        Ok(())
    }
    async fn window_change_request(
        &mut self,
        _: ChannelId,
        cols: u32,
        rows: u32,
        _: u32,
        _: u32,
        _: &mut Session,
    ) -> Result<(), Self::Error> {
        self.peer.send(Peer::Resize(cols, rows)).await.unwrap();
        Ok(())
    }
    async fn channel_close(
        &mut self,
        channel: ChannelId,
        _: &mut Session,
    ) -> Result<(), Self::Error> {
        self.channels.remove(&channel);
        if self.sftp_ids.contains(&channel) {
            return Ok(());
        }
        self.peer.send(Peer::Closed).await.unwrap();
        Ok(())
    }
}
struct Fixture {
    server_handle: Arc<std::sync::Mutex<Option<russh::server::Handle>>>,
    authentications: Arc<std::sync::atomic::AtomicUsize>,
    address: SocketAddr,
    server: tokio::task::JoinHandle<()>,
    trust: TempKnownHosts,
    peer: async_channel::Receiver<Peer>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}
impl Fixture {
    async fn new() -> Self {
        Self::with_sftp(SftpMode::Unsupported).await
    }
    async fn with_sftp(mode: SftpMode) -> Self {
        let (tx, peer) = async_channel::bounded(128);
        let authentications = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let server_handle = Arc::default();
        let (address, server) = spawn_server(Probe {
            server_handle: Arc::clone(&server_handle),
            peer: tx,
            sftp_ids: Default::default(),
            channels: Default::default(),
            mode,
            authentications: authentications.clone(),
        })
        .await;
        let trust = TempKnownHosts(std::env::temp_dir().join(format!(
            "native-connect-{}-{}",
            std::process::id(),
            address.port()
        )));
        Self {
            server_handle,
            authentications,
            address,
            server,
            trust,
            peer,
        }
    }
    fn cfg(&self, policy: HostKeyPolicy, password: &str) -> SshConfig {
        SshConfig {
            host: self.address.ip().to_string(),
            port: self.address.port(),
            username: "user".into(),
            auth: SshAuthMethod::Password {
                password: SecretString::new(password),
            },
            cancellation: Default::default(),
            host_key_policy: policy,
            shell_integration: false,
            jump_hops: vec![],
            port_forwards: vec![],
            agent_forwarding: false,
        }
    }
    async fn approved(&self) -> SshConfig {
        self.cfg(
            HostKeyPolicy::AcceptNewFingerprint(
                loopback_fingerprint(self.address, &self.trust).await,
            ),
            "secret",
        )
    }
    async fn connect(
        &self,
        cfg: SshConfig,
        security: TerminalSecurityPolicy,
    ) -> oneterm_core::Result<Arc<dyn oneterm_terminal::ghostty_session::NativeSession>> {
        self.connect_logging(cfg, security, Default::default())
            .await
    }
    async fn connect_logging(
        &self,
        cfg: SshConfig,
        security: TerminalSecurityPolicy,
        logging: TerminalLogConfig,
    ) -> oneterm_core::Result<Arc<dyn oneterm_terminal::ghostty_session::NativeSession>> {
        let handler =
            SshClientHandler::new(cfg.host.clone(), cfg.port, cfg.host_key_policy.clone())
                .with_known_hosts_path(self.trust.0.clone());
        tokio::task::spawn_blocking(move || {
            connect_with_handler(
                cfg,
                handler,
                PtySize { rows: 3, cols: 16 },
                0,
                security,
                Default::default(),
                logging,
            )
        })
        .await
        .unwrap()
    }
}
async fn bounded<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(15), future)
        .await
        .expect("native connection proof deadline")
}
async fn until_title(
    events: &async_channel::Receiver<SessionEvent>,
    title: &str,
) -> Vec<SessionEvent> {
    let mut seen = vec![];
    loop {
        let event = bounded(events.recv()).await.unwrap();
        let done = event == SessionEvent::Title(title.into());
        seen.push(event);
        if done {
            return seen;
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_jump_route_forwards_and_logging_complete_together() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    bounded(async {
        let hop = Fixture::new().await;
        let second_hop = Fixture::new().await;
        let root = crate::sftp_task::handle_limit_tests::scratch("native-routed-sftp");
        std::fs::write(root.0.join("routed.txt"), b"same target connection").unwrap();
        let target = Fixture::with_sftp(SftpMode::Files(root.0.clone())).await;
        let echo = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let echo_address = echo.local_addr().unwrap();
        let echo_task = tokio::spawn(async move {
            while let Ok((stream, _)) = echo.accept().await {
                tokio::spawn(async move {
                    let (mut read, mut write) = stream.into_split();
                    if let Err(error) = tokio::io::copy(&mut read, &mut write).await {
                        log::debug!("Native echo fixture ended: {error}");
                    }
                });
            }
        });
        let reserve_local = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let local = reserve_local.local_addr().unwrap();
        let reserve_dynamic = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let dynamic = reserve_dynamic.local_addr().unwrap();
        let occupied = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut cfg = target.approved().await;
        cfg.jump_hops.push(oneterm_core::SshHop {
            host: hop.address.ip().to_string(), port: hop.address.port(), username: "hop-user".into(),
            auth: SshAuthMethod::Password { password: SecretString::new("secret") },
            host_key_policy: HostKeyPolicy::AcceptNewFingerprint(loopback_fingerprint(hop.address, &target.trust).await),
        });
        cfg.jump_hops.push(oneterm_core::SshHop {
            host: second_hop.address.ip().to_string(), port: second_hop.address.port(), username: "second-hop-user".into(),
            auth: SshAuthMethod::Password { password: SecretString::new("secret") },
            host_key_policy: HostKeyPolicy::AcceptNewFingerprint(loopback_fingerprint(second_hop.address, &target.trust).await),
        });
        cfg.port_forwards = vec![
            oneterm_core::PortForward::Local { bind: local.ip(), bind_port: local.port(), target_host: echo_address.ip().to_string(), target_port: echo_address.port() },
            oneterm_core::PortForward::Dynamic { bind: dynamic.ip(), bind_port: dynamic.port() },
            oneterm_core::PortForward::Remote { bind_host: "127.0.0.1".into(), bind_port: 40001, target_host: echo_address.ip().to_string(), target_port: echo_address.port() },
            oneterm_core::PortForward::Dynamic { bind: occupied.local_addr().unwrap().ip(), bind_port: occupied.local_addr().unwrap().port() },
        ];
        drop(reserve_local);
        drop(reserve_dynamic);
        let directory = std::env::temp_dir().join(format!("native-ssh-log-{}-{}", std::process::id(), target.address.port()));
        let session = target.connect_logging(cfg, Default::default(), TerminalLogConfig { enabled: true, directory: directory.clone(), ..Default::default() }).await.unwrap();
        let logging = session.capabilities().logging.unwrap();
        let sftp = session.capabilities().sftp.unwrap();
        assert!(sftp.stat(oneterm_core::RemotePath::new("/routed.txt")).await.is_ok());
        assert_eq!(target.authentications.load(std::sync::atomic::Ordering::Relaxed), 1);
        let oneterm_terminal::TerminalLogState::Running { path } = logging.state() else { panic!("automatic logging missing") };
        let events = session.take_events().unwrap();
        let initial = until_title(&events, "native-ssh").await;
        assert!(initial.iter().any(|event| matches!(event, SessionEvent::Notification(message) if message.contains("not started"))));
        let consumer = tokio::spawn(async move { while events.recv().await.is_ok() {} });
        let mut local_stream = tokio::net::TcpStream::connect(local).await.unwrap();
        local_stream.write_all(b"local").await.unwrap();
        let mut back = [0; 5];
        local_stream.read_exact(&mut back).await.unwrap();
        assert_eq!(&back, b"local");
        let mut socks = tokio::net::TcpStream::connect(dynamic).await.unwrap();
        socks.write_all(&[5, 1, 0]).await.unwrap();
        let mut greeting = [0; 2];
        socks.read_exact(&mut greeting).await.unwrap();
        assert_eq!(greeting, [5, 0]);
        let mut request = vec![5, 1, 0, 1, 127, 0, 0, 1];
        request.extend_from_slice(&echo_address.port().to_be_bytes());
        socks.write_all(&request).await.unwrap();
        let mut response = [0; 10];
        socks.read_exact(&mut response).await.unwrap();
        assert_eq!(response[1], 0);
        socks.write_all(b"socks").await.unwrap();
        socks.read_exact(&mut back).await.unwrap();
        assert_eq!(&back, b"socks");
        let handle = target.server_handle.lock().unwrap().clone().unwrap();
        let mut remote = handle.channel_open_forwarded_tcpip("127.0.0.1", 40001, "127.0.0.1", 12345).await.unwrap().into_stream();
        remote.write_all(b"remote").await.unwrap();
        let mut remote_back = [0; 6];
        remote.read_exact(&mut remote_back).await.unwrap();
        assert_eq!(&remote_back, b"remote");
        // An unfinished SOCKS handshake must also be cancelled and observed.
        let mut stalled = tokio::net::TcpStream::connect(dynamic).await.unwrap();
        stalled.write_all(&[5]).await.unwrap();
        session.request_shutdown().unwrap();
        assert!(session.complete().await.unwrap().lifecycle_delivered);
        consumer.await.unwrap();
        assert!(!sftp.alive());
        assert_eq!(logging.state(), oneterm_terminal::TerminalLogState::Stopped);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("ready"));
        assert!(!text.contains("native-ssh"));
        assert!(!text.contains('\x1b'));
        assert!(tokio::net::TcpListener::bind(local).await.is_ok());
        assert!(tokio::net::TcpListener::bind(dynamic).await.is_ok());
        let mut byte = [0];
        assert!(matches!(stalled.read(&mut byte).await, Ok(0) | Err(_)));
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(directory).unwrap();
        echo_task.abort();
    }).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_jump_trust_auth_and_cancellation_are_per_hop() {
    bounded(async {
        let hop = Fixture::new().await;
        let target = Fixture::new().await;
        let mut cfg = target.cfg(HostKeyPolicy::Strict, "secret");
        cfg.jump_hops.push(oneterm_core::SshHop {
            host: hop.address.ip().to_string(), port: hop.address.port(), username: "hop-user".into(),
            auth: SshAuthMethod::Password { password: SecretString::new("secret") },
            host_key_policy: HostKeyPolicy::Strict,
        });
        let error = target.connect(cfg.clone(), Default::default()).await.err().unwrap();
        let fingerprint = match error {
            AppError::HostKeyUnknown { port, fingerprint, .. } => { assert_eq!(port, hop.address.port()); fingerprint }
            other => panic!("{other}"),
        };
        assert!(!target.trust.0.exists());
        cfg.jump_hops[0].host_key_policy = HostKeyPolicy::AcceptNewFingerprint(fingerprint);
        assert!(matches!(target.connect(cfg.clone(), Default::default()).await, Err(AppError::HostKeyUnknown { port, .. }) if port == target.address.port()));
        assert_eq!(target.authentications.load(std::sync::atomic::Ordering::Relaxed), 0);
        let trust = std::fs::read_to_string(&target.trust.0).unwrap();
        assert!(trust.contains(&format!("[{}]:{}", hop.address.ip(), hop.address.port())));
        assert!(!trust.contains(&format!("[{}]:{}", target.address.ip(), target.address.port())));
        cfg.jump_hops[0].host_key_policy = HostKeyPolicy::Strict;
        cfg.jump_hops[0].auth = SshAuthMethod::Password { password: SecretString::new("wrong") };
        assert!(matches!(target.connect(cfg.clone(), Default::default()).await, Err(AppError::Connect { phase: ConnectPhase::Authentication, .. })));
        cfg.jump_hops[0].auth = SshAuthMethod::Password { password: SecretString::new("hold") };
        let cancellation = cfg.cancellation.clone();
        let connect = target.connect(cfg, Default::default());
        let cancel = async {
            while hop.peer.recv().await.unwrap() != Peer::Bytes(b"auth-pending".to_vec()) {}
            cancellation.cancel();
        };
        let (result, ()) = tokio::join!(connect, cancel);
        assert!(matches!(result, Err(AppError::Cancelled)));
        assert_eq!(target.authentications.load(std::sync::atomic::Ordering::Relaxed), 0);
    }).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn strict_trust_approval_changed_key_auth_and_cancellation() {
    bounded(async {
        let f = Fixture::new().await;
        let error = f
            .connect(f.cfg(HostKeyPolicy::Strict, "secret"), Default::default())
            .await
            .err()
            .unwrap();
        let fingerprint = match error {
            AppError::HostKeyUnknown { fingerprint, .. } => fingerprint,
            other => panic!("{other}"),
        };
        assert!(!f.trust.0.exists());
        let cfg = f.cfg(
            HostKeyPolicy::AcceptNewFingerprint(fingerprint.clone()),
            "wrong",
        );
        assert!(matches!(
            f.connect(cfg, Default::default()).await,
            Err(AppError::Connect {
                phase: ConnectPhase::Authentication,
                ..
            })
        ));
        assert!(f.trust.0.exists());
        let cancelled = f.cfg(HostKeyPolicy::Strict, "secret");
        cancelled.cancellation.cancel();
        assert!(matches!(
            f.connect(cancelled, Default::default()).await,
            Err(AppError::Cancelled)
        ));
        let held = f.cfg(HostKeyPolicy::Strict, "hold");
        let cancellation = held.cancellation.clone();
        let connect = f.connect(held, Default::default());
        let cancel = async {
            assert_eq!(
                f.peer.recv().await.unwrap(),
                Peer::Bytes(b"auth-pending".to_vec())
            );
            cancellation.cancel();
        };
        let (result, ()) = tokio::join!(connect, cancel);
        assert!(matches!(result, Err(AppError::Cancelled)));
        let changed = Fixture::new().await;
        let line = std::fs::read_to_string(&f.trust.0).unwrap();
        let old = format!("[{}]:{}", f.address.ip(), f.address.port());
        let new = format!("[{}]:{}", changed.address.ip(), changed.address.port());
        std::fs::write(&changed.trust.0, line.replace(&old, &new)).unwrap();
        assert!(matches!(
            changed
                .connect(
                    changed.cfg(HostKeyPolicy::AcceptNewFingerprint(fingerprint), "secret"),
                    Default::default()
                )
                .await,
            Err(AppError::HostKeyChanged { .. })
        ));
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn configured_native_surface_io_policy_and_remote_exit() {
    bounded(async {
        let f = Fixture::new().await;
        let session = f
            .connect(f.approved().await, Default::default())
            .await
            .unwrap();
        assert_eq!(session.kind(), SessionKind::Ssh);
        let events = session.take_events().unwrap();
        assert!(session.take_events().is_none());
        until_title(&events, "native-ssh").await;
        while session.cwd().is_none() {
            tokio::task::yield_now().await;
        }
        assert_eq!(
            session.cwd().unwrap(),
            std::path::PathBuf::from("/srv/native")
        );
        let model = session.render_model();
        let mut reader = model.reader();
        let frame = loop {
            if let Some(frame) = reader.poll().unwrap().frame {
                break frame;
            }
            tokio::task::yield_now().await;
        };
        let text: String = frame
            .source()
            .rows
            .iter()
            .flat_map(|r| &r.cells)
            .map(|c| c.text.as_str())
            .collect();
        assert!(text.contains("ready"), "{text}");
        assert_eq!(
            model.query().viewport.unwrap().metadata().scrollbar.total,
            3
        );
        assert_eq!(f.peer.recv().await.unwrap(), Peer::Pty(16, 3, false));
        let mut env = vec![];
        while env.len() < 3 {
            if let Peer::Env(name, value) = f.peer.recv().await.unwrap() {
                env.push((name, value));
            }
        }
        assert!(env.contains(&("COLORTERM".into(), "truecolor".into())));
        session
            .resize(oneterm_terminal::ghostty_session::Size {
                cols: 20,
                rows: 4,
                cell_width_px: 8,
                cell_height_px: 16,
            })
            .unwrap();
        assert_eq!(f.peer.recv().await.unwrap(), Peer::Resize(20, 4));
        session.commit_text("query").unwrap();
        let seen = until_title(&events, "policy-done").await;
        assert!(
            !seen
                .iter()
                .any(|e| matches!(e, SessionEvent::Clipboard(_) | SessionEvent::ClipboardRead))
        );
        assert_eq!(f.peer.recv().await.unwrap(), Peer::Bytes(b"query".to_vec()));
        let Peer::Bytes(reply) = f.peer.recv().await.unwrap() else {
            panic!("cursor reply")
        };
        assert!(
            reply.starts_with(b"\x1b[") && reply.ends_with(b"R"),
            "{reply:?}"
        );
        session.commit_text("exit").unwrap();
        let consumer = tokio::spawn(async move {
            let mut seen = vec![];
            while let Ok(e) = events.recv().await {
                seen.push(e);
            }
            seen
        });
        let report = session.complete().await.unwrap();
        assert_eq!(report.exit_code, Some(7));
        assert!(report.lifecycle_delivered);
        let seen = consumer.await.unwrap();
        assert!(seen.contains(&SessionEvent::Exited(Some(7))));
        assert_eq!(seen.last(), Some(&SessionEvent::Closed));
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bootstrap_enabled_policy_orderly_close_and_drop_observe_teardown() {
    bounded(async {
        let f = Fixture::new().await;
        let mut cfg = f.approved().await;
        cfg.shell_integration = true;
        let session = f
            .connect(
                cfg,
                TerminalSecurityPolicy {
                    allow_remote_clipboard_read: true,
                    allow_remote_clipboard_write: true,
                },
            )
            .await
            .unwrap();
        let events = session.take_events().unwrap();
        until_title(&events, "native-ssh").await;
        assert_eq!(f.peer.recv().await.unwrap(), Peer::Pty(16, 3, true));
        let bootstrap = loop {
            if let Peer::Bytes(bytes) = f.peer.recv().await.unwrap() {
                break bytes;
            }
        };
        assert!(bootstrap.starts_with(b"export COLORTERM=truecolor"));
        session.commit_text("query").unwrap();
        let seen = until_title(&events, "policy-done").await;
        let clipboard: Vec<_> = seen
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    SessionEvent::Clipboard(_) | SessionEvent::ClipboardRead
                )
            })
            .cloned()
            .collect();
        assert_eq!(
            clipboard,
            [
                SessionEvent::Clipboard(Some("hi".into())),
                SessionEvent::ClipboardRead
            ]
        );
        assert_eq!(f.peer.recv().await.unwrap(), Peer::Bytes(b"query".to_vec()));
        let Peer::Bytes(cursor) = f.peer.recv().await.unwrap() else {
            panic!("cursor response")
        };
        assert!(cursor.starts_with(b"\x1b[") && cursor.ends_with(b"R"));
        assert!(
            f.peer.try_recv().is_err(),
            "no premature clipboard response"
        );
        session.reply_clipboard("reply").unwrap();
        assert_eq!(
            f.peer.recv().await.unwrap(),
            Peer::Bytes(b"\x1b]52;c;cmVwbHk=\x07".to_vec())
        );
        session.request_shutdown().unwrap();
        assert!(session.commit_text("sealed").is_err());
        let consumer = tokio::spawn(async move { while events.recv().await.is_ok() {} });
        assert!(session.complete().await.unwrap().lifecycle_delivered);
        consumer.await.unwrap();
        let second = f
            .connect(f.cfg(HostKeyPolicy::Strict, "secret"), Default::default())
            .await
            .unwrap();
        let events = second.take_events().unwrap();
        drop(second);
        while events.recv().await.is_ok() {}
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_sftp_same_login_metadata_transfer_and_retained_backend_close() {
    bounded(async {
        use oneterm_core::RemotePath;
        let root = crate::sftp_task::handle_limit_tests::scratch("native-remote");
        let local = crate::sftp_task::handle_limit_tests::scratch("native-local");
        std::fs::write(root.0.join("hello.txt"), b"native sftp bytes").unwrap();
        let f = Fixture::with_sftp(SftpMode::Files(root.0.clone())).await;
        let session = f
            .connect(f.approved().await, Default::default())
            .await
            .unwrap();
        assert_eq!(
            f.authentications.load(std::sync::atomic::Ordering::Relaxed),
            1
        );
        let events = session.take_events().unwrap();
        until_title(&events, "native-ssh").await;
        let capabilities = session.capabilities();
        let backend = capabilities.sftp.unwrap();
        assert!(backend.alive());
        let entries = backend.read_dir(RemotePath::new("/")).await.unwrap();
        assert_eq!(
            backend.realpath(RemotePath::new(".")).await.unwrap(),
            RemotePath::new("/")
        );
        assert!(entries.iter().any(|entry| entry.name == "hello.txt"));
        assert!(backend.stat(RemotePath::new("/hello.txt")).await.is_ok());
        let downloaded = local.0.join("download.txt");
        let transfer = backend.download(71, RemotePath::new("/hello.txt"), downloaded.clone());
        transfer.result.recv().await.unwrap().unwrap();
        assert_eq!(std::fs::read(downloaded).unwrap(), b"native sftp bytes");
        let uploaded = local.0.join("upload.txt");
        std::fs::write(&uploaded, b"upload native bytes").unwrap();
        let transfer = backend.upload(72, uploaded, RemotePath::new("/uploaded.txt"));
        transfer.result.recv().await.unwrap().unwrap();
        assert_eq!(
            std::fs::read(root.0.join("uploaded.txt")).unwrap(),
            b"upload native bytes"
        );
        let state = capabilities.cwd_source.unwrap();
        assert_eq!(state.cwd(), Some(std::path::PathBuf::from("/srv/native")));
        assert!(session.capabilities().network_stats.unwrap().rx_bytes > 0);
        session.request_shutdown().unwrap();
        let consumer = tokio::spawn(async move { while events.recv().await.is_ok() {} });
        assert!(session.complete().await.unwrap().lifecycle_delivered);
        consumer.await.unwrap();
        assert!(
            !backend.alive(),
            "retained browser Arc cannot retain SFTP task"
        );
        assert!(backend.stat(RemotePath::new("/hello.txt")).await.is_err());
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_sftp_setup_cancellation_leaves_no_native_session() {
    bounded(async {
        let f = Fixture::with_sftp(SftpMode::Hold).await;
        let cfg = f.approved().await;
        let cancellation = cfg.cancellation.clone();
        let cancel = async {
            loop {
                if f.peer.recv().await.unwrap() == Peer::SftpPending {
                    break;
                }
            }
            cancellation.cancel();
        };
        let (result, ()) = tokio::join!(f.connect(cfg, Default::default()), cancel);
        assert!(matches!(result, Err(AppError::Cancelled)));
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_sftp_remote_exit_and_drop_close_retained_browser_backend() {
    bounded(async {
        let root = crate::sftp_task::handle_limit_tests::scratch("native-exit-drop");
        for remote_exit in [true, false] {
            let f = Fixture::with_sftp(SftpMode::Files(root.0.clone())).await;
            let session = f
                .connect(f.approved().await, Default::default())
                .await
                .unwrap();
            let retained = session.capabilities().sftp.unwrap();
            let events = session.take_events().unwrap();
            until_title(&events, "native-ssh").await;
            let consumer = tokio::spawn(async move { while events.recv().await.is_ok() {} });
            if remote_exit {
                session.commit_text("exit").unwrap();
                assert_eq!(session.complete().await.unwrap().exit_code, Some(7));
            } else {
                drop(session);
            }
            consumer.await.unwrap();
            // Channel event closure may precede SFTP observation on Drop; a
            // command response is an explicit admission/liveness observation.
            while retained.alive() {
                tokio::task::yield_now().await;
            }
            assert!(
                retained
                    .realpath(oneterm_core::RemotePath::new("."))
                    .await
                    .is_err()
            );
        }
    })
    .await;
}

#[test]
fn agent_forwarding_rejects_but_native_services_pass_preflight() {
    let mut cfg = SshConfig {
        host: "must-not-resolve.invalid".into(),
        port: 22,
        username: "test".into(),
        auth: oneterm_core::SshAuthMethod::None,
        cancellation: Default::default(),
        host_key_policy: Default::default(),
        shell_integration: false,
        jump_hops: vec![],
        port_forwards: vec![],
        agent_forwarding: true,
    };
    let error = connect_native(
        cfg.clone(),
        PtySize::INITIAL,
        20,
        Default::default(),
        Default::default(),
        Default::default(),
    )
    .err()
    .expect("preflight rejection");
    assert!(error.to_string().contains("agent forwarding"));
    cfg.agent_forwarding = false;
    cfg.jump_hops.push(oneterm_core::SshHop {
        host: "must-not-resolve.invalid".into(),
        port: 22,
        username: "user".into(),
        auth: SshAuthMethod::None,
        host_key_policy: Default::default(),
    });
    assert!(validate(&cfg, &Default::default()).is_ok());
    cfg.jump_hops.clear();
    cfg.port_forwards.push(oneterm_core::PortForward::Dynamic {
        bind: std::net::Ipv4Addr::LOCALHOST.into(),
        bind_port: 1080,
    });
    assert!(validate(&cfg, &Default::default()).is_ok());
    cfg.port_forwards.clear();
    let logging = TerminalLogConfig {
        enabled: true,
        ..Default::default()
    };
    assert!(validate(&cfg, &logging).is_ok());
    assert!(validate(&cfg, &Default::default()).is_ok());
}
