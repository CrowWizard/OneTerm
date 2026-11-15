//! Native local lifecycle/configuration and consuming completion adapter.
//! No production factory selector or legacy TerminalSession render shim.

use std::path::PathBuf;
use std::sync::Mutex;

use oneterm_core::{AppError, LocalShellConfig};
use oneterm_terminal::ghostty::input::{KeyEvent, MouseEvent};
use oneterm_terminal::ghostty_model::SharedModel;
use oneterm_terminal::{
    PtySize, SessionEvent, SessionKind, TerminalError, TerminalLifecycle, TerminalSecurityPolicy,
};
use oneterm_vt::pty::WindowSize;

use crate::ghostty_local::{Error, InputError, NativeLocal, Report};
use crate::session::resolved_options;

/// Native session seam. Only lifecycle implements the existing session traits;
/// render readers and controls are explicitly native, with typed failure paths.
pub struct NativeLocalSession {
    endpoint: NativeLocal,
    events: Mutex<Option<async_channel::Receiver<SessionEvent>>>,
    composition: oneterm_terminal::ghostty_ime::Composition,
}

impl NativeLocalSession {
    pub fn spawn(
        cfg: LocalShellConfig,
        initial: PtySize,
        security: TerminalSecurityPolicy,
    ) -> Result<Self, AppError> {
        Self::spawn_inner(cfg, initial, security, None, Default::default())
    }

    /// Preserve workspace configuration and initialize logging on the PTY owner.
    pub fn spawn_configured(
        cfg: LocalShellConfig,
        initial: PtySize,
        scrollback: usize,
        security: TerminalSecurityPolicy,
        logging: oneterm_core::TerminalLogConfig,
    ) -> Result<Self, AppError> {
        Self::spawn_inner(cfg, initial, security, Some(scrollback), logging)
    }

    fn spawn_inner(
        cfg: LocalShellConfig,
        initial: PtySize,
        security: TerminalSecurityPolicy,
        scrollback: Option<usize>,
        logging: oneterm_core::TerminalLogConfig,
    ) -> Result<Self, AppError> {
        let (opts, program) = resolved_options(&cfg)?;
        let (tx, rx) = async_channel::bounded(4096);
        let endpoint = NativeLocal::spawn_with_logging(
            opts,
            WindowSize {
                cols: initial.cols,
                rows: initial.rows,
                ..Default::default()
            },
            tx,
            security,
            scrollback,
            (logging, program.clone()),
        )
        .map_err(|error| AppError::ShellResolution {
            shell: program.display().to_string(),
            reason: error.to_string(),
        })?;
        Ok(Self {
            endpoint,
            events: Mutex::new(Some(rx)),
            composition: Default::default(),
        })
    }

    pub fn model(&self) -> SharedModel {
        self.endpoint.model()
    }

    /// Update full native defaults and acknowledge a complete published frame.
    pub async fn set_default_colors(
        &self,
        colors: oneterm_terminal::ghostty::colors::DefaultColors,
    ) -> Result<(), oneterm_terminal::backend::AsyncGhosttyPumpError> {
        self.endpoint.set_default_colors(colors).await
    }

    /// Queue a native viewport change and repaint. Success means admission, not
    /// publication completion; no subsequent child output is required.
    pub fn scroll(
        &self,
        scroll: oneterm_terminal::ghostty::state::Scroll,
    ) -> Result<(), InputError> {
        self.endpoint.scroll(scroll)
    }

    /// Apply native tracked-anchor selection and acknowledge its publication.
    pub async fn update_selection(
        &self,
        update: oneterm_terminal::ghostty::selection::Update,
    ) -> Result<(), oneterm_terminal::backend::AsyncGhosttyPumpError> {
        self.endpoint.update_selection(update).await
    }

    /// Call after drawing or intentionally skipping this session's publication.
    pub fn release_repaint_hint(&self) {
        self.endpoint.state().release_repaint_hint();
    }

    pub fn write(&self, bytes: &[u8]) -> Result<(), TerminalError> {
        self.endpoint.write(bytes)
    }

