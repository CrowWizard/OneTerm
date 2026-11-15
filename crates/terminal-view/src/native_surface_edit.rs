//! Clipboard I/O stays on GPUI; native formatting/paste use asynchronous owners.

use gpui::{ClipboardItem, Context, KeyDownEvent, Task};
use oneterm_terminal::{TerminalError, backend::AsyncGhosttyPumpError};

use super::{NativeSurfaceError, NativeTerminalSurface};

#[derive(Default)]
pub(super) struct Editing {
    pub(super) busy: bool,
    task: Option<Task<()>>,
}

impl NativeTerminalSurface {
    #[cfg(test)]
    pub(crate) fn edit_pending(&self) -> bool {
        self.edit.busy
    }

    /// Select native history and screen; publication is acknowledged before completion.
    pub fn select_all(&mut self, cx: &mut Context<Self>) {
        self.edit_selection(false, cx);
    }

    /// Preserve shell clear-command semantics without resetting native history.
    pub fn clear_command(&mut self, cx: &mut Context<Self>) {
        self.edit_selection(true, cx);
    }

    fn edit_selection(&mut self, clear: bool, cx: &mut Context<Self>) {
        if !self.start_edit(cx) {
            return;
        }
        let session = self.session.clone();
        cx.spawn(async move |surface, cx| {
            let operation = if clear { "clear command" } else { "select-all" };
            let result = if clear {
                session.clear_command().await
            } else {
                session
                    .selection(oneterm_terminal::ghostty::selection::Update::All)
                    .await
            };
            if let Err(error) = surface.update(cx, |surface, cx| {
                surface.edit.busy = false;
                if let Err(error) = &result {
                    (surface.on_error)(NativeSurfaceError::Edit(error.clone()), cx);
                }
                cx.notify();
            }) {
                if let Err(failure) = result {
                    log::warn!("Native {operation} failed after recipient release: {failure}");
                }
                log::debug!("Native {operation} recipient closed: {error}");
            }
        })
        .detach();
    }

    pub(super) fn clipboard_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        let stroke = &event.keystroke;
        let command = if crate::input::keys::is_clipboard_chord(&stroke.modifiers) {
            match stroke.key.as_str() {
                "c" | "C" => Some(false),
                "v" | "V" => Some(true),
                _ => None,
            }
        } else if stroke.modifiers.shift
            && !stroke.modifiers.control
            && !stroke.modifiers.alt
            && !stroke.modifiers.platform
            && stroke.key == "insert"
        {
            Some(true)
        } else {
            None
        };
        let Some(paste) = command else {
            return false;
        };
        if !event.is_held {
            if paste {
                self.paste_clipboard(cx);
            } else {
                self.copy_clipboard(cx);
            }
        }
        // Never let the platform resend a clipboard chord through keys or IME.
        cx.stop_propagation();
        true
    }

    fn start_edit(&mut self, cx: &mut Context<Self>) -> bool {
        if self.edit.busy {
            (self.on_error)(
                NativeSurfaceError::Edit(AsyncGhosttyPumpError::Transport(
                    TerminalError::QueueFull,
                )),
                cx,
            );
            return false;
        }
        self.edit.busy = true;
        true
    }

    /// Copy current native selection, not painted cells or legacy row numbers.
    pub fn copy_clipboard(&mut self, cx: &mut Context<Self>) {
        if !self.start_edit(cx) {
            return;
        }
        let model = self.session.render_model();
        self.edit.task = Some(cx.spawn(async move |surface, cx| {
            let result = model.selection_text().await;
            if let Err(error) = surface.update(cx, |surface, cx| {
                surface.edit.busy = false;
                match result {
                    Ok(Some(text)) if !text.is_empty() => {
                        cx.write_to_clipboard(ClipboardItem::new_string(text))
                    }
                    Ok(_) => {}
                    Err(error) => (surface.on_error)(NativeSurfaceError::Copy(error), cx),
                }
            }) {
                // The view was dropped; there is no clipboard recipient left.
                log::debug!("Native clipboard copy recipient closed: {error}");
            }
        }));
    }

    /// Read once. Only the session chooses encoding, filtering and transport.
    pub fn paste_clipboard(&mut self, cx: &mut Context<Self>) {
        if self.edit.busy {
            self.start_edit(cx);
            return;
        }
        let Some(text) = cx
            .read_from_clipboard()
            .and_then(|item| item.text())
            .filter(|text| !text.is_empty())
        else {
            return;
        };
        if let Err(error) = oneterm_terminal::ghostty_session::validate_paste(&text) {
            (self.on_error)(
                NativeSurfaceError::Edit(AsyncGhosttyPumpError::Paste(error)),
                cx,
            );
            return;
        }
        self.start_edit(cx);
        if self.channels.bound() {
            if !self.queue_channel_paste(text, cx) {
                self.edit.busy = false;
            }
            return;
        }
        let session = self.session.clone();
        self.edit.task = Some(cx.spawn(async move |surface, cx| {
            let result = session.paste_visible(&text).await;
            if let Err(error) = surface.update(cx, |surface, cx| {
                surface.edit.busy = false;
                if let Err(error) = result {
                    (surface.on_error)(NativeSurfaceError::Edit(error), cx);
                }
                cx.notify();
            }) {
                // View teardown owns task cancellation; no implicit paste retry.
                log::debug!("Native clipboard paste recipient closed: {error}");
            }
        }));
    }
}
