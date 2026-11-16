//! Mouse group: mouse wheel and right-click context-menu behavior.

use serde::{Deserialize, Serialize};

/// Who owns a native terminal left drag while an application reports the mouse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MouseDragPriority {
    /// Preserve application mouse controls; Shift forces terminal selection.
    #[default]
    TuiMouse,
    /// Select terminal text without holding Shift.
    Selection,
}

/// Mouse group.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MouseConfig {
    /// Show OneTerm's context menu on right click.
    ///
    /// Default `true`: right click opens the terminal panel context menu.
    /// Disable this to let CLI apps receive right click directly.
    pub show_context_menu: bool,
    /// Copy the selection to the system clipboard as soon as the mouse button
    /// is released (X11 primary-selection habit).
    ///
    /// Default `true` (the historical behaviour). Disable it to keep the
    /// clipboard untouched until an explicit Copy (SEC-10).
    pub copy_on_select: bool,
    /// Native left-drag ownership. Defaults to TUI mouse controls.
    pub drag_priority: MouseDragPriority,
    /// Paste the clipboard on a middle-button click (IN-0024).
    ///
    /// Default `true`. A program in mouse mode receives the click instead
    /// unless Shift is held; when disabled the middle button never pastes.
    pub middle_click_paste: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drag_priority_defaults_and_both_values_roundtrip() {
        let old: MouseConfig = serde_json::from_str(r#"{"copy_on_select":false}"#).unwrap();
        assert_eq!(old.drag_priority, MouseDragPriority::TuiMouse);
        assert!(!old.copy_on_select);
        for (value, priority) in [
            ("tui_mouse", MouseDragPriority::TuiMouse),
            ("selection", MouseDragPriority::Selection),
        ] {
            let config: MouseConfig =
                serde_json::from_value(serde_json::json!({"drag_priority": value})).unwrap();
            assert_eq!(config.drag_priority, priority);
            assert_eq!(
                serde_json::to_value(&config).unwrap()["drag_priority"],
                value
            );
            let settings = crate::TerminalSettings::from_config(&crate::TerminalConfig {
                mouse: config,
                ..Default::default()
            });
            assert_eq!(settings.drag_priority, priority);
            assert_eq!(settings.to_config().mouse.drag_priority, priority);
        }
    }

    #[test]
    fn right_click_context_menu_defaults_to_enabled() {
        assert!(MouseConfig::default().show_context_menu);
        let config: MouseConfig = serde_json::from_str("{}").unwrap();
        assert!(config.show_context_menu);
    }

    #[test]
    fn copy_on_select_defaults_to_enabled_and_can_be_disabled() {
        assert!(MouseConfig::default().copy_on_select);
        let config: MouseConfig = serde_json::from_str("{}").unwrap();
        assert!(config.copy_on_select);
        let config: MouseConfig = serde_json::from_str(r#"{ "copy_on_select": false }"#).unwrap();
        assert!(!config.copy_on_select);
    }

    #[test]
    fn middle_click_paste_defaults_to_enabled_and_can_be_disabled() {
        assert!(MouseConfig::default().middle_click_paste);
        let config: MouseConfig = serde_json::from_str("{}").unwrap();
        assert!(config.middle_click_paste);
        let config: MouseConfig =
            serde_json::from_str(r#"{ "middle_click_paste": false }"#).unwrap();
        assert!(!config.middle_click_paste);
    }

    #[test]
    fn right_click_context_menu_can_be_disabled_explicitly() {
        let config: MouseConfig =
            serde_json::from_str(r#"{ "show_context_menu": false }"#).unwrap();
        assert!(!config.show_context_menu);
    }
}

impl Default for MouseConfig {
    fn default() -> Self {
        Self {
            show_context_menu: true,
            copy_on_select: true,
            drag_priority: MouseDragPriority::default(),
            middle_click_paste: true,
        }
    }
}
