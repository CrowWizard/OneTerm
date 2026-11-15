//! The concrete [`SessionFactory`] implementation.
//!
//! This is the only place that depends on both backend crates (`oneterm-local-shell`
//! + `oneterm-ssh`). It is installed as a process global at startup so the UI
//! feature crates can create sessions without a UI→backend dependency edge.

use std::sync::Arc;

use oneterm_core::{LocalShellConfig, Result, SshConfig, SshKeepaliveConfig, TerminalLogConfig};
use oneterm_terminal::{PtySize, SessionFactory, TerminalSecurityPolicy, TerminalSession};

/// App-owned factory that dispatches to the local + SSH backends.
struct AppSessionFactory {
    #[cfg(feature = "native-desktop")]
    native_only: bool,
}

impl SessionFactory for AppSessionFactory {
    fn create_local(
        &self,
        cfg: LocalShellConfig,
        size: PtySize,
        scrollback: usize,
        security: TerminalSecurityPolicy,
        logging: TerminalLogConfig,
    ) -> Result<oneterm_terminal::CreatedSession> {
        #[cfg(feature = "native-desktop")]
        if self.native_only {
            return self
                .spawn_native_local(cfg, size, scrollback, security, logging)
                .map(oneterm_terminal::CreatedSession::Native);
        }
        self.spawn_local(cfg, size, scrollback, security, logging)
            .map(oneterm_terminal::CreatedSession::Legacy)
    }

    #[cfg(feature = "native-desktop")]
    fn spawn_native_local(
        &self,
        cfg: LocalShellConfig,
        size: PtySize,
        scrollback: usize,
        security: TerminalSecurityPolicy,
        logging: TerminalLogConfig,
    ) -> Result<Arc<dyn oneterm_terminal::ghostty_session::NativeSession>> {
        oneterm_local_shell::NativeLocalSession::spawn_configured(
            cfg, size, scrollback, security, logging,
        )
        .map(|session| {
            Arc::new(session) as Arc<dyn oneterm_terminal::ghostty_session::NativeSession>
        })
    }

    fn spawn_local(
        &self,
        cfg: LocalShellConfig,
        size: PtySize,
        scrollback: usize,
        security: TerminalSecurityPolicy,
        logging: TerminalLogConfig,
    ) -> Result<Box<dyn TerminalSession>> {
        #[cfg(feature = "native-desktop")]
        if self.native_only {
            return Err(oneterm_core::AppError::msg(
                "Legacy local sessions are disabled in the native workspace",
            ));
        }
        oneterm_local_shell::LocalSession::spawn(cfg, size, scrollback, security, logging)
            .map(|s| Box::new(s) as Box<dyn TerminalSession>)
    }

    fn create_ssh(
        &self,
        cfg: SshConfig,
        size: PtySize,
        scrollback: usize,
        security: TerminalSecurityPolicy,
        logging: TerminalLogConfig,
        keepalive: SshKeepaliveConfig,
    ) -> Result<oneterm_terminal::CreatedSession> {
        #[cfg(feature = "native-desktop")]
        if self.native_only {
            return oneterm_ssh::connect_native(
                cfg, size, scrollback, security, logging, keepalive,
            )
            .map(oneterm_terminal::CreatedSession::Native);
        }
        self.connect_ssh(cfg, size, scrollback, security, logging, keepalive)
            .map(oneterm_terminal::CreatedSession::Legacy)
    }

    fn connect_ssh(
        &self,
        cfg: SshConfig,
        size: PtySize,
        scrollback: usize,
        security: TerminalSecurityPolicy,
        logging: TerminalLogConfig,
        keepalive: SshKeepaliveConfig,
    ) -> Result<Box<dyn TerminalSession>> {
        #[cfg(feature = "native-desktop")]
        if self.native_only {
            return Err(oneterm_core::AppError::msg(
                "Legacy SSH sessions are disabled in the native workspace; use tagged session creation",
            ));
        }
        oneterm_ssh::connect(cfg, size, scrollback, security, logging, keepalive)
    }
}

/// Build the app's session factory for the application service bundle.
pub(crate) fn build() -> Arc<dyn SessionFactory> {
    Arc::new(AppSessionFactory {
        #[cfg(feature = "native-desktop")]
        native_only: false,
    })
}

#[cfg(feature = "native-desktop")]
pub(crate) fn build_native() -> Arc<dyn SessionFactory> {
    Arc::new(AppSessionFactory { native_only: true })
}

