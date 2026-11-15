//! Workspace cursor settings; native visibility and steady mode remain authoritative.

use std::time::Duration;

use gpui::{App, Context, Task};
use oneterm_settings::TerminalBlink;

use super::NativeTerminalSurface;
use crate::render::cursor::CursorConfig;
use crate::terminal_view::cursor_shape_from_setting;

pub(super) struct Cursor {
    pub(super) focused: bool,
    visible: bool,
    task: Option<Task<()>>,
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::Ordering;

    use gpui::{AppContext as _, TestAppContext, px};
    use oneterm_settings::TerminalCursorShape;
    use oneterm_terminal::ghostty_bridge::{AsyncEngine, Command};

    use super::*;
    use crate::native_surface::tests::{Endpoint, call, color_surface, wait_colors};
    use crate::render::{
        cursor::resolve,
        frame::{CursorShape, Frame},
    };

    #[gpui::test]
    fn native_cursor_live_settings_focus_and_native_blink_policy(cx: &mut TestAppContext) {
        let (surface, window, endpoint, _, settings) = color_surface(cx, false);
        wait_colors(window, || {
            endpoint.color_completed.load(Ordering::Relaxed) == 1
        });
        let engine = endpoint.selection_engine.as_ref().unwrap();
        call(engine, Command::Feed(b"\x1b[1 q".to_vec()));
        call(engine, Command::Capture);
        surface.update(window, |surface, cx| {
            surface.cursor_focus(true, cx);
            surface.cursor_tick(cx);
            assert!(!surface.state.borrow().inputs.cursor.blink_visible);
            surface.cursor_tick(cx);
            assert!(surface.state.borrow().inputs.cursor.blink_visible);
            surface.cursor_focus(false, cx);
            surface.cursor_tick(cx);
            assert!(surface.state.borrow().inputs.cursor.blink_visible);
            surface.cursor_focus(true, cx);
        });
        settings.update(window, |settings, cx| {
            settings.cursor_shape = TerminalCursorShape::Underline;
            settings.cursor_color = Some(gpui::rgb(0xabcdef).into());
            settings.cursor_blink = TerminalBlink::Off;
            cx.notify();
        });
        window.run_until_parked();
        surface.update(window, |surface, cx| {
            surface.cursor_focus(true, cx);
            surface.cursor_tick(cx);
            let cursor = surface.state.borrow().inputs.cursor;
            assert_eq!(cursor.shape, Some(CursorShape::Underline));
            assert_eq!(cursor.color, Some(gpui::rgb(0xabcdef).into()));
            assert!(cursor.blink_visible);
        });
        surface.update_in(window, |surface, window, _| {
            let mut state = surface.state.borrow_mut();
            state.frame.snapshot_native(endpoint.as_ref());
            let geometry = state
                .geometry
                .expect("bound surface completed its initial layout");
            let painted = state.resolve_cursor(&geometry, window).unwrap();
            assert_eq!(
                painted.color,
                gpui::rgb(0xabcdef).into(),
                "explicit setting wins in the actual cursor painter"
            );
            assert_eq!(painted.shape, CursorShape::Underline);
        });
        settings.update(window, |settings, cx| {
            settings.cursor_shape = TerminalCursorShape::Bar;
            settings.cursor_color = None;
            settings.cursor_blink = TerminalBlink::On;
            cx.notify();
        });
        window.run_until_parked();
        call(engine, Command::Feed(b"\x1b[2 q".to_vec()));
        call(engine, Command::Capture);
        surface.update(window, |surface, cx| {
            surface.cursor_focus(true, cx);
            surface.cursor_tick(cx);
            assert!(
                surface.state.borrow().inputs.cursor.blink_visible,
                "native steady cursor stays visible"
            );
            let mut frame = Frame::new();
            frame.snapshot_native(endpoint.as_ref());
            let config = surface.state.borrow().inputs.cursor;
            assert_eq!(config.color, None);
            assert_eq!(resolve(&frame, &config).unwrap().shape, CursorShape::Beam);
        });
        call(engine, Command::Feed(b"\x1b[?25l".to_vec()));
        call(engine, Command::Capture);
        surface.update(window, |surface, cx| {
            surface.cursor_tick(cx);
            let mut frame = Frame::new();
            frame.snapshot_native(endpoint.as_ref());
            assert!(
                resolve(&frame, &surface.state.borrow().inputs.cursor).is_none(),
                "settings never unhide a program-hidden cursor"
            );
        });
    }

