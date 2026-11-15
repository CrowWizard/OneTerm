//! Static workspace defaults are not effective program-controlled OSC colors.

use std::rc::Rc;

use gpui::Context;
use gpui_component::ActiveTheme as _;
use oneterm_terminal::ghostty::{colors::DefaultColors, render::Rgb};

use super::{NativeSurfaceError, NativeTerminalSurface};
use crate::theme::{apply_color_overrides, build_terminal_theme};

#[derive(Default)]
pub(super) struct Colors {
    desired: Option<DefaultColors>,
    applied: Option<DefaultColors>,
    busy: bool,
    failed: bool,
}

impl NativeTerminalSurface {
    pub(super) fn refresh_colors(&mut self, cx: &mut Context<Self>) {
        let Some(settings) = &self.settings else {
            return;
        };
        let theme = apply_color_overrides(
            build_terminal_theme(cx.theme()),
            &settings.read(cx).color_overrides,
        );
        let palette = &theme.palette;
        let native = |color: oneterm_terminal::Rgb| Rgb {
            r: color.r,
            g: color.g,
            b: color.b,
        };
        let defaults = DefaultColors {
            foreground: native(palette.foreground),
            background: native(palette.background),
            cursor: Some(native(palette.cursor)),
            palette: std::array::from_fn(|index| {
                native(oneterm_terminal::resolve_color(
                    &oneterm_terminal::Color::Palette(index as u8),
                    palette,
                ))
            }),
        };
        self.state.borrow_mut().inputs.theme = Rc::new(theme);
        self.colors.desired = Some(defaults.clone());
        if self.colors.busy || self.colors.failed || self.colors.applied.as_ref() == Some(&defaults)
        {
            return;
        }
        self.colors.busy = true;
        let session = self.session.clone();
        let executor = cx.background_executor().clone();
        let (sender, receiver) = async_channel::bounded(1);
        let initial = defaults.clone();
        executor
            .spawn(async move {
                let result = session.set_default_colors(initial).await;
                drop(session);
                if let Err(error) = &result {
                    log::error!("Native color publication failed: {error}");
                }
                if sender.send(result).await.is_err() {
                    log::debug!("Native color publication UI observer released");
                }
            })
            .detach();
        // Only result delivery needs GPUI. The admitted wait and session owner
        // finish independently even when shutdown stops foreground task polling.
        cx.spawn(async move |surface, cx| {
            let Ok(result) = receiver.recv().await else {
                log::error!("Native color publication result lost");
                return;
            };
            let updated = surface.update(cx, |surface, cx| {
                if let Err(error) = result {
                    surface.colors.busy = false;
                    surface.colors.failed = true;
                    (surface.on_error)(NativeSurfaceError::Colors(error), cx);
                    cx.notify();
                    return;
                }
                surface.colors.applied = Some(defaults.clone());
                cx.notify();
                surface.colors.busy = false;
                surface.refresh_colors(cx);
            });
            if let Err(error) = updated {
                log::debug!("Native color publication recipient closed: {error}");
            }
        })
        .detach();
    }
}
