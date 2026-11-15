//! Opt-in native render/input core. The caller owns lifecycle, reliable events
//! and security, and invalidates this entity after a native publication.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    App, Context, Entity, FocusHandle, Focusable, Font, InteractiveElement as _, IntoElement,
    KeyDownEvent, KeyUpEvent, Keystroke, MouseButton, ParentElement as _, Pixels, Render,
    Styled as _, Subscription, Window, div, px,
};
use gpui_component::ActiveTheme as _;
use oneterm_terminal::ghostty::input::{KeyAction, KeyCode, KeyEvent, Modifiers};
use oneterm_terminal::ghostty_session::{InputError, SurfaceSession};

use crate::render::element::{ElementSession, TerminalElement, TerminalElementSpec};
use crate::render::state::{RenderInputs, RenderState};
use crate::theme::build_terminal_theme;

#[path = "native_surface_channels.rs"]
mod channels;
#[path = "native_surface_colors.rs"]
mod colors;
#[path = "native_surface_cursor.rs"]
mod cursor;
#[path = "native_surface_edit.rs"]
mod edit;
#[path = "native_surface_ime.rs"]
mod ime;
#[path = "native_surface_input_handler.rs"]
mod input_handler;
#[path = "native_surface_pointer.rs"]
mod pointer;
#[path = "native_surface_search.rs"]
mod search;
#[path = "native_surface_selection.rs"]
mod selection;

/// Input failures are delivered to the embedding UI, never silently discarded.
#[derive(Debug)]
pub enum NativeSurfaceError {
    Control(InputError),
    Composition(oneterm_terminal::ghostty_ime::Error),
    Edit(oneterm_terminal::backend::AsyncGhosttyPumpError),
    Copy(oneterm_terminal::ghostty_bridge::Error),
    Colors(oneterm_terminal::backend::AsyncGhosttyPumpError),
}

/// Embeddable surface, not a production panel or a legacy TerminalSession shim.
pub struct NativeTerminalSurface {
    channels: channels::OriginQueue,
    settings: Option<Entity<oneterm_settings::TerminalSettings>>,
    settings_subscription: Option<Subscription>,
    theme_subscription: Option<Subscription>,
    colors: colors::Colors,
    cursor: cursor::Cursor,
    session: Arc<dyn SurfaceSession>,
    focus: FocusHandle,
    state: Rc<RefCell<RenderState>>,
    on_error: ErrorHandler,
    held: Vec<(String, KeyEvent)>,
    pointer: pointer::Pointer,
    edit: edit::Editing,
    drag: selection::Drag,
    search: search::Search,
    _subscriptions: Vec<Subscription>,
}

type ErrorHandler = Rc<dyn Fn(NativeSurfaceError, &mut App)>;

impl NativeTerminalSurface {
    /// The callback must present failures to the user. This surface never takes
    /// the reliable SessionEvent receiver. Native colors remain caller-owned
    /// unless workspace settings are explicitly bound.
    pub fn new(
        session: Arc<dyn SurfaceSession>,
        font: Font,
        font_size: Pixels,
        on_error: impl Fn(NativeSurfaceError, &mut App) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        let subscriptions = vec![
            cx.on_release(|surface, cx| {
                log::info!(
                    "Native surface released: session references={}, search busy={}, edit busy={}",
                    Arc::strong_count(&surface.session),
                    surface.search.busy,
                    surface.edit.busy,
                );
                surface.cleanup_search_on_release(cx);
            }),
            cx.on_focus(&focus, window, |surface, _, cx| {
                surface.cursor_focus(true, cx);
            }),
            cx.on_blur(&focus, window, |surface, _, cx| {
                surface.cursor_focus(false, cx);
                surface.release_held_keys(cx);
                surface.release_pointer(cx);
                surface.session.clear_marked_text();
                cx.notify();
            }),
        ];
        Self {
            channels: channels::OriginQueue::default(),
            settings: None,
            settings_subscription: None,
            theme_subscription: None,
            colors: colors::Colors::default(),
            cursor: cursor::Cursor::default(),
            session,
            focus,
            state: Rc::new(RefCell::new(RenderState::new(RenderInputs::new(
                Rc::new(build_terminal_theme(cx.theme())),
                font,
                font_size,
            )))),
            on_error: Rc::new(on_error),
            held: Vec::new(),
            pointer: pointer::Pointer::default(),
            edit: edit::Editing::default(),
            drag: selection::Drag::default(),
            search: search::Search::default(),
            _subscriptions: subscriptions,
        }
    }

    /// Explicit workspace binding; standalone surfaces keep caller typography.
    pub(crate) fn bind_settings(
        &mut self,
        settings: Entity<oneterm_settings::TerminalSettings>,
        cx: &mut Context<Self>,
    ) {
        self.settings_subscription = Some(cx.observe(&settings, |surface, _, cx| {
            surface.refresh_typography(cx);
            surface.refresh_colors(cx);
            surface.refresh_cursor(cx);
            cx.notify();
        }));
        self.theme_subscription =
            Some(cx.observe_global::<gpui_component::Theme>(|surface, cx| {
                surface.refresh_typography(cx);
                surface.refresh_colors(cx);
                surface.refresh_cursor(cx);
                cx.notify();
            }));
        self.settings = Some(settings);
        self.bind_cursor(cx);
        self.refresh_typography(cx);
        self.refresh_colors(cx);
        self.refresh_cursor(cx);
        cx.notify();
    }

