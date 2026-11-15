//! Native pane lifetime, independent reliable-event draining and close proof.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::terminal_view::{OverlayColors, bell_badge, progress_overlay};
use gpui::{
    App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable, Font, IntoElement,
    ParentElement as _, Pixels, Render, Styled as _, Subscription, Window, div,
};
use gpui_component::{ActiveTheme as _, WindowExt as _, notification::NotificationType};
use oneterm_terminal::ghostty_session::{Completion, NativeSession};
use oneterm_terminal::security_policy::MAX_QUEUED_NOTIFICATIONS;
use oneterm_terminal::{ProgramState, SessionEvent, TerminalError};
use oneterm_theme::notif_ext::notify;

use crate::{NativeTerminalSurface, terminal_view::TerminalViewEvent};

const CLOSE_BUDGET: Duration = Duration::from_secs(5);
const REFRESH_INTERVAL: Duration = Duration::from_millis(16);

#[path = "native_pane_channels.rs"]
mod channels;
#[path = "native_pane_menu.rs"]
mod menu;

/// Close admission is not completion. The observation survives its pane entity.
#[derive(Clone)]
pub struct NativePaneCompletion(Arc<Mutex<CloseState>>);

#[derive(Clone, Debug, PartialEq, Eq)]
enum CloseState {
    Running,
    Closing,
    Finished(Result<Completion, TerminalError>),
}

impl NativePaneCompletion {
    /// None means completion has not been observed, not successful teardown.
    pub fn result(&self) -> Option<Result<Completion, TerminalError>> {
        match &*self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
        {
            CloseState::Finished(result) => Some(result.clone()),
            CloseState::Running | CloseState::Closing => None,
        }
    }

    fn finish(&self, result: Result<Completion, TerminalError>) {
        *self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = CloseState::Finished(result);
    }
}

struct CompletionGuard(Option<NativePaneCompletion>);

impl CompletionGuard {
    fn finish(mut self, result: Result<Completion, TerminalError>) {
        if let Some(observation) = self.0.take() {
            if let Err(error) = &result {
                log::error!("Native pane cleanup failed: {error}");
            }
            observation.finish(result);
        }
    }
}

impl Drop for CompletionGuard {
    fn drop(&mut self) {
        if let Some(observation) = self.0.take() {
            let error = failure("completion observation cancelled");
            log::error!("Native pane cleanup failed: {error}");
            observation.finish(Err(error));
        }
    }
}

#[derive(Default)]
struct EventState {
    dirty: bool,
    title: Option<String>,
    cwd: Option<PathBuf>,
    status: Option<ProgramState>,
    exited: Option<Option<i32>>,
    closed: bool,
    error: Option<String>,
    clipboard: VecDeque<ClipboardEffect>,
    notifications: VecDeque<String>,
    dropped_clipboard: u64,
    dropped_notifications: u64,
    progress: Option<oneterm_terminal::TerminalProgress>,
    bell: bool,
    stopped: bool,
}

enum ClipboardEffect {
    Write(Option<String>),
    Read,
}

