//! Native routed connection using the existing trust/authentication pipeline.

use std::sync::Arc;

use oneterm_core::{AppError, ConnectPhase, SshConfig, SshKeepaliveConfig, TerminalLogConfig};
use oneterm_terminal::{PtySize, TerminalSecurityPolicy};

use crate::ghostty_channel::{NativeChannel, Services};
use crate::ghostty_session::NativeSshSession;
use crate::handler::SshClientHandler;
use crate::route::{connect_hop, hop_error};
use crate::session::{
    CONNECT_DEADLINE, ConnectPhases, SHELL_INTEGRATION_PTY_MODES, phase_error,
    request_remote_shell_env, send_shell_integration_bootstrap, shared_runtime,
};

fn validate(cfg: &SshConfig, _logging: &TerminalLogConfig) -> oneterm_core::Result<()> {
    let unsupported = if cfg.agent_forwarding {
        Some("agent forwarding")
    } else {
        None
    };
    if let Some(option) = unsupported {
        return Err(AppError::msg(format!(
            "Native direct SSH does not support {option} yet; disable it before connecting"
        )));
    }
    Ok(())
}

/// Blocking explicit factory entry; never constructs a legacy terminal engine.
pub fn connect_native(
    cfg: SshConfig,
    initial: PtySize,
    scrollback: usize,
    security: TerminalSecurityPolicy,
    logging: TerminalLogConfig,
    keepalive: SshKeepaliveConfig,
) -> oneterm_core::Result<Arc<dyn oneterm_terminal::ghostty_session::NativeSession>> {
    validate(&cfg, &logging)?;
    let handler = SshClientHandler::new(cfg.host.clone(), cfg.port, cfg.host_key_policy.clone())
        .with_known_hosts_path(oneterm_core::config_dir().join("known_hosts"));
    connect_with_handler(
        cfg, handler, initial, scrollback, security, keepalive, logging,
    )
}

#[allow(clippy::too_many_arguments)]
fn connect_with_handler(
    mut cfg: SshConfig,
    handler: SshClientHandler,
    initial: PtySize,
    scrollback: usize,
    security: TerminalSecurityPolicy,
    keepalive: SshKeepaliveConfig,
    logging: TerminalLogConfig,
) -> oneterm_core::Result<Arc<dyn oneterm_terminal::ghostty_session::NativeSession>> {
    let runtime = shared_runtime()?;
    let phases = ConnectPhases::new(cfg.cancellation.clone());
    runtime.block_on(async {
        let operation = async {
            let mut services = Services {
                identity: oneterm_terminal::ssh_log_identity(&cfg.username, &cfg.host, cfg.port),
                logging,
                carriers: Default::default(),
                shutdown: Default::default(),
                tasks: Default::default(),
                opens: None,
            };
            let known_hosts = handler
                .known_hosts_path()
                .unwrap_or_else(|| oneterm_core::config_dir().join("known_hosts"));
            for hop in std::mem::take(&mut cfg.jump_hops) {
                let label = hop.label();
                let handler = SshClientHandler::new(hop.host, hop.port, hop.host_key_policy)
                    .with_known_hosts_path(known_hosts.clone());
                let handle = connect_hop(
                    &phases,
                    services.carriers.last(),
                    handler,
                    &hop.username,
                    hop.auth,
                    keepalive,
                )
                .await
                .map_err(|error| hop_error(&label, error))?;
                services.carriers.push(handle);
            }
            let forward_table: crate::tunnel::ForwardTable = Arc::default();
            let handler = handler
                .with_forwards(forward_table.clone(), services.shutdown.clone())
                .with_forward_tasks(services.tasks.clone());
            let auth = std::mem::replace(&mut cfg.auth, oneterm_core::SshAuthMethod::None);
            let handle = connect_hop(
                &phases,
                services.carriers.last(),
                handler,
                &cfg.username,
                auth,
                keepalive,
            )
            .await?;
            let channel = phases
                .run(ConnectPhase::ChannelOpen, async {
                    handle
                        .channel_open_session()
                        .await
                        .map_err(|error| phase_error(ConnectPhase::ChannelOpen, error))
                })
                .await?;
            phases
                .run(ConnectPhase::PtyRequest, async {
                    let modes = if cfg.shell_integration {
                        SHELL_INTEGRATION_PTY_MODES
                    } else {
                        &[]
                    };
                    channel
                        .request_pty(
                            false,
                            "xterm-256color",
                            u32::from(initial.cols),
                            u32::from(initial.rows),
                            0,
                            0,
                            modes,
                        )
                        .await
                        .map_err(|error| phase_error(ConnectPhase::PtyRequest, error))?;
                    request_remote_shell_env(&channel).await
                })
                .await?;
            phases
                .run(ConnectPhase::ShellRequest, async {
                    channel
                        .request_shell(true)
                        .await
                        .map_err(|error| phase_error(ConnectPhase::ShellRequest, error))
                })
                .await?;
            let bootstrap_state = oneterm_terminal::SharedSessionState::new_alive();
            if cfg.shell_integration {
                phases
                    .run(ConnectPhase::ShellIntegration, async {
                        send_shell_integration_bootstrap(&channel, &bootstrap_state).await
                    })
                    .await?;
            }
            if cfg.cancellation.is_cancelled() {
                return Err(AppError::Cancelled);
            }
            let state = oneterm_terminal::SharedSessionState::new_alive();
            let sftp = match phases
                .run(
                    ConnectPhase::SftpSetup,
                    crate::native_sftp::prepare(&handle, &state),
                )
                .await
            {
                Ok(sftp) => Some(sftp),
                Err(AppError::Cancelled) => return Err(AppError::Cancelled),
                Err(error) => {
                    log::warn!("Native SSH SFTP unavailable: {error} — terminal only");
                    None
                }
            };
            if cfg.cancellation.is_cancelled() {
                return Err(AppError::Cancelled);
            }
            let (tx, rx) = async_channel::bounded(4096);
            let (open_tx, open_rx) =
                tokio::sync::mpsc::channel(crate::tunnel::HANDLE_REQUEST_CAPACITY);
            let context = crate::tunnel::ForwardContext {
                tasks: services.tasks.clone(),
                open_tx,
                shutdown: services.shutdown.clone(),
                notify: tx.clone(),
            };
            phases
                .run(ConnectPhase::ChannelOpen, async {
                    crate::tunnel::start_forwards(
                        &cfg.port_forwards,
                        &handle,
                        &forward_table,
                        &context,
                    )
                    .await;
                    Ok(())
                })
                .await?;
            services.opens = Some(open_rx);
            let endpoint = NativeChannel::spawn_with_sftp(
                handle,
                channel,
                initial.cols,
                initial.rows,
                tx,
                security,
                Some(scrollback),
                state,
                sftp,
                services,
            )
            .map_err(|error| phase_error(ConnectPhase::ShellRequest, error))?;
            endpoint
                .state()
                .add_tx_bytes(bootstrap_state.net_stats().tx_bytes);
            Ok(Arc::new(NativeSshSession::new(endpoint, rx))
                as Arc<
                    dyn oneterm_terminal::ghostty_session::NativeSession,
                >)
        };
        tokio::time::timeout(CONNECT_DEADLINE, operation)
            .await
            .unwrap_or_else(|_| {
                Err(AppError::Connect {
                    phase: phases.current(),
                    message: format!(
                        "connection attempt timed out after {} s",
                        CONNECT_DEADLINE.as_secs()
                    ),
                })
            })
    })
}

#[cfg(test)]
#[path = "native_connect_tests.rs"]
mod tests;