    /// Session-local preedit only; callers request redraw themselves.
    pub fn set_marked_text(
        &self,
        text: String,
    ) -> Result<(), oneterm_terminal::ghostty_ime::Error> {
        self.composition.set_marked_text(text)
    }

    pub fn marked_text(&self) -> Option<String> {
        self.composition.marked_text()
    }

    pub fn clear_marked_text(&self) {
        self.composition.clear_marked_text();
    }

    /// Clear preedit and enqueue exact UTF-8, not paste. Success is admission.
    pub fn commit_text(&self, text: &str) -> Result<(), oneterm_terminal::ghostty_ime::Error> {
        self.composition
            .commit_text(text, |bytes| self.write(bytes))
    }
    pub fn key(&self, key: KeyEvent) -> Result<(), InputError> {
        self.endpoint.key(key)
    }

    /// Acknowledges original FIFO admission, not child receipt. Cancellation
    /// after control admission requests independent endpoint shutdown.
    pub async fn paste(
        &self,
        text: &str,
    ) -> Result<(), oneterm_terminal::backend::AsyncGhosttyPumpError> {
        self.endpoint.paste(text).await
    }
    pub fn mouse(&self, mouse: MouseEvent) -> Result<(), InputError> {
        self.endpoint.mouse(mouse)
    }

    /// All metrics travel in one ordered control, not a separate mutable slot.
    pub fn resize(&self, size: WindowSize) -> Result<(), InputError> {
        self.endpoint.resize(size)
    }

    pub async fn wait(self) -> Result<Report, Error> {
        self.endpoint.wait().await
    }
}

impl oneterm_terminal::ghostty_content::RenderSource for NativeLocalSession {
    fn render_model(&self) -> SharedModel {
        self.endpoint.model()
    }
}

impl oneterm_terminal::ghostty_session::SurfaceSession for NativeLocalSession {
    fn clear_command(&self) -> oneterm_terminal::ghostty_session::PasteFuture<'_> {
        Box::pin(self.endpoint.clear_command())
    }
    fn set_default_colors(
        &self,
        colors: oneterm_terminal::ghostty::colors::DefaultColors,
    ) -> oneterm_terminal::ghostty_session::ColorFuture<'_> {
        Box::pin(self.endpoint.set_default_colors(colors))
    }
    fn reveal_search(
        &self,
        navigation: oneterm_terminal::ghostty::search::Navigation,
    ) -> oneterm_terminal::ghostty_session::SearchFuture<'_> {
        Box::pin(
            self.endpoint
                .search_request(oneterm_terminal::ghostty::search::Request::Reveal(
                    navigation,
                )),
        )
    }
    fn search(
        &self,
        needle: String,
        options: oneterm_terminal::ghostty::search::Options,
    ) -> oneterm_terminal::ghostty_session::SearchFuture<'_> {
        Box::pin(
            self.endpoint
                .search_request(oneterm_terminal::ghostty::search::Request::Set(
                    needle, options,
                )),
        )
    }
    fn search_step(&self) -> oneterm_terminal::ghostty_session::SearchFuture<'_> {
        Box::pin(
            self.endpoint
                .search_request(oneterm_terminal::ghostty::search::Request::Step),
        )
    }
    fn scroll_viewport(
        &self,
        scroll: oneterm_terminal::ghostty::state::Scroll,
    ) -> oneterm_terminal::ghostty_session::PasteFuture<'_> {
        Box::pin(self.endpoint.scroll_viewport(scroll))
    }
    fn selection(
        &self,
        update: oneterm_terminal::ghostty::selection::Update,
    ) -> oneterm_terminal::ghostty_session::PasteFuture<'_> {
        Box::pin(self.endpoint.update_selection(update))
    }
    fn paste_visible<'a>(
        &'a self,
        text: &'a str,
    ) -> oneterm_terminal::ghostty_session::PasteFuture<'a> {
        Box::pin(self.endpoint.paste_visible(text))
    }
    fn wheel(
        &self,
        wheel: oneterm_terminal::ghostty::wheel::WheelEvent,
    ) -> Result<(), oneterm_terminal::ghostty_session::InputError> {
        self.endpoint.wheel(wheel).map_err(surface_error)
    }
    fn mouse(
        &self,
        mouse: MouseEvent,
    ) -> Result<(), oneterm_terminal::ghostty_session::InputError> {
        self.mouse(mouse).map_err(surface_error)
    }
    fn resize(
        &self,
        size: oneterm_terminal::ghostty_session::Size,
    ) -> Result<(), oneterm_terminal::ghostty_session::InputError> {
        self.resize(WindowSize {
            cols: size.cols,
            rows: size.rows,
            cell_width: size.cell_width_px,
            cell_height: size.cell_height_px,
        })
        .map_err(surface_error)
    }
    fn key(&self, key: KeyEvent) -> Result<(), oneterm_terminal::ghostty_session::InputError> {
        self.key(key).map_err(surface_error)
    }
    fn key_receipt(
        &self,
        key: KeyEvent,
    ) -> oneterm_terminal::ghostty_session::KeyReceiptFuture<'_> {
        Box::pin(self.endpoint.key_receipt(key))
    }
    fn write_visible<'a>(
        &'a self,
        bytes: &'a [u8],
    ) -> oneterm_terminal::ghostty_session::PasteFuture<'a> {
        Box::pin(self.endpoint.write_visible(bytes))
    }
    fn release_repaint_hint(&self) {
        self.release_repaint_hint();
    }
    fn set_marked_text(&self, text: String) -> Result<(), oneterm_terminal::ghostty_ime::Error> {
        self.set_marked_text(text)
    }
    fn marked_text(&self) -> Option<String> {
        self.marked_text()
    }
    fn clear_marked_text(&self) {
        self.clear_marked_text();
    }
    fn commit_text(&self, text: &str) -> Result<(), oneterm_terminal::ghostty_ime::Error> {
        self.commit_text(text)
    }
}

