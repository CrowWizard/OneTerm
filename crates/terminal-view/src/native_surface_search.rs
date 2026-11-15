//! One UI driver owns search progress. Supersession never drops an admitted
//! endpoint future: it waits for acknowledgment, discards it, then sends the latest query.

use std::time::Duration;

use gpui::{
    AppContext as _, Context, Entity, Focusable as _, InteractiveElement as _, IntoElement,
    KeyDownEvent, MouseButton, ParentElement as _, SharedString, Styled as _, Subscription, Task,
    Window, div, px,
};
use gpui_component::{
    ActiveTheme as _, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
    input::{Input, InputEvent, InputState},
};
use oneterm_terminal::ghostty::search::{MAX_NEEDLE_BYTES, Options, Snapshot, Status};

use super::{NativeSurfaceError, NativeTerminalSurface};

const STEP_BUDGET: usize = 64;

#[derive(Default)]
pub(super) struct Search {
    pub(super) open: bool,
    pub(super) input: Option<Entity<InputState>>,
    subscription: Option<Subscription>,
    pub(super) query: String,
    generation: u64,
    requested: bool,
    pub(super) busy: bool,
    steps: usize,
    pub(super) result: Option<Snapshot>,
    pub(super) index: Option<usize>,
    navigate: Option<bool>,
    pub(super) error: Option<String>,
    task: Option<Task<()>>,
}

enum Work {
    Set(String),
    Step,
    Reveal(oneterm_terminal::ghostty::search::Navigation),
}

impl Drop for Search {
    fn drop(&mut self) {
        // An admitted endpoint future is fail-stop on drop. Let the driver
        // acknowledge it and clear native state instead of closing the session.
        if let Some(task) = self.task.take() {
            task.detach();
        }
    }
}

