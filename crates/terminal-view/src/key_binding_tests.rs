//! Terminal key scope must not depend on constructing a legacy view.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    AppContext as _, Context, FocusHandle, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, Styled as _, TestAppContext, Window, div,
};

struct TabTargets {
    terminal: FocusHandle,
    first: FocusHandle,
    second: FocusHandle,
    keys: Rc<RefCell<Vec<String>>>,
}

impl Render for TabTargets {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let keys = self.keys.clone();
        div()
            .tab_group()
            .size_full()
            .child(
                div()
                    .size_8()
                    .track_focus(&self.terminal)
                    .key_context("Terminal")
                    .on_key_down(move |event, _, cx| {
                        keys.borrow_mut().push(event.keystroke.key.clone());
                        cx.stop_propagation();
                    }),
            )
            .child(div().size_8().track_focus(&self.first))
            .child(div().size_8().track_focus(&self.second))
    }
}

#[gpui::test]
fn terminal_tab_binding_is_shared_but_form_traversal_is_preserved(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    cx.update(super::init);
    let keys = Rc::new(RefCell::new(Vec::new()));
    let received = keys.clone();
    let handles = Rc::new(RefCell::new(None));
    let created = handles.clone();
    let (_, cx) = cx.add_window_view(move |window, cx| {
        let targets = cx.new(|cx| TabTargets {
            terminal: cx.focus_handle(),
            first: cx.focus_handle().tab_stop(true).tab_index(0),
            second: cx.focus_handle().tab_stop(true).tab_index(1),
            keys: received,
        });
        let view = targets.read(cx);
        *created.borrow_mut() = Some((
            view.terminal.clone(),
            view.first.clone(),
            view.second.clone(),
        ));
        gpui_component::Root::new(targets, window, cx)
    });
    let (terminal, first, second) = handles.borrow_mut().take().unwrap();
    cx.update(|window, cx| {
        window.focus(&terminal, cx);
        window.draw(cx).clear(cx);
    });
    for chord in ["tab", "shift-tab"] {
        cx.simulate_keystrokes(chord);
        cx.update(|window, _| assert!(terminal.is_focused(window)));
    }
    assert_eq!(*keys.borrow(), ["tab", "tab"]);
    cx.update(|window, cx| {
        window.focus(&first, cx);
        window.draw(cx).clear(cx);
    });
    cx.simulate_keystrokes("tab");
    cx.update(|window, cx| {
        assert!(second.is_focused(window));
        window.draw(cx).clear(cx);
    });
    cx.simulate_keystrokes("shift-tab");
    cx.update(|window, _| assert!(first.is_focused(window)));
    assert_eq!(
        keys.borrow().len(),
        2,
        "form navigation sends no terminal input"
    );
}
