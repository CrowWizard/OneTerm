//! Pointer geometry/pairing only; terminal protocols belong to the native owner.

use gpui::{
    Context, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, Window,
};
use oneterm_terminal::ghostty::input::{
    Geometry, Modifiers, MouseAction, MouseButton as NativeButton, MouseEvent,
};

use super::{NativeSurfaceError, NativeTerminalSurface};
use crate::render::metrics::GridGeometry;

#[derive(Default)]
pub(super) struct Pointer {
    // At most the three supported GPUI buttons; each retains its last valid
    // geometry so a release outside/after resize cannot invent coordinates.
    held: Vec<(MouseButton, MouseEvent)>,
    wheel_remainder: f32,
}

impl NativeTerminalSurface {
    pub(super) fn pointer_wheel(
        &mut self,
        event: &gpui::ScrollWheelEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(pointer) =
            self.pointer_event(event.position, event.modifiers, MouseAction::Motion, None)
        else {
            self.pointer.wheel_remainder = 0.0;
            return;
        };
        let Some(geometry) = self.state.borrow().geometry else {
            return;
        };
        let lines =
            crate::input::mouse::wheel_lines(event.delta, geometry.metrics.line_height, 1.0);
        if lines.is_finite() && lines != 0.0 {
            // Sub-line input belongs to this surface even before a whole step
            // is admitted; do not also scroll an enclosing container.
            cx.stop_propagation();
        }
        let Some(steps) = accumulate_wheel(&mut self.pointer.wheel_remainder, lines) else {
            return;
        };
        let wheel = oneterm_terminal::ghostty::wheel::WheelEvent { steps, pointer };
        if let Err(error) = self.session.wheel(wheel) {
            (self.on_error)(NativeSurfaceError::Control(error), cx);
        }
        cx.stop_propagation();
    }
    fn pointer_event(
        &self,
        position: Point<Pixels>,
        modifiers: gpui::Modifiers,
        action: MouseAction,
        button: Option<NativeButton>,
    ) -> Option<MouseEvent> {
        let geometry = self.state.borrow().geometry?;
        let viewport = self.session.query().viewport?;
        if viewport.columns() != geometry.size.cols
            || viewport.lines() != usize::from(geometry.size.rows)
        {
            return None;
        }
        event_at(
            geometry,
            position,
            modifiers,
            action,
            button,
            !self.pointer.held.is_empty(),
        )
    }

    pub(super) fn pointer_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus.focus(window, cx);
        if event.button == MouseButton::Right
            && self
                .settings
                .as_ref()
                .is_some_and(|settings| settings.read(cx).show_context_menu)
        {
            // The enclosing pane owns this press; never also send it to the PTY.
            return;
        }
        if self
            .pointer
            .held
            .iter()
            .any(|(held, _)| held == &event.button)
        {
            return;
        }
        if event.button == MouseButton::Left
            && self.selection_down(event.position, event.modifiers.shift, cx)
        {
            cx.stop_propagation();
            return;
        }
        if event.modifiers.shift {
            return;
        }
        let Some(button) = button(event.button) else {
            return;
        };
        // Duplicate down events must not lose an existing release obligation.
        if self
            .pointer
            .held
            .iter()
            .any(|(held, _)| held == &event.button)
        {
            return;
        }
        let Some(mut mouse) = self.pointer_event(
            event.position,
            event.modifiers,
            MouseAction::Press,
            Some(button),
        ) else {
            return;
        };
        mouse.any_button_pressed = true;
        match self.session.mouse(mouse) {
            Ok(()) => self.pointer.held.push((event.button, mouse)),
            Err(error) => (self.on_error)(NativeSurfaceError::Control(error), cx),
        }
        cx.stop_propagation();
    }

    pub(super) fn pointer_move(
        &mut self,
        event: &MouseMoveEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // GPUI may report a departure release as a later no-button motion.
        if self.selection_move(event.position, event.pressed_button) {
            cx.stop_propagation();
            return;
        }
        if event.pressed_button.is_none() && !self.pointer.held.is_empty() {
            self.release_pointer(cx);
        }
        if event.modifiers.shift {
            return;
        }
        let owned = event
            .pressed_button
            .filter(|pressed| self.pointer.held.iter().any(|(held, _)| held == pressed));
        let Some(mouse) = self.pointer_event(
            event.position,
            event.modifiers,
            MouseAction::Motion,
            owned.and_then(button),
        ) else {
            return;
        };
        match self.session.mouse(mouse) {
            Ok(()) => {
                for (_, held) in &mut self.pointer.held {
                    held.x = mouse.x;
                    held.y = mouse.y;
                    held.geometry = mouse.geometry;
                }
            }
            Err(error) => (self.on_error)(NativeSurfaceError::Control(error), cx),
        }
    }

    pub(super) fn pointer_up(
        &mut self,
        event: &MouseUpEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.button == MouseButton::Left
            && self.selection_move(event.position, Some(MouseButton::Left))
        {
            self.finish_selection();
            cx.stop_propagation();
        }
        let Some(index) = self
            .pointer
            .held
            .iter()
            .position(|(held, _)| held == &event.button)
        else {
            return;
        };
        let (_, previous) = self.pointer.held.remove(index);
        let mut mouse = self
            .pointer_event(
                event.position,
                event.modifiers,
                MouseAction::Release,
                previous.button,
            )
            .unwrap_or(previous);
        mouse.action = MouseAction::Release;
        mouse.modifiers = mouse_modifiers(event.modifiers);
        mouse.any_button_pressed = !self.pointer.held.is_empty();
        if let Err(error) = self.session.mouse(mouse) {
            (self.on_error)(NativeSurfaceError::Control(error), cx);
        }
        // Release an admitted press even when Shift now belongs to selection.
        cx.stop_propagation();
    }

    pub(super) fn release_pointer(&mut self, cx: &mut Context<Self>) {
        self.pointer.wheel_remainder = 0.0;
        self.finish_selection();
        while let Some((_, mut mouse)) = self.pointer.held.pop() {
            mouse.action = MouseAction::Release;
            mouse.modifiers = Modifiers::default();
            mouse.any_button_pressed = !self.pointer.held.is_empty();
            if let Err(error) = self.session.mouse(mouse) {
                (self.on_error)(NativeSurfaceError::Control(error), cx);
            }
        }
    }
}