impl EventState {
    fn event(&mut self, event: SessionEvent) {
        self.dirty = true;
        match event {
            SessionEvent::Title(title) => self.title = Some(title),
            SessionEvent::Cwd(cwd) => self.cwd = Some(cwd),
            SessionEvent::ProgramStatus(status) => {
                self.status = status.filter(|state| *state != ProgramState::Idle);
            }
            SessionEvent::Exited(code) => {
                self.exited = Some(code);
                self.stop_effects();
                self.clear_live_status();
            }
            SessionEvent::Closed => {
                self.closed = true;
                self.stop_effects();
                self.clear_live_status();
            }
            SessionEvent::Clipboard(text) if !self.stopped => {
                self.queue_clipboard(ClipboardEffect::Write(text))
            }
            SessionEvent::ClipboardRead if !self.stopped => {
                self.queue_clipboard(ClipboardEffect::Read)
            }
            SessionEvent::Notification(message) if !self.stopped => {
                if self.notifications.len() == MAX_QUEUED_NOTIFICATIONS {
                    self.notifications.pop_front();
                    self.dropped_notifications = self.dropped_notifications.saturating_add(1);
                }
                self.notifications.push_back(message);
            }
            SessionEvent::Progress(progress) if !self.stopped => {
                self.progress = match progress {
                    oneterm_terminal::TerminalProgress::Remove => None,
                    other => Some(other),
                };
            }
            SessionEvent::Bell if !self.stopped => self.bell = true,
            SessionEvent::Output
            | SessionEvent::ShellIntegration(_)
            | SessionEvent::Clipboard(_)
            | SessionEvent::ClipboardRead
            | SessionEvent::Notification(_)
            | SessionEvent::Progress(_)
            | SessionEvent::Bell => {}
        }
    }

    fn queue_clipboard(&mut self, operation: ClipboardEffect) {
        if self.clipboard.len() == MAX_QUEUED_NOTIFICATIONS {
            self.dropped_clipboard = self.dropped_clipboard.saturating_add(1);
            self.error = Some(format!(
                "{} native clipboard operations dropped while the UI was busy",
                self.dropped_clipboard
            ));
            return;
        }
        self.clipboard.push_back(operation);
    }

    fn stop_effects(&mut self) {
        self.stopped = true;
        self.clipboard.clear();
        self.notifications.clear();
        self.progress = None;
        self.bell = false;
    }

    fn clear_live_status(&mut self) {
        if matches!(
            self.status,
            Some(ProgramState::Working | ProgramState::Blocked)
        ) {
            self.status = None;
        }
    }
}

/// Owns a real native surface and a single session completion obligation.
/// Not selected by production factories; program effects retain backend policy.
pub struct NativeTerminalPane {
    channels: channels::Channels,
    pub(crate) duplicate_config: Option<oneterm_core::SessionDuplicateConfig>,
    pub(crate) split_ctx: Option<crate::space::SplitContext>,
    surface: Option<Entity<NativeTerminalSurface>>,
    session: Option<Arc<dyn NativeSession>>,
    events_done: Option<async_channel::Receiver<()>>,
    state: Arc<Mutex<EventState>>,
    completion: NativePaneCompletion,
    focus: FocusHandle,
    settings: Option<Entity<oneterm_settings::TerminalSettings>>,
    _release: Subscription,
}