    fn refresh_typography(&mut self, cx: &App) {
        let Some(settings) = &self.settings else {
            return;
        };
        let settings = settings.read(cx);
        let theme = cx.theme();
        let family = settings
            .font_family
            .clone()
            .unwrap_or_else(|| theme.mono_font_family.clone());
        let mut state = self.state.borrow_mut();
        let inputs = &mut state.inputs;
        inputs.font = crate::terminal_view::terminal_font(settings, &family);
        inputs.font_size = settings.font_size.map(px).unwrap_or(theme.mono_font_size);
        inputs.line_height_factor = settings.line_height_factor;
        inputs.cell_width_override = settings.cell_width;
        inputs.padding = gpui::Edges {
            top: px(settings.padding.top),
            right: px(settings.padding.right),
            bottom: px(settings.padding.bottom),
            left: px(settings.padding.left),
        };
        inputs.semantic.set_enabled(!matches!(
            settings.semantic_highlighting,
            oneterm_settings::SemanticHighlightingMode::Off
        ));
        inputs
            .semantic
            .set_profile(crate::terminal_view::shell_kind_to_profile(
                settings.shell.kind,
            ));
    }

    /// The event/security owner calls this after processing a publication hint.
    pub fn invalidate(&mut self, cx: &mut Context<Self>) {
        if self
            .session
            .query()
            .viewport
            .is_some_and(|v| v.metadata().alternate_screen)
        {
            self.session.clear_marked_text();
        }
        cx.notify();
    }

    fn release_held_keys(&mut self, cx: &mut Context<Self>) {
        let held = std::mem::take(&mut self.held);
        for (name, mut key) in held {
            key.action = KeyAction::Release;
            key.modifiers = Modifiers::default();
            if self.channels.bound() {
                if !self.queue_channel_key(name.clone(), key.clone(), false, cx) {
                    self.held.push((name, key));
                }
                continue;
            }
            if let Err(error) = self.session.key(key) {
                (self.on_error)(NativeSurfaceError::Control(error), cx);
            }
        }
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.search_key(event, window, cx) {
            return;
        }
        if self.clipboard_key(event, cx) {
            return;
        }
        let Some(viewport) = self.session.query().viewport else {
            // No invented power-on modes. A publication is required for choosing
            // between keyboard text and IME ownership.
            return;
        };
        let stroke = &event.keystroke;
        // Composition owns Enter/arrows too; they choose candidates rather than
        // submitting a line to the application behind the IME.
        if self.session.marked_text().is_some() {
            return;
        }
        let Some(mut key) = key_event(stroke) else {
            return;
        };
        let functional = key.code != KeyCode::UNIDENTIFIED;
        let chord = stroke.modifiers.control || stroke.modifiers.alt || stroke.modifiers.platform;
        if !functional
            && !viewport.metadata().alternate_screen
            && (!chord || event.prefer_character_input)
        {
            return;
        }
        key.action = if event.is_held {
            KeyAction::Repeat
        } else {
            KeyAction::Press
        };
        if self.channels.bound() {
            let interrupt = stroke.key.eq_ignore_ascii_case("c")
                && stroke.modifiers.control
                && !stroke.modifiers.shift
                && !stroke.modifiers.alt
                && !stroke.modifiers.platform;
            if self.queue_channel_key(stroke.key.clone(), key.clone(), interrupt, cx)
                && !self.held.iter().any(|(name, _)| name == &stroke.key)
            {
                self.held.push((stroke.key.clone(), key));
            }
        } else {
            match self.session.key(key.clone()) {
                Ok(()) => {
                    if !self.held.iter().any(|(name, _)| name == &stroke.key) {
                        self.held.push((stroke.key.clone(), key));
                    }
                }
                Err(error) => (self.on_error)(NativeSurfaceError::Control(error), cx),
            }
        }
        // Native keyboard owns this event even if admission failed. Do not let
        // GPUI retry the same text via the IME path.
        cx.stop_propagation();
        cx.notify();
    }

    fn key_up(&mut self, event: &KeyUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(index) = self
            .held
            .iter()
            .position(|(name, _)| name == &event.keystroke.key)
        else {
            return;
        };
        let (_, mut key) = self.held[index].clone();
        key.action = KeyAction::Release;
        key.modifiers = modifiers(&event.keystroke);
        if self.channels.bound() {
            if self.queue_channel_key(event.keystroke.key.clone(), key, false, cx) {
                self.held.remove(index);
            }
            cx.stop_propagation();
            return;
        }
        self.held.remove(index);
        if let Err(error) = self.session.key(key) {
            (self.on_error)(NativeSurfaceError::Control(error), cx);
        }
        cx.stop_propagation();
    }
}