impl NativeTerminalSurface {
    pub(crate) fn toggle_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.search.open {
            self.close_search(window, cx);
        } else {
            self.open_search(window, cx);
        }
    }
    pub(super) fn search_input_focused(&self, window: &Window, cx: &gpui::App) -> bool {
        self.search
            .input
            .as_ref()
            .is_some_and(|input| input.read(cx).focus_handle(cx).is_focused(window))
    }
    pub(super) fn search_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let key = &event.keystroke;
        let find = (key.modifiers.control || key.modifiers.platform)
            && !key.modifiers.alt
            && !key.modifiers.shift
            && key.key.eq_ignore_ascii_case("f");
        if find || (self.search.open && key.key == "escape") {
            if self.search.open {
                self.close_search(window, cx);
            } else {
                self.open_search(window, cx);
            }
            cx.stop_propagation();
            return true;
        }
        // InputState owns ordinary editing and IME. Never forward a bubbled key
        // to the terminal while this input has focus.
        self.search_input_focused(window, cx)
    }

    fn open_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.release_held_keys(cx);
        self.release_pointer(cx);
        self.session.clear_marked_text();
        self.search.open = true;
        let input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Find in terminal (ASCII case-insensitive)")
        });
        self.search.subscription = Some(cx.subscribe(
            &input,
            |surface, input, event: &InputEvent, cx| match event {
                InputEvent::Change => {
                    let query = input.read(cx).value().to_string();
                    // InputState may emit Change after Enter even when its text
                    // is unchanged. Do not supersede the navigation it just sent.
                    if surface.search.query != query {
                        surface.search.query = query;
                        surface.request_search(cx);
                    }
                }
                InputEvent::PressEnter { shift, .. } => surface.navigate_search(*shift, cx),
                InputEvent::Focus | InputEvent::Blur => {}
            },
        ));
        input.update(cx, |input, cx| input.focus(window, cx));
        self.search.input = Some(input);
        self.search.query.clear();
        self.request_search(cx);
    }

    pub(super) fn close_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search.open = false;
        self.search.input = None;
        self.search.subscription = None;
        self.search.query.clear();
        // Keep the active task alive until its admitted operation is acknowledged.
        // The generation change then drives an ordered native clear.
        self.request_search(cx);
        window.focus(&self.focus, cx);
    }

    pub(super) fn request_search(&mut self, cx: &mut Context<Self>) {
        self.search.generation = self.search.generation.wrapping_add(1);
        self.search.result = None;
        self.search.index = None;
        self.search.navigate = None;
        self.search.error = None;
        self.search.steps = 0;
        self.search.requested = true;
        self.drive_search(cx);
        cx.notify();
    }

    pub(super) fn cleanup_search_on_release(&mut self, cx: &mut gpui::App) {
        // A running serial driver owns the only safe clear ordering. If it is
        // idle, schedule a detached clear while the session Arc is still alive.
        if self.search.busy {
            self.search.generation = self.search.generation.wrapping_add(1);
            self.search.requested = true;
            return;
        }
        let session = self.session.clone();
        cx.background_executor()
            .spawn(async move {
                if let Err(error) = session.search(String::new(), Options::default()).await {
                    log::debug!("Native search release clear failed: {error}");
                }
            })
            .detach();
    }

    pub(super) fn refresh_search(&mut self, cx: &mut Context<Self>) {
        if !self.search.busy
            && self.search.open
            && self
                .search
                .result
                .as_ref()
                .is_some_and(|result| !self.session.query().matches_search(result))
        {
            self.request_search(cx);
        }
    }

    pub(super) fn navigate_search(&mut self, backward: bool, cx: &mut Context<Self>) {
        self.refresh_search(cx);
        if self
            .search
            .result
            .as_ref()
            .is_some_and(|result| !result.matches.is_empty())
        {
            self.search.navigate = Some(backward);
            self.drive_search(cx);
        }
    }

    fn next_search_work(&mut self) -> Option<(u64, Work)> {
        if self
            .search
            .result
            .as_ref()
            .is_some_and(|result| !self.session.query().matches_search(result))
        {
            self.search.generation = self.search.generation.wrapping_add(1);
            self.search.result = None;
            self.search.index = None;
            self.search.navigate = None;
            self.search.steps = 0;
            self.search.requested = true;
        }
        let work = if self.search.requested {
            self.search.requested = false;
            // Oversized input is visible as a policy error; still clear native
            // state so the rejected query cannot leave old matches retained.
            let needle = if self.search.query.len() > MAX_NEEDLE_BYTES {
                self.search.error = Some("Query exceeds 4096 UTF-8 bytes".into());
                String::new()
            } else {
                self.search.query.clone()
            };
            Work::Set(needle)
        } else if let Some(backward) = self.search.navigate.take() {
            let result = self.search.result.as_ref()?;
            if !self.session.query().matches_search(result) {
                return None;
            }
            let index = next_index(result.matches.len(), self.search.index, backward)?;
            self.search.index = Some(index);
            Work::Reveal(result.navigation(index)?)
        } else if self.search.open
            && self.search.steps < STEP_BUDGET
            && self
                .search
                .result
                .as_ref()
                .is_some_and(|result| result.status == Status::Running)
        {
            Work::Step
        } else {
            return None;
        };
        Some((self.search.generation, work))
    }

    fn drive_search(&mut self, cx: &mut Context<Self>) {
        if self.search.busy {
            return;
        }
        self.search.busy = true;
        let session = self.session.clone();
        self.search.task = Some(cx.spawn(async move |surface, cx| {
            loop {
                let work = surface.update(cx, |surface, _| {
                    let work = surface.next_search_work();
                    if work.is_none() {
                        surface.search.busy = false;
                    }
                    work
                });
                let (generation, work) = match work {
                    Ok(Some(work)) => work,
                    Ok(None) => break,
                    Err(_) => {
                        if let Err(error) = session.search(String::new(), Options::default()).await
                        {
                            log::debug!("Native search teardown clear failed: {error}");
                        }
                        break;
                    }
                };
                if matches!(&work, Work::Set(needle) if !needle.is_empty()) {
                    cx.background_executor()
                        .timer(Duration::from_millis(150))
                        .await;
                    if !surface
                        .update(cx, |surface, _| surface.search.generation == generation)
                        .unwrap_or(false)
                    {
                        continue;
                    }
                }
                let revealing = matches!(&work, Work::Reveal(_));
                let result = match work {
                    Work::Set(needle) => session.search(needle, Options::default()).await,
                    Work::Step => session.search_step().await,
                    Work::Reveal(navigation) => session.reveal_search(navigation).await,
                };
                // A result-cap rejection can leave a large native result set.
                // Release it in this same serial driver, even when superseded.
                let result = if result
                    .as_ref()
                    .is_err_and(|error| error.is_search_rejection())
                {
                    match session.search(String::new(), Options::default()).await {
                        Ok(_) => result,
                        Err(error) => Err(error),
                    }
                } else {
                    result
                };
                if surface
                    .update(cx, |surface, cx| {
                        if surface.search.generation != generation {
                            return;
                        }
                        match result {
                            Ok(result) => {
                                surface.search.steps += 1;
                                if surface.search.open
                                    && surface.search.error.is_none()
                                    && !surface.search.query.is_empty()
                                {
                                    if session.query().matches_search(&result) {
                                        if !revealing
                                            && surface.search.result.as_ref().is_some_and(
                                                |previous| {
                                                    !session.query().matches_search(previous)
                                                },
                                            )
                                        {
                                            surface.search.index = None;
                                        }
                                        surface.search.index = surface
                                            .search
                                            .index
                                            .filter(|&i| i < result.matches.len())
                                            .or_else(|| (!result.matches.is_empty()).then_some(0));
                                        surface.search.result = Some(result);
                                    } else {
                                        // Output won the race; refresh, but never navigate old coordinates.
                                        surface.request_search(cx);
                                    }
                                }
                            }
                            Err(error) => {
                                if error.is_search_expired() && surface.search.open {
                                    surface.request_search(cx);
                                    return;
                                }
                                surface.search.result = None;
                                surface.search.index = None;
                                surface.search.error = Some(error.to_string());
                                (surface.on_error)(NativeSurfaceError::Edit(error), cx);
                            }
                        }
                        cx.notify();
                    })
                    .is_err()
                {
                    if let Err(error) = session.search(String::new(), Options::default()).await {
                        log::debug!("Native search teardown clear failed: {error}");
                    }
                    break;
                }
                // Native work is one tick per await; yield between progress ticks.
                cx.background_executor()
                    .timer(Duration::from_millis(1))
                    .await;
            }
        }));
    }

    pub(super) fn render_search(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        if !self.search.open {
            return None;
        }
        let input = self.search.input.clone()?;
        let label: SharedString = if let Some(error) = &self.search.error {
            error.clone().into()
        } else if self.search.query.is_empty() {
            "".into()
        } else if let Some(result) = &self.search.result {
            let suffix = if result.status == Status::Complete {
                ""
            } else {
                " (partial)"
            };
            format!(
                "{} / {}{}",
                self.search.index.map_or(0, |index| index + 1),
                result.matches.len(),
                suffix
            )
            .into()
        } else {
            "Searching…".into()
        };
        Some(
            div()
                .id("native-search-bar")
                .absolute()
                .top_1()
                .right_1()
                .flex()
                .items_center()
                .gap_1()
                .p_1()
                .rounded_md()
                .shadow_md()
                .bg(cx.theme().background)
                .border_1()
                .border_color(cx.theme().border)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_down(MouseButton::Middle, |_, _, cx| cx.stop_propagation())
                .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
                .on_mouse_move(|_, _, cx| cx.stop_propagation())
                .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
                .child(Input::new(&input).w(px(230.)).small())
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(label),
                )
                .child(
                    Button::new("native-search-prev")
                        .ghost()
                        .xsmall()
                        .icon(IconName::ChevronUp)
                        .on_click(
                            cx.listener(|surface, _, _, cx| surface.navigate_search(true, cx)),
                        ),
                )
                .child(
                    Button::new("native-search-next")
                        .ghost()
                        .xsmall()
                        .icon(IconName::ChevronDown)
                        .on_click(
                            cx.listener(|surface, _, _, cx| surface.navigate_search(false, cx)),
                        ),
                )
                .child(
                    Button::new("native-search-close")
                        .ghost()
                        .xsmall()
                        .icon(IconName::Close)
                        .on_click(
                            cx.listener(|surface, _, window, cx| surface.close_search(window, cx)),
                        ),
                ),
        )
    }
}

fn next_index(len: usize, index: Option<usize>, backward: bool) -> Option<usize> {
    if len == 0 {
        return None;
    }
    Some(match index.filter(|&i| i < len) {
        Some(i) if backward => (i + len - 1) % len,
        Some(i) => (i + 1) % len,
        None => 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_wraps_and_rejects_empty_lists() {
        assert_eq!(next_index(0, None, false), None);
        assert_eq!(next_index(3, Some(2), false), Some(0));
        assert_eq!(next_index(3, Some(0), true), Some(2));
        assert_eq!(next_index(1, Some(0), true), Some(0));
        assert_eq!(next_index(3, Some(99), false), Some(0));
    }
}