impl NativeTerminalPane {
    /// The caller passes actual session ownership, never a legacy render shim.
    /// Reliable events are drained on the background executor, including while
    /// hidden and after the pane/surface is released.
    pub fn new(
        session: Arc<dyn NativeSession>,
        font: Font,
        font_size: Pixels,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<Self, TerminalError> {
        let events = session
            .take_events()
            .ok_or_else(|| failure("event receiver already taken"))?;
        Ok(Self::with_events(
            session, events, font, font_size, window, cx,
        ))
    }

    pub(crate) fn with_events(
        session: Arc<dyn NativeSession>,
        events: async_channel::Receiver<SessionEvent>,
        font: Font,
        font_size: Pixels,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let state = Arc::new(Mutex::new(EventState::default()));
        let received = state.clone();
        let (done, events_done) = async_channel::bounded(1);
        cx.background_spawn(async move {
            while let Ok(event) = events.recv().await {
                received
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .event(event);
            }
            if done.send(()).await.is_err() {
                // A failed/cancelled close observer no longer has a recipient.
                log::debug!("Native pane event completion recipient closed");
            }
        })
        .detach();
        let source = session.clone();
        let errors = state.clone();
        let surface = cx.new(|cx| {
            NativeTerminalSurface::new(
                source,
                font,
                font_size,
                move |error, _| {
                    let mut state = errors
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    state.error = Some(format!("Native terminal input failed: {error:?}"));
                    state.dirty = true;
                },
                window,
                cx,
            )
        });
        let focus = surface.read(cx).focus_handle(cx);
        cx.spawn(async move |pane, cx| {
            loop {
                cx.background_executor().timer(REFRESH_INTERVAL).await;
                let Ok(alive) = pane.update(cx, |pane, cx| {
                    if pane.session.is_none() {
                        return false;
                    }
                    if !pane.session_alive() {
                        pane.leave_channel(cx);
                    }
                    pane.apply_clipboard(cx);
                    let dirty = {
                        let mut state = pane
                            .state
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        std::mem::take(&mut state.dirty)
                    };
                    if dirty {
                        if let Some(session) = &pane.session {
                            session.release_repaint_hint();
                        }
                        if let Some(surface) = &pane.surface {
                            surface.update(cx, |surface, cx| surface.invalidate(cx));
                        }
                        cx.emit(TerminalViewEvent::TitleChanged);
                        cx.notify();
                    }
                    true
                }) else {
                    break;
                };
                if !alive {
                    break;
                }
            }
        })
        .detach();
        let release = cx.on_release(|pane, cx| pane.shutdown(cx));
        let completion = NativePaneCompletion(Arc::new(Mutex::new(CloseState::Running)));
        crate::NativeWorkspaceSessions::register(cx.entity().downgrade(), completion.clone(), cx);
        Self {
            channels: channels::Channels::default(),
            duplicate_config: None,
            split_ctx: None,
            surface: Some(surface),
            session: Some(session),
            events_done: Some(events_done),
            state,
            focus,
            settings: None,
            completion,
            _release: release,
        }
    }

    pub fn completion(&self) -> NativePaneCompletion {
        self.completion.clone()
    }

    #[cfg(test)]
    pub(crate) fn edit_pending(&self, cx: &App) -> bool {
        self.surface
            .as_ref()
            .is_some_and(|surface| surface.read(cx).edit_pending())
    }

    pub(crate) fn bind_settings(
        &mut self,
        settings: Entity<oneterm_settings::TerminalSettings>,
        cx: &mut App,
    ) {
        if let Some(surface) = &self.surface {
            surface.update(cx, |surface, cx| {
                surface.bind_settings(settings.clone(), cx)
            });
        }
        self.settings = Some(settings);
    }

    fn apply_clipboard(&self, cx: &mut App) {
        let Some(session) = self.session.as_ref().filter(|session| session.alive()) else {
            return;
        };
        let operations = {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if state.stopped {
                return;
            }
            std::mem::take(&mut state.clipboard)
        };
        for operation in operations {
            if !session.alive()
                || self
                    .state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .stopped
            {
                break;
            }
            match operation {
                ClipboardEffect::Write(text) => {
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(text.unwrap_or_default()))
                }
                ClipboardEffect::Read => {
                    if !self
                        .settings
                        .as_ref()
                        .is_some_and(|settings| settings.read(cx).allow_clipboard_read)
                    {
                        continue;
                    }
                    let text = cx
                        .read_from_clipboard()
                        .and_then(|item| item.text())
                        .unwrap_or_default();
                    if let Err(error) = session.reply_clipboard(&text) {
                        let mut state = self
                            .state
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        state.error = Some(format!("Native clipboard reply failed: {error}"));
                        state.dirty = true;
                    }
                }
            }
        }
    }