fn button(button: MouseButton) -> Option<NativeButton> {
    match button {
        MouseButton::Left => Some(NativeButton::Left),
        MouseButton::Middle => Some(NativeButton::Middle),
        MouseButton::Right => Some(NativeButton::Right),
        MouseButton::Navigate(_) => None,
    }
}

fn accumulate_wheel(remainder: &mut f32, lines: f32) -> Option<i32> {
    let total = *remainder + lines;
    if !total.is_finite() {
        *remainder = 0.0;
        return None;
    }
    *remainder = total.fract();
    let limit = oneterm_terminal::ghostty::wheel::MAX_STEPS as f32;
    let steps = total.trunc().clamp(-limit, limit) as i32;
    (steps != 0).then_some(steps)
}

fn mouse_modifiers(modifiers: gpui::Modifiers) -> Modifiers {
    Modifiers {
        shift: modifiers.shift,
        control: modifiers.control,
        alt: modifiers.alt,
        super_key: modifiers.platform,
        ..Modifiers::default()
    }
}

fn event_at(
    geometry: GridGeometry,
    position: Point<Pixels>,
    modifiers: gpui::Modifiers,
    action: MouseAction,
    button: Option<NativeButton>,
    pressed: bool,
) -> Option<MouseEvent> {
    let (row, col) = geometry.pixel_to_grid(position)?;
    if !row.is_finite() || !col.is_finite() {
        return None;
    }
    let cell_width = u32::try_from(geometry.metrics.device.w).ok()?;
    let cell_height = u32::try_from(geometry.metrics.device.h).ok()?;
    let mouse = MouseEvent {
        action,
        button,
        modifiers: mouse_modifiers(modifiers),
        x: col * cell_width as f32,
        y: row * cell_height as f32,
        any_button_pressed: pressed,
        geometry: Geometry {
            width: u32::from(geometry.size.cols).checked_mul(cell_width)?,
            height: u32::from(geometry.size.rows).checked_mul(cell_height)?,
            cell_width,
            cell_height,
            padding_top: 0,
            padding_bottom: 0,
            padding_left: 0,
            padding_right: 0,
        },
    };
    mouse.validate().ok()?;
    Some(mouse)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::metrics::CellMetrics;

    #[test]
    fn wheel_fractional_accumulation_is_bounded_and_does_not_replay_overflow() {
        let mut remainder = 0.0;
        assert_eq!(accumulate_wheel(&mut remainder, 0.25), None);
        assert_eq!(accumulate_wheel(&mut remainder, 0.5), None);
        assert_eq!(accumulate_wheel(&mut remainder, 0.5), Some(1));
        assert_eq!(remainder, 0.25);
        assert_eq!(accumulate_wheel(&mut remainder, -1.5), Some(-1));
        assert_eq!(remainder, -0.25);
        assert_eq!(accumulate_wheel(&mut remainder, 100.5), Some(10));
        assert_eq!(remainder, 0.25);
        assert_eq!(accumulate_wheel(&mut remainder, 0.0), None);
        assert_eq!(accumulate_wheel(&mut remainder, f32::NAN), None);
        assert_eq!(remainder, 0.0);
        assert_eq!(accumulate_wheel(&mut remainder, f32::NEG_INFINITY), None);
    }
    use gpui::{Bounds, Edges, point, px, size};

    #[test]
    fn pointer_device_geometry_excludes_padding_and_invalid_coordinates() {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let metrics = CellMetrics::snapped(8.0, 16.0, 10.0, 3.0, 5.0, scale);
            let geometry = GridGeometry::new(
                Bounds::new(point(px(20.0), px(30.0)), size(px(240.0), px(160.0))),
                metrics,
                px(0.0),
                Edges::all(px(4.0)),
            );
            let position = point(
                geometry.origin.x + geometry.metrics.cell_width * 2.5,
                geometry.origin.y + geometry.metrics.line_height * 1.25,
            );
            let mouse = event_at(
                geometry,
                position,
                gpui::Modifiers::default(),
                MouseAction::Motion,
                None,
                false,
            )
            .unwrap();
            assert_eq!(mouse.x, metrics.device.w as f32 * 2.5);
            assert_eq!(mouse.y, metrics.device.h as f32 * 1.25);
            assert_eq!(
                mouse.geometry.width,
                u32::from(geometry.size.cols) * metrics.device.w as u32
            );
            assert_eq!(
                mouse.geometry.height,
                u32::from(geometry.size.rows) * metrics.device.h as u32
            );
            assert_eq!(mouse.geometry.padding_left, 0);
            mouse.validate().unwrap();
            for position in [
                geometry.bounds.origin,
                geometry.cell_origin(0, usize::from(geometry.size.cols)),
                geometry.cell_origin(usize::from(geometry.size.rows), 0),
                point(px(f32::NAN), px(40.0)),
                point(px(f32::INFINITY), px(40.0)),
            ] {
                assert!(
                    event_at(
                        geometry,
                        position,
                        gpui::Modifiers::default(),
                        MouseAction::Motion,
                        None,
                        false
                    )
                    .is_none()
                );
            }
        }
    }
}