impl Focusable for NativeTerminalSurface {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for NativeTerminalSurface {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.refresh_typography(cx);
        self.refresh_colors(cx);
        self.cursor.focused = self.focus.is_focused(window);
        self.refresh_cursor(cx);
        self.refresh_search(cx);
        {
            let mut state = self.state.borrow_mut();
            state.inputs.native_search = self
                .search
                .result
                .clone()
                .map(|result| (result, self.search.index));
            if self.settings.is_none() {
                state.inputs.theme = Rc::new(build_terminal_theme(cx.theme()));
            }
            state.inputs.cursor.focused = self.focus.is_focused(window);
        }
        let ime = self.focus.is_focused(window).then(|| {
            let focus = self.focus.clone();
            let entity = cx.entity().downgrade();
            Box::new(move |bounds, window: &mut Window, cx: &mut App| {
                window.handle_input(
                    &focus,
                    input_handler::WeakInputHandler::new(entity, bounds),
                    cx,
                );
            }) as Box<dyn FnOnce(_, &mut Window, &mut App)>
        });
        let on_error = self.on_error.clone();
        let element = TerminalElement::new(TerminalElementSpec {
            id: "native-terminal-grid".into(),
            session: ElementSession::Native {
                session: self.session.clone(),
                on_error: Rc::new(move |error, cx| {
                    on_error(NativeSurfaceError::Control(error), cx)
                }),
            },
            state: self.state.clone(),
            ime,
        });
        div()
            .id("native-terminal-surface")
            .size_full()
            .relative()
            .track_focus(&self.focus)
            .key_context("Terminal")
            .on_action(
                cx.listener(|surface, _: &oneterm_actions::TerminalCopy, window, cx| {
                    if !surface.search_input_focused(window, cx) {
                        surface.copy_clipboard(cx);
                    }
                    cx.stop_propagation();
                }),
            )
            .on_action(
                cx.listener(|surface, _: &oneterm_actions::TerminalPaste, window, cx| {
                    if !surface.search_input_focused(window, cx) {
                        surface.paste_clipboard(cx);
                    }
                    cx.stop_propagation();
                }),
            )
            .on_mouse_down(MouseButton::Left, cx.listener(Self::pointer_down))
            .on_mouse_down(MouseButton::Middle, cx.listener(Self::pointer_down))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::pointer_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::pointer_up))
            .on_mouse_up(MouseButton::Middle, cx.listener(Self::pointer_up))
            .on_mouse_up(MouseButton::Right, cx.listener(Self::pointer_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::pointer_up))
            .on_mouse_up_out(MouseButton::Middle, cx.listener(Self::pointer_up))
            .on_mouse_up_out(MouseButton::Right, cx.listener(Self::pointer_up))
            .on_mouse_move(cx.listener(Self::pointer_move))
            .on_scroll_wheel(cx.listener(Self::pointer_wheel))
            .on_key_down(cx.listener(Self::key_down))
            .on_key_up(cx.listener(Self::key_up))
            .child(element)
            .children(self.render_search(cx))
    }
}

fn modifiers(stroke: &Keystroke) -> Modifiers {
    Modifiers {
        shift: stroke.modifiers.shift,
        control: stroke.modifiers.control,
        alt: stroke.modifiers.alt,
        super_key: stroke.modifiers.platform,
        ..Modifiers::default()
    }
}

fn key_event(stroke: &Keystroke) -> Option<KeyEvent> {
    let code = match stroke.key.as_str() {
        "backspace" => KeyCode::BACKSPACE,
        "enter" | "return" => KeyCode::ENTER,
        "tab" => KeyCode::TAB,
        "down" => KeyCode::ARROW_DOWN,
        "left" => KeyCode::ARROW_LEFT,
        "right" => KeyCode::ARROW_RIGHT,
        "up" => KeyCode::ARROW_UP,
        "escape" => KeyCode::ESCAPE,
        "delete" => KeyCode::from_code(68).expect("pinned Delete"),
        "end" => KeyCode::from_code(69).expect("pinned End"),
        "home" => KeyCode::from_code(71).expect("pinned Home"),
        "pagedown" => KeyCode::from_code(73).expect("pinned PageDown"),
        "pageup" => KeyCode::from_code(74).expect("pinned PageUp"),
        name => {
            if let Some(number) = name
                .strip_prefix('f')
                .and_then(|n| n.parse::<u16>().ok())
                .filter(|n| (1..=25).contains(n))
            {
                KeyCode::from_code(120 + number).expect("pinned function key")
            } else {
                KeyCode::UNIDENTIFIED
            }
        }
    };
    let mut key = KeyEvent::new(code);
    key.modifiers = modifiers(stroke);
    if code == KeyCode::UNIDENTIFIED {
        key.text = stroke
            .key_char
            .clone()
            .filter(|text| !text.is_empty() && !text.chars().any(char::is_control))
            .or_else(|| (stroke.key.chars().count() == 1).then(|| stroke.key.clone()))
            .or_else(|| (stroke.key == "space").then(|| " ".to_string()))?;
        // GPUI supplies layout text, not a physical key or an unshifted codepoint.
    }
    Some(key)
}

#[cfg(test)]
#[path = "native_surface_tests.rs"]
pub(crate) mod tests;