    pub(crate) fn title(&self) -> Option<String> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .title
            .clone()
    }
    pub(crate) fn cwd(&self) -> Option<PathBuf> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .cwd
            .clone()
    }
    pub(crate) fn kind(&self) -> Option<oneterm_terminal::SessionKind> {
        self.session.as_ref().map(|session| session.kind())
    }
    pub(crate) fn capabilities(&self) -> oneterm_terminal::TerminalCapabilities {
        self.session
            .as_ref()
            .map_or_else(Default::default, |session| session.capabilities())
    }
    pub(crate) fn program_status(&self) -> Option<ProgramState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .status
    }
    pub(crate) fn acknowledge_program_status(&mut self, cx: &mut Context<Self>) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if matches!(state.status, Some(ProgramState::Done | ProgramState::Error)) {
            state.status = None;
            cx.emit(TerminalViewEvent::StatusChanged);
        }
    }
    pub(crate) fn toggle_search(&self, window: &mut Window, cx: &mut App) {
        if let Some(surface) = &self.surface {
            surface.update(cx, |surface, cx| surface.toggle_search(window, cx));
        }
    }

    pub(crate) fn unsupported(&self, operation: &str, cx: &mut Context<Self>) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.error = Some(format!("Native terminal {operation} is not integrated yet"));
        state.dirty = true;
        cx.notify();
    }

    pub(crate) fn edit(&self, paste: bool, cx: &mut App) {
        if let Some(surface) = &self.surface {
            surface.update(cx, |surface, cx| {
                if paste {
                    surface.paste_clipboard(cx);
                } else {
                    surface.copy_clipboard(cx);
                }
            });
        }
    }

    pub(crate) fn select_all(&self, cx: &mut App) {
        if let Some(surface) = &self.surface {
            surface.update(cx, |surface, cx| surface.select_all(cx));
        }
    }

    pub(crate) fn clear_command(&self, cx: &mut App) {
        if let Some(surface) = &self.surface {
            surface.update(cx, |surface, cx| surface.clear_command(cx));
        }
    }

    /// Request close once and release foreground control ownership before
    /// observing native destruction and independent event-stream completion.
    pub fn shutdown(&mut self, cx: &mut App) {
        self.leave_channel(cx);
        self.channels.stop();
        let Some(session) = self.session.take() else {
            return;
        };
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stop_effects();
        *self
            .completion
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = CloseState::Closing;
        let close = session.request_shutdown();
        self.surface.take();
        let events_done = self
            .events_done
            .take()
            .expect("native pane owns its event completion receiver");
        let guard = CompletionGuard(Some(self.completion.clone()));
        let executor = cx.background_executor().clone();
        let timer = executor.clone();
        let deadline = Instant::now() + CLOSE_BUDGET;
        executor
            .spawn(async move {
                let started = Instant::now();
                log::debug!(
                    "Native pane cleanup waiting for {} references",
                    Arc::strong_count(&session)
                );
                while Arc::strong_count(&session) > 1 {
                    if Instant::now() >= deadline {
                        guard.finish(Err(failure("session references remain at close deadline")));
                        return;
                    }
                    timer.timer(Duration::from_millis(1)).await;
                }
                log::debug!(
                    "Native pane references released after {:?}",
                    started.elapsed()
                );
                let result = async move {
                    let result = session.complete().await;
                    log::debug!(
                        "Native pane owner completed after {:?}: {result:?}",
                        started.elapsed()
                    );
                    events_done
                        .recv()
                        .await
                        .map_err(|_| failure("event consumer completion lost"))?;
                    log::debug!("Native pane events completed after {:?}", started.elapsed());
                    close?;
                    let report = result?;
                    if !report.lifecycle_delivered {
                        return Err(failure("lifecycle delivery incomplete"));
                    }
                    Ok(report)
                };
                let result = std::pin::pin!(result);
                let timeout =
                    std::pin::pin!(timer.timer(deadline.saturating_duration_since(Instant::now())));
                guard.finish(race_completion(result, timeout).await);
            })
            .detach();
    }
}