fn surface_error(error: InputError) -> oneterm_terminal::ghostty_session::InputError {
    match error {
        InputError::Invalid(error) => oneterm_terminal::ghostty_session::InputError::Invalid(error),
        InputError::Admission(error) => {
            oneterm_terminal::ghostty_session::InputError::Admission(error)
        }
    }
}

impl TerminalLifecycle for NativeLocalSession {
    fn take_events(&self) -> Option<async_channel::Receiver<SessionEvent>> {
        self.events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
    }
    fn alive(&self) -> bool {
        self.endpoint.state().alive()
    }
    fn close(&self) -> Result<(), TerminalError> {
        self.endpoint.close()
    }
    fn kind(&self) -> SessionKind {
        SessionKind::Local
    }
    fn title(&self) -> Option<String> {
        self.endpoint.state().title()
    }
    fn cwd(&self) -> Option<PathBuf> {
        self.endpoint.state().cwd()
    }
}

impl oneterm_terminal::ghostty_session::NativeSession for NativeLocalSession {
    fn capabilities(&self) -> oneterm_terminal::TerminalCapabilities {
        oneterm_terminal::TerminalCapabilities {
            logging: Some(self.endpoint.logging()),
            ..Default::default()
        }
    }
    fn request_shutdown(&self) -> Result<(), TerminalError> {
        self.endpoint.request_shutdown()
    }

    fn reply_clipboard(&self, text: &str) -> Result<(), TerminalError> {
        self.write(&oneterm_terminal::ghostty_session::clipboard_reply(text)?)
    }

    fn complete(self: std::sync::Arc<Self>) -> oneterm_terminal::ghostty_session::CompletionFuture {
        Box::pin(async move {
            let session = std::sync::Arc::try_unwrap(self).map_err(|session| {
                if let Err(error) = session.close() {
                    log::error!("Native local retained completion close: {error}");
                }
                TerminalError::Transport(format!(
                    "Native local completion requires released controls; {} references remain",
                    std::sync::Arc::strong_count(&session),
                ))
            })?;
            let report = session
                .wait()
                .await
                .map_err(|error| TerminalError::Transport(error.to_string()))?;
            Ok(oneterm_terminal::ghostty_session::Completion {
                exit_code: report.exit_code,
                lifecycle_delivered: report.lifecycle
                    == crate::ghostty_local::LifecycleDelivery::Complete,
            })
        })
    }
}

#[cfg(all(test, windows))]
#[path = "ghostty_session_tests.rs"]
mod tests;