    #[gpui::test]
    fn native_cursor_timer_drives_bound_focused_phase(cx: &mut TestAppContext) {
        let (_host, window, endpoint, _, settings) = color_surface(cx, false);
        wait_colors(window, || {
            endpoint.color_completed.load(Ordering::Relaxed) == 1
        });
        let engine = endpoint.selection_engine.as_ref().unwrap();
        call(engine, Command::Feed(b"\x1b[1 q".to_vec()));
        call(engine, Command::Capture);
        // Test windows do not report OS focus. Keep this surface unhosted so
        // render cannot overwrite the explicit focus used to prove scheduling.
        let source = endpoint.clone();
        let surface = window.update(|window, cx| {
            cx.new(|cx| {
                NativeTerminalSurface::new(
                    source,
                    gpui::font("monospace"),
                    px(14.),
                    |_, _| {},
                    window,
                    cx,
                )
            })
        });
        surface.update(window, |surface, cx| {
            surface.bind_settings(settings, cx);
            surface.cursor_focus(true, cx);
        });
        window.run_until_parked();
        assert!(surface.read_with(window, |surface, _| surface.cursor.visible));
        for _ in 0..650 {
            window.executor().advance_clock(Duration::from_millis(1));
            window.run_until_parked();
            if !surface.read_with(window, |surface, _| surface.cursor.visible) {
                break;
            }
        }
        assert!(
            !surface.read_with(window, |surface, _| surface.cursor.visible),
            "registered timer must drive the phase without a manual tick"
        );
    }

    #[gpui::test]
    fn native_cursor_timer_uses_weak_surface_and_cancels_on_release(cx: &mut TestAppContext) {
        let (surface, window, endpoint, _, _) = color_surface(cx, false);
        wait_colors(window, || {
            endpoint.color_completed.load(Ordering::Relaxed) == 1
        });
        let weak_surface = surface.downgrade();
        let weak_session = Arc::downgrade(&endpoint);
        window.update(|window, _| window.remove_window());
        window.cx.update(|_| drop(surface));
        drop(endpoint);
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while weak_session.upgrade().is_some() {
            window.executor().advance_clock(Duration::from_millis(500));
            window.run_until_parked();
            assert!(
                std::time::Instant::now() < deadline,
                "cursor timer retained native session"
            );
        }
        assert!(weak_surface.upgrade().is_none());
    }

    #[gpui::test]
    fn native_cursor_unbound_surface_keeps_native_appearance_and_has_no_timer(
        cx: &mut TestAppContext,
    ) {
        cx.executor().allow_parking();
        cx.update(gpui_component::init);
        let engine = AsyncEngine::spawn(16, 3).unwrap();
        call(&engine, Command::Feed(b"\x1b[6 q".to_vec()));
        call(&engine, Command::Capture);
        let endpoint = Arc::new(Endpoint::new(engine.model()));
        let source = endpoint.clone();
        let (surface, window) = cx.add_window_view(move |window, cx| {
            NativeTerminalSurface::new(
                source,
                gpui::font("monospace"),
                px(14.),
                |_, _| {},
                window,
                cx,
            )
        });
        surface.update(window, |surface, cx| {
            surface.cursor_focus(true, cx);
            surface.cursor_tick(cx);
            assert!(surface.cursor.task.is_none());
            let config = surface.state.borrow().inputs.cursor;
            assert_eq!(config.shape, None);
            assert_eq!(config.color, None);
            let mut frame = Frame::new();
            frame.snapshot_native(endpoint.as_ref());
            assert_eq!(resolve(&frame, &config).unwrap().shape, CursorShape::Beam);
        });
    }
}

impl Default for Cursor {
    fn default() -> Self {
        Self {
            focused: false,
            visible: true,
            task: None,
        }
    }
}

impl NativeTerminalSurface {
    pub(super) fn bind_cursor(&mut self, cx: &mut Context<Self>) {
        if self.cursor.task.is_some() {
            return;
        }
        self.cursor.task = Some(cx.spawn(async move |surface, cx| {
            loop {
                let wait = oneterm_state::until_next_tick(Duration::from_millis(500));
                cx.background_executor().timer(wait).await;
                if surface
                    .update(cx, |surface, cx| surface.cursor_tick(cx))
                    .is_err()
                {
                    break;
                }
            }
        }));
    }

    pub(super) fn cursor_focus(&mut self, focused: bool, cx: &mut Context<Self>) {
        self.cursor.focused = focused;
        self.cursor.visible = true;
        self.refresh_cursor(cx);
        cx.notify();
    }

    pub(super) fn refresh_cursor(&mut self, cx: &App) {
        let Some(settings) = &self.settings else {
            return;
        };
        let settings = settings.read(cx);
        if settings.cursor_blink == TerminalBlink::Off || !self.cursor.focused {
            self.cursor.visible = true;
        }
        self.state.borrow_mut().inputs.cursor = CursorConfig {
            shape: Some(cursor_shape_from_setting(settings.cursor_shape)),
            color: settings.cursor_color,
            focused: self.cursor.focused,
            blink_visible: self.cursor.visible,
        };
    }

    fn cursor_tick(&mut self, cx: &mut Context<Self>) {
        let enabled = self
            .settings
            .as_ref()
            .is_some_and(|settings| settings.read(cx).cursor_blink == TerminalBlink::On);
        let query = self.session.query();
        let blinking = matches!(
            query.state,
            oneterm_terminal::ghostty_bridge::State::Running
        ) && query.viewport.is_some_and(|viewport| {
            let cursor = viewport.cursor();
            cursor.blinking && cursor.visible && cursor.position.is_some()
        });
        if !enabled || !self.cursor.focused || !blinking {
            self.cursor.visible = true;
            self.refresh_cursor(cx);
            return;
        }
        self.cursor.visible = !self.cursor.visible;
        self.refresh_cursor(cx);
        cx.notify();
    }
}