/// A connection can finish after its initiating window or duplicate target died.
/// It still owns reliable event draining and a consuming completion obligation.
pub(crate) fn close_unplaced(
    session: Arc<dyn NativeSession>,
    executor: gpui::BackgroundExecutor,
) -> NativePaneCompletion {
    let completion = NativePaneCompletion(Arc::new(Mutex::new(CloseState::Closing)));
    let observation = completion.clone();
    let guard = CompletionGuard(Some(completion));
    let close = session.request_shutdown();
    let events = session.take_events();
    let drain = executor.spawn(async move {
        let events = events.ok_or_else(|| failure("unplaced event receiver already taken"))?;
        while events.recv().await.is_ok() {}
        Ok::<(), TerminalError>(())
    });
    let timer = executor.clone();
    executor
        .spawn(async move {
            let result = async move {
                let report = session.complete().await;
                drain.await?;
                close?;
                let report = report?;
                if !report.lifecycle_delivered {
                    return Err(failure("unplaced lifecycle delivery incomplete"));
                }
                Ok(report)
            };
            let result = std::pin::pin!(result);
            let timeout = std::pin::pin!(timer.timer(CLOSE_BUDGET));
            guard.finish(race_completion(result, timeout).await);
        })
        .detach();
    observation
}

async fn race_completion(
    mut result: std::pin::Pin<
        &mut impl std::future::Future<Output = Result<Completion, TerminalError>>,
    >,
    mut timeout: std::pin::Pin<&mut impl std::future::Future<Output = ()>>,
) -> Result<Completion, TerminalError> {
    std::future::poll_fn(|cx| {
        if let std::task::Poll::Ready(result) = result.as_mut().poll(cx) {
            return std::task::Poll::Ready(result);
        }
        if timeout.as_mut().poll(cx).is_ready() {
            return std::task::Poll::Ready(Err(failure(
                "native owner or event consumer close deadline exceeded",
            )));
        }
        std::task::Poll::Pending
    })
    .await
}

fn failure(message: &str) -> TerminalError {
    TerminalError::Transport(format!("Native pane: {message}"))
}

impl EventEmitter<TerminalViewEvent> for NativeTerminalPane {}

impl Focusable for NativeTerminalPane {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for NativeTerminalPane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let error = state.error.clone();
        let progress = state.progress;
        let bell = state.bell;
        let notifications = std::mem::take(&mut state.notifications);
        let dropped = std::mem::take(&mut state.dropped_notifications);
        drop(state);
        if dropped > 0 {
            window.push_notification(
                notify(
                    NotificationType::Warning,
                    format!("{dropped} terminal notifications were dropped while the UI was busy."),
                    cx,
                ),
                cx,
            );
        }
        for message in notifications {
            window.push_notification(notify(NotificationType::Info, message, cx), cx);
        }
        let logging = self.capabilities().logging;
        if let Some(message) = logging.as_ref().and_then(|logging| logging.take_error()) {
            window.push_notification(notify(NotificationType::Error, message, cx), cx);
        }
        let recording = logging.is_some_and(|logging| {
            matches!(
                logging.state(),
                oneterm_terminal::TerminalLogState::Running { .. }
            )
        }) && self
            .split_ctx
            .as_ref()
            .and_then(|split| split.panel.upgrade())
            .is_some_and(|panel| panel.read(cx).leaf_count() > 1);
        let theme = cx.theme();
        let colors = OverlayColors {
            blue: theme.blue,
            danger: theme.danger,
            warning: theme.warning,
            muted: theme.muted,
        };
        let bell_enabled = self
            .settings
            .as_ref()
            .is_some_and(|settings| settings.read(cx).bell_enabled);
        let body = div()
            .size_full()
            .flex()
            .flex_col()
            .children(error.map(|error| div().child(error)))
            .children(self.surface.clone().map(|surface| {
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .child(surface)
                    .children(progress_overlay(progress, colors))
                    .children(bell_badge(bell, bell_enabled, colors))
                    .children(recording.then(|| {
                        div()
                            .absolute()
                            .top_1()
                            .right_2()
                            .text_xs()
                            .text_color(colors.danger)
                            .child("●")
                    }))
            }));
        self.with_context_menu(body, cx)
    }
}

#[cfg(test)]
#[path = "native_pane_tests.rs"]
mod tests;
