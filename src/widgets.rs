use std::ops::Not;

use eframe::egui::{Response, RichText, Ui};
use egui_taffy::taffy::prelude::{auto, length, percent};
use egui_taffy::taffy::{self, Style};
use egui_taffy::{TuiBuilderLogic, tui};

pub fn prefix_widget(ui: &mut Ui, name: &str, code: &str, selected: bool) -> Response {
    let active_fill = ui.visuals().extreme_bg_color;
    let button_width = ui.available_width() - ui.spacing().item_spacing.x - 12.;
    tui(ui, ui.id().with(name).with(code))
        .reserve_available_width()
        .style(Style {
            flex_direction: taffy::FlexDirection::Column,
            min_size: taffy::Size {
                width: length(button_width),
                height: auto(),
            },
            align_items: Some(taffy::AlignItems::Stretch),
            max_size: percent(1.),
            margin: length(6.),
            gap: length(3.),
            ..Default::default()
        })
        .show(|tui| {
            tui.selectable(selected, |tui| {
                tui.style(Style {
                    flex_direction: taffy::FlexDirection::Column,
                    min_size: taffy::Size {
                        width: length(button_width),
                        height: auto(),
                    },
                    max_size: percent(1.),
                    margin: length(3.),
                    gap: length(3.),
                    ..Default::default()
                })
                .add(|tui| {
                    if selected {
                        let vis = tui.egui_ui_mut().visuals_mut();
                        vis.widgets.noninteractive.fg_stroke = vis.widgets.active.fg_stroke;
                    }
                    let labels = [
                        tui.label(RichText::new(name).size(18.)),
                        tui.label(RichText::new(code).size(11.)),
                    ];
                });
            })
            .response
        })
}
