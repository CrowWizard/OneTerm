//! Composition belongs to the native session, never the legacy document model.

use std::ops::Range;

use gpui::{Bounds, Context, EntityInputHandler, Pixels, Point, UTF16Selection, Window, size};

use super::{NativeSurfaceError, NativeTerminalSurface};

impl NativeTerminalSurface {
    fn accepts_composition(&self) -> bool {
        self.session.query().viewport.is_some()
    }
}

impl EntityInputHandler for NativeTerminalSurface {
    fn text_for_range(
        &mut self,
        _: Range<usize>,
        _: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        None
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        self.accepts_composition().then_some(UTF16Selection {
            range: 0..0,
            reversed: false,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.session
            .marked_text()
            .map(|t| 0..t.encode_utf16().count())
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.session.clear_marked_text();
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.accepts_composition() {
            return;
        }
        if self.channels.bound() {
            self.queue_channel_text(text, cx);
            cx.notify();
            return;
        }
        if let Err(error) = self.session.commit_text(text) {
            (self.on_error)(NativeSurfaceError::Composition(error), cx);
        }
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        text: &str,
        _: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.accepts_composition() {
            return;
        }
        if text.is_empty() {
            self.session.clear_marked_text();
            cx.notify();
            return;
        }
        if let Err(error) = self.session.set_marked_text(text.to_string()) {
            (self.on_error)(NativeSurfaceError::Composition(error), cx);
        }
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let geometry = self.state.borrow().geometry?;
        let query = self.session.query();
        let viewport = query.viewport?;
        let (col, row) = viewport.cursor().position?;
        if col >= geometry.size.cols || row >= geometry.size.rows {
            return None;
        }
        // Conservative anchor: no claim that UTF-16 units equal native cells.
        Some(Bounds::new(
            geometry.cell_origin(usize::from(row), usize::from(col)),
            size(geometry.metrics.cell_width, geometry.metrics.line_height),
        ))
    }

    fn character_index_for_point(
        &mut self,
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }

    fn accepts_text_input(&self, _: &mut Window, _: &mut Context<Self>) -> bool {
        self.accepts_composition()
    }
}
