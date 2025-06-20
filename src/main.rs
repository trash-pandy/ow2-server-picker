use std::collections::HashMap;

#[cfg(target_os = "linux")]
use ::{anyhow::ensure, clap::Parser, libc::geteuid};

use anyhow::{Context, Result, anyhow};
use eframe::NativeOptions;
use eframe::egui::{
    Align, CentralPanel, Layout, ScrollArea, TopBottomPanel, ViewportBuilder,
    global_theme_preference_switch, vec2,
};
use rfd::AsyncFileDialog;
use tokio::task::JoinHandle;

use crate::modal::{ModalDisplay, ModalLevel};

mod daemon;
mod fw;
mod modal;
mod prefixes;
mod util;
mod widgets;

fn main() -> Result<()> {
    #[cfg(target_os = "linux")]
    if let Ok(cli) = daemon::Cli::try_parse() {
        if cli.kill {
            daemon::kill().ok();
            return Ok(());
        }

        if cli.daemon {
            ensure!(unsafe { geteuid() == 0 }, "not authorized");

            daemon::daemon_main(cli)?;
            return Ok(());
        }
    }

    ui_main()
}

fn ui_main() -> Result<()> {
    let mut opts = NativeOptions::default();
    opts.viewport = ViewportBuilder::default()
        .with_inner_size(vec2(300., 400.))
        .with_min_inner_size(vec2(300., 200.));

    let mut picked_file: Option<rfd::FileHandle> = None;
    let mut file_pick_task: Option<JoinHandle<Option<rfd::FileHandle>>> = None;

    let mut prefixes = prefixes::load();
    prefixes.sort_by_key(|v| v.name.clone());
    let mut block_selection = prefixes
        .iter()
        .map(|v| (v.key.clone(), false))
        .collect::<HashMap<_, _>>();
    let (update_modal, read_modal) = tokio::sync::watch::channel(Option::<ModalDisplay>::None);
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_io()
        .build()?;
    eframe::run_simple_native("OW2 Server Picker", opts, move |ctx, _fr| {
        egui_extras::install_image_loaders(ctx);
        ctx.style_mut(|style| {
            style.interaction.selectable_labels = false;
        });
        if file_pick_task.as_ref().is_some_and(|t| t.is_finished()) {
            let task = file_pick_task.take().unwrap();
            let file = runtime.block_on(task);
            if let Err(e) = file {
                update_modal
                    .send(Some(ModalDisplay {
                        level: ModalLevel::Error,
                        title: "Unable to read the file selection".to_string(),
                        content: e.to_string(),
                    }))
                    .ok();
            } else if let Some(file) = file.unwrap() {
                picked_file = Some(file);
            }
        }
        TopBottomPanel::bottom("bottom panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                global_theme_preference_switch(ui);
                let result = ui
                    .with_layout(Layout::right_to_left(Align::Center), |ui| -> Result<()> {
                        let has_file = picked_file.is_some();
                        let has_region = block_selection.iter().any(|(_, &v)| v);
                        let start_clicked = ui.small_button("enable").clicked();
                        if start_clicked && !has_region {
                            update_modal
                                .send(Some(ModalDisplay {
                                    level: ModalLevel::Error,
                                    title: "No regions selected".to_string(),
                                    content: "Please select at least one region".to_string(),
                                }))
                                .ok();
                        }
                        if start_clicked && has_file && has_region {
                            if let Err(e) = daemon::kill() {
                                eprintln!("{e:#?}");
                            }

                            let selected = block_selection
                                .iter()
                                .filter_map(|(prefix, sel)| sel.then(|| prefix.clone()));

                            let res = daemon::start(
                                selected,
                                picked_file
                                    .clone()
                                    .unwrap()
                                    .path()
                                    .to_string_lossy()
                                    .to_string(),
                            )
                            .context("failed to start daemon");
                            if let Err(e) = res {
                                update_modal
                                    .send(Some(ModalDisplay {
                                        level: ModalLevel::Error,
                                        title: "Failed to start the daemon".to_string(),
                                        content: e.to_string(),
                                    }))
                                    .ok();
                            }

                            update_modal
                                .send(Some(ModalDisplay {
                                    level: ModalLevel::Info,
                                    title: "Server list updated".to_string(),
                                    content: "Please restart Overwatch to avoid connection issues."
                                        .to_string(),
                                }))
                                .ok();
                        }
                        if ui.small_button("disable").clicked() {
                            if let Err(e) = daemon::kill() {
                                update_modal
                                    .send(Some(ModalDisplay {
                                        level: ModalLevel::Error,
                                        title: "Failed to stop the daemon".to_string(),
                                        content: e.to_string(),
                                    }))
                                    .ok();
                            }
                        }
                        if ui.small_button("select game path").clicked()
                            || start_clicked && picked_file.is_none()
                        {
                            if file_pick_task.is_none() {
                                let ctx = ui.ctx().clone();
                                let task = runtime.spawn(async move {
                                    let res = AsyncFileDialog::new()
                                        .set_title("Find your Overwatch installation")
                                        .add_filter("Overwatch.exe", &["exe"])
                                        .pick_file()
                                        .await;
                                    ctx.request_repaint();
                                    res
                                });
                                file_pick_task.replace(task);
                            }
                        }

                        Ok(())
                    })
                    .inner;

                if let Err(e) = result {
                    update_modal
                        .send(Some(ModalDisplay {
                            level: ModalLevel::Error,
                            title: "Failed to start the daemon".to_string(),
                            content: e.to_string(),
                        }))
                        .ok();
                }
            });
        });
        CentralPanel::default().show(ctx, |ui| {
            ui.label("select desired matchmaking regions");
            ui.separator();
            ScrollArea::vertical().show(ui, |ui| {
                for region in &prefixes {
                    if widgets::prefix_widget(
                        ui,
                        &region.name,
                        &region.code,
                        block_selection[&region.key],
                    )
                    .clicked()
                    {
                        block_selection
                            .entry(region.key.clone())
                            .and_modify(|v| *v = !*v);
                    }
                }
            });
        });

        let mut clear_modal = false;

        if let Some(msg) = &*read_modal.borrow() {
            modal::show_modal(ctx, msg, || clear_modal = true);
        }

        if clear_modal {
            update_modal.send(None).ok();
        }
    })
    .map_err(|e| anyhow!("{}", e.to_string()))?;

    Ok(())
}
