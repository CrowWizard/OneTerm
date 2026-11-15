//! Shared edge-only focus policy: theme paths, decoration and real controls.

use gpui::{
    AppContext as _, Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, Styled as _, TestAppContext, Window, div, px,
};
use gpui_component::{
    ActiveTheme as _, Root, Theme, ThemeMode, ThemeRegistry, ThemeStyled as _,
    button::Button,
    checkbox::Checkbox,
    combobox::{Combobox, ComboboxState},
    input::{Input, InputState, NumberInput},
    radio::Radio,
    searchable_list::SearchableVec,
    select::{Select, SelectState},
    switch::Switch,
};
use oneterm_settings::{UiConfig, ui_config::UiConfigGlobal};

fn init(cx: &mut gpui::App) {
    gpui_component::init(cx);
    let config = cx.new(|_| UiConfig {
        persist_blocked: true,
        ..UiConfig::default()
    });
    cx.set_global(UiConfigGlobal(config));
    super::init(cx);
}

#[gpui::test]
fn edge_only_focus_survives_every_theme_path(cx: &mut TestAppContext) {
    cx.update(|cx| {
        init(cx);
        assert!(!Theme::global(cx).focus_ring, "startup");
        for mode in [ThemeMode::Light, ThemeMode::Dark] {
            Theme::change(mode, None, cx);
            assert!(!Theme::global(cx).focus_ring, "mode change");
        }
        let configs: Vec<_> = ThemeRegistry::global(cx)
            .themes()
            .values()
            .cloned()
            .collect();
        for config in configs {
            Theme::global_mut(cx).apply_config(&config);
            assert!(!Theme::global(cx).focus_ring, "config {}", config.name);
            let palette = Theme::global(cx).colors;
            Theme::update(cx, |theme| theme.font_size = px(15.));
            assert!(!Theme::global(cx).focus_ring, "theme update");
            assert_eq!(
                Theme::global(cx).colors,
                palette,
                "focus policy does not recolor"
            );
            super::apply_list_style_override(cx);
            super::apply_warning_legibility(cx);
            assert!(!Theme::global(cx).focus_ring, "application overrides");
            assert_eq!(Theme::global(cx).list_active, Theme::global(cx).list_hover);
        }
    });
    let cx = cx.add_empty_window();
    cx.update(|window, cx| {
        let mut edge = div().border_1().focus_ring_style(window, cx);
        assert_eq!(edge.style().border_color, Some(cx.theme().ring));
        assert!(cx.theme().ring.a > 0., "focus remains visible");
    });
}

struct Controls {
    kind: usize,
    input: Entity<InputState>,
    combo: Entity<ComboboxState<SearchableVec<&'static str>>>,
    select: Entity<SelectState<Vec<&'static str>>>,
}

impl Render for Controls {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let control = match self.kind {
            0 => Input::new(&self.input).into_any_element(),
            1 => Combobox::new(&self.combo).into_any_element(),
            2 => Select::new(&self.select).into_any_element(),
            3 => Button::new("button").label("Connect").into_any_element(),
            4 => Checkbox::new("checkbox").label("Save").into_any_element(),
            5 => Radio::new("radio").label("Password").into_any_element(),
            6 => Switch::new("switch").label("Enabled").into_any_element(),
            7 => NumberInput::new(&self.input).into_any_element(),
            _ => unreachable!(),
        };
        div().id("controls").tab_group().w(px(400.)).child(control)
    }
}

#[gpui::test]
fn shared_controls_remain_keyboard_focusable(cx: &mut TestAppContext) {
    cx.update(init);
    for kind in 0..8 {
        let (_, cx) = cx.add_window_view(move |window, cx| {
            let controls = cx.new(|cx| Controls {
                kind,
                input: cx.new(|cx| InputState::new(window, cx)),
                combo: cx.new(|cx| {
                    ComboboxState::new(SearchableVec::new(vec!["SSH", "Local"]), vec![], window, cx)
                }),
                select: cx.new(|cx| SelectState::new(vec!["SSH", "Local"], None, window, cx)),
            });
            Root::new(controls, window, cx)
        });
        cx.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        cx.update(|window, cx| {
            window.focus_next(cx);
            assert!(
                window.focused(cx).is_some(),
                "control {kind} retains keyboard focus"
            );
            assert!(!cx.theme().focus_ring);
            window.draw(cx).clear(cx);
        });
    }
}