#[cfg(all(test, feature = "native-desktop", windows))]
mod native_tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn native_ssh_factory_selects_native_preflight_and_refuses_legacy_api() {
        let cfg = SshConfig {
            host: "must-not-resolve.invalid".into(),
            port: 22,
            username: "user".into(),
            auth: oneterm_core::SshAuthMethod::None,
            cancellation: Default::default(),
            host_key_policy: Default::default(),
            shell_integration: false,
            jump_hops: vec![],
            port_forwards: vec![],
            agent_forwarding: true,
        };
        let factory = build_native();
        let error = factory
            .create_ssh(
                cfg.clone(),
                PtySize::INITIAL,
                10,
                Default::default(),
                Default::default(),
                Default::default(),
            )
            .err()
            .unwrap();
        assert!(
            error
                .to_string()
                .contains("Native direct SSH does not support agent forwarding")
        );
        let error = factory
            .connect_ssh(
                cfg,
                PtySize::INITIAL,
                10,
                Default::default(),
                Default::default(),
                Default::default(),
            )
            .err()
            .unwrap();
        assert!(
            error
                .to_string()
                .contains("Legacy SSH sessions are disabled")
        );
    }

    #[test]
    fn native_startup_factory_never_falls_back_to_legacy() {
        let factory = build_native();
        let config = LocalShellConfig {
            kind: oneterm_core::ShellKind::Custom,
            program: Some("missing-native-startup-shell".into()),
            ..Default::default()
        };
        assert!(
            factory
                .spawn_local(
                    config.clone(),
                    PtySize::INITIAL,
                    0,
                    TerminalSecurityPolicy::default(),
                    TerminalLogConfig::default()
                )
                .err()
                .unwrap()
                .to_string()
                .contains("Legacy local sessions are disabled")
        );
        let created = factory
            .create_local(
                config,
                PtySize::INITIAL,
                0,
                TerminalSecurityPolicy::default(),
                TerminalLogConfig::default(),
            )
            .unwrap();
        let oneterm_terminal::CreatedSession::Native(session) = created else {
            panic!("native startup must never return a legacy session");
        };
        let events = session.take_events().unwrap();
        let consumer = std::thread::spawn(move || while events.recv_blocking().is_ok() {});
        session.close().unwrap();
        let report = futures::executor::block_on(session.complete()).unwrap();
        assert!(report.lifecycle_delivered);
        consumer.join().unwrap();
    }

    #[test]
    fn native_factory_explicit_configured_owner_supports_logging() {
        let factory = build();
        let directory = std::env::temp_dir().join(format!(
            "oneterm-factory-logging-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let config = LocalShellConfig {
            kind: oneterm_core::ShellKind::Custom,
            program: Some(
                format!(
                    "{}\\System32\\WindowsPowerShell\\v1.0\\powershell.exe",
                    std::env::var("SystemRoot").unwrap()
                )
                .into(),
            ),
            args: vec![
                "-NoLogo".into(),
                "-NoProfile".into(),
                "-Command".into(),
                "[Console]::Write('NATIVE_FACTORY_READY'); $a=[Console]::ReadLine(); exit 0".into(),
            ],
            cwd: Some(std::env::current_dir().unwrap()),
            ..Default::default()
        };
        let session = factory
            .spawn_native_local(
                config,
                PtySize::INITIAL,
                0,
                TerminalSecurityPolicy::default(),
                TerminalLogConfig {
                    enabled: true,
                    directory: directory.clone(),
                    ..Default::default()
                },
            )
            .unwrap();
        let model = session.render_model();
        let events = session.take_events().unwrap();
        let consumer = std::thread::spawn(move || while events.recv_blocking().is_ok() {});
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            let published = model.reader().poll().unwrap();
            let text = published
                .frame
                .as_ref()
                .map(|frame| {
                    frame
                        .source()
                        .rows
                        .iter()
                        .flat_map(|row| &row.cells)
                        .map(|cell| cell.text.as_str())
                        .collect::<String>()
                })
                .unwrap_or_default();
            if text.contains("NATIVE_FACTORY_READY") {
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(
            model.query().viewport.unwrap().metadata().scrollbar.total,
            24
        );
        session.close().unwrap();
        assert!(session.capabilities().logging.is_some());
        let report = futures::executor::block_on(session.complete()).unwrap();
        assert!(report.lifecycle_delivered);
        consumer.join().unwrap();
        let logs: String = std::fs::read_dir(&directory)
            .unwrap()
            .map(|entry| std::fs::read_to_string(entry.unwrap().path()).unwrap())
            .collect();
        assert!(logs.contains("NATIVE_FACTORY_READY"));
        std::fs::remove_dir_all(directory).unwrap();
    }
}
