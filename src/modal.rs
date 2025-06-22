use eframe::egui::{self, Color32, Image, Modal, ModalResponse, RichText, include_image};

#[derive(Debug, Clone)]
pub enum ModalLevel {
    Error,
    Warning,
    Info,
    Success,
}

#[derive(Debug, Clone)]
pub struct ModalDisplay {
    pub level: ModalLevel,
    pub title: String,
    pub content: String,
}

use egui::{Context, FontId, TextStyle};

fn get_font_size(ctx: &Context, text_style: TextStyle) -> f32 {
    let style = ctx.style();
    let font_id: &FontId = style
        .text_styles
        .get(&text_style)
        .unwrap_or_else(|| style.text_styles.get(&TextStyle::Body).unwrap());
    font_id.size
}

pub fn show_modal(
    ctx: &egui::Context,
    msg: &ModalDisplay,
    mut on_close: impl FnMut(),
) -> ModalResponse<()> {
    Modal::new(format!("modal {}", msg.title).into()).show(ctx, |ui| {
        ui.horizontal(|ui| {
            let font_size = get_font_size(ctx, TextStyle::Body);
            ui.add(
                Image::new(match msg.level {
                    ModalLevel::Info => include_image!("../assets/icons/info.svg"),
                    ModalLevel::Warning => include_image!("../assets/icons/triangle-alert.svg"),
                    ModalLevel::Error => include_image!("../assets/icons/circle-x.svg"),
                    ModalLevel::Success => include_image!("../assets/icons/check.svg"),
                })
                .tint(match msg.level {
                    ModalLevel::Info => match ui.visuals().dark_mode {
                        true => Color32::from_rgb(100, 100, 255),
                        false => Color32::from_rgb(0, 75, 255),
                    },
                    ModalLevel::Warning => ui.visuals().warn_fg_color,
                    ModalLevel::Error => ui.visuals().error_fg_color,
                    ModalLevel::Success => match ui.visuals().dark_mode {
                        true => Color32::from_rgb(72, 240, 72),
                        false => Color32::from_rgb(0, 132, 21),
                    },
                })
                .maintain_aspect_ratio(true)
                .max_height(font_size)
                .fit_to_fraction([1.0, 1.0].into()),
            );
            ui.label(RichText::new(&msg.title));
        });
        ui.separator();
        ui.label(RichText::new(&msg.content));
        ui.separator();

        ui.with_layout(
            egui::Layout::top_down_justified(egui::Align::Center),
            |ui| {
                if ui.button("close").clicked() {
                    on_close();
                }
            },
        );
    })
}
