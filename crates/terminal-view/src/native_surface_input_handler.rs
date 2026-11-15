//! Platform IME state may outlive the window root; it must not own the session.

use gpui::{
    App, Bounds, ClipboardItem, ElementInputHandler, InputHandler, Pixels, Point,
    TextInputConfiguration, UTF16Selection, WeakEntity, Window,
};
use std::ops::Range;

use super::NativeTerminalSurface;

pub(super) struct WeakInputHandler {
    view: WeakEntity<NativeTerminalSurface>,
    bounds: Bounds<Pixels>,
}

impl WeakInputHandler {
    pub(super) fn new(view: WeakEntity<NativeTerminalSurface>, bounds: Bounds<Pixels>) -> Self {
        Self { view, bounds }
    }

    fn with<R: Default>(
        &self,
        f: impl FnOnce(&mut ElementInputHandler<NativeTerminalSurface>) -> R,
    ) -> R {
        self.view
            .upgrade()
            .map(|view| f(&mut ElementInputHandler::new(self.bounds, view)))
            .unwrap_or_default()
    }
}

impl InputHandler for WeakInputHandler {
    fn selected_text_range(
        &mut self,
        disabled: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<UTF16Selection> {
        self.with(|h| h.selected_text_range(disabled, window, cx))
    }
    fn marked_text_range(&mut self, window: &mut Window, cx: &mut App) -> Option<Range<usize>> {
        self.with(|h| h.marked_text_range(window, cx))
    }
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<String> {
        self.with(|h| h.text_for_range(range, adjusted, window, cx))
    }
    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.with(|h| h.replace_text_in_range(range, text, window, cx));
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.with(|h| h.replace_and_mark_text_in_range(range, text, selected, window, cx));
    }
    fn unmark_text(&mut self, window: &mut Window, cx: &mut App) {
        self.with(|h| h.unmark_text(window, cx));
    }
    fn paste(&mut self, item: ClipboardItem, window: &mut Window, cx: &mut App) {
        self.with(|h| h.paste(item, window, cx));
    }
    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Bounds<Pixels>> {
        self.with(|h| h.bounds_for_range(range, window, cx))
    }
    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<usize> {
        self.with(|h| h.character_index_for_point(point, window, cx))
    }
    fn set_selected_text_range(&mut self, range: Range<usize>, window: &mut Window, cx: &mut App) {
        self.with(|h| h.set_selected_text_range(range, window, cx));
    }
    fn element_bounds(&mut self, window: &mut Window, cx: &mut App) -> Option<Bounds<Pixels>> {
        self.with(|h| h.element_bounds(window, cx))
    }
    fn text_length_utf16(&mut self, window: &mut Window, cx: &mut App) -> Option<usize> {
        self.with(|h| h.text_length_utf16(window, cx))
    }
    fn accepts_text_input(&mut self, window: &mut Window, cx: &mut App) -> bool {
        self.with(|h| h.accepts_text_input(window, cx))
    }
    fn prefers_ime_for_printable_keys(&mut self, window: &mut Window, cx: &mut App) -> bool {
        self.with(|h| h.prefers_ime_for_printable_keys(window, cx))
    }
    fn text_input_configuration(
        &mut self,
        window: &mut Window,
        cx: &mut App,
    ) -> TextInputConfiguration {
        self.with(|h| h.text_input_configuration(window, cx))
    }
    fn text_input_editable_range(
        &mut self,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Range<usize>> {
        self.with(|h| h.text_input_editable_range(window, cx))
    }
}
