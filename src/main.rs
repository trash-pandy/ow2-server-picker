use std::collections::HashMap;
#[cfg(target_os = "linux")]
use std::time::Duration;

#[cfg(target_os = "linux")]
use ::{anyhow::ensure, clap::Parser, libc::geteuid, notify_rust::Notification};
#[cfg(target_os = "windows")]
use ::windows::{
    Win32::{
        System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance},
        UI::Shell::UserNotification,
    },
    core::HSTRING,
};
use anyhow::{Context, Result, anyhow};
use eframe::NativeOptions;
use eframe::egui::{
    Align, CentralPanel, Layout, Modal, RichText, ScrollArea, TopBottomPanel, ViewportBuilder,
    global_theme_preference_switch, vec2,
};
use rfd::AsyncFileDialog;
use tokio::task::JoinHandle;

#[cfg(target_os = "windows")]
use crate::fw::Com;

mod daemon;
mod fw;
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
    let (update_err, read_err) = tokio::sync::watch::channel(None);
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_io()
        .build()?;
    eframe::run_simple_native("OW2 Server Picker", opts, move |ctx, _fr| {
        ctx.style_mut(|style| {
            style.interaction.selectable_labels = false;
        });
        if file_pick_task.as_ref().is_some_and(|t| t.is_finished()) {
            let task = file_pick_task.take().unwrap();
            let file = runtime.block_on(task);
            if let Err(e) = file {
                update_err.send(Some(e.to_string())).ok();
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
                            update_err.send(Some("No regions are selected".to_string())).ok();
                        }
                        if start_clicked && has_file && has_region {
                            if let Err(e) = daemon::kill() {
                                eprintln!("{e:#?}");
                            }

                            let selected = block_selection
                                .iter()
                                .filter_map(|(prefix, sel)| sel.then(|| prefix.clone()));

                            let res = daemon::start(selected, picked_file.clone().unwrap().path().to_string_lossy().to_string())
                                .context("failed to start daemon");
                            if let Err(e) = res {
                                update_err.send(Some(e.to_string())).ok();
                            }

                            let notification_text =
                                "If Overwatch is already running, it will need to be restarted for changes to take effect.";
                            #[cfg(target_os = "linux")]
                            {
                                // an error in the notification doesn't particularly matter, so we'll ignore it
                                Notification::new()
                                    .appname("ow2-server-picker")
                                    .auto_icon()
                                    .body(notification_text)
                                    .timeout(Duration::from_secs(8))
                                    .show()
                                    .ok();
                            }
                            #[cfg(target_os = "windows")]
                            {
                                runtime.spawn(async move {
                                    // an error in the notification doesn't particularly matter, so we'll ignore it
                                    notification(notification_text).ok();
                                    fn notification(content: impl Into<String>) -> Result<()> {
                                        unsafe {
                                            use windows::Win32::UI::Shell::IUserNotification2;

                                            let _com = Com::init()?;
                                            let notification: IUserNotification2 = CoCreateInstance(&UserNotification, None, CLSCTX_INPROC_SERVER)?;
                                            notification.SetBalloonInfo(&HSTRING::from("ow2-server-picker"), &HSTRING::from(content.into()), 0)?;
                                            notification.SetBalloonRetry(8000, 0, 0)?;
                                            notification.Show(None, 8000, None)?;
                                        }
                                        Ok(())
                                    }
                                });
                            }
                        }
                        if ui.small_button("disable").clicked() {
                            if let Err(e) = daemon::kill() {
                                update_err.send(Some(e.to_string())).ok();
                            }
                        }
                        if ui.small_button("select game path").clicked() || start_clicked && picked_file.is_none() {
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
                    update_err.send(Some(e.to_string())).ok();
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
        let mut clear_err = false;
        if let Some(msg) = &*read_err.borrow() {
            Modal::new("error".into()).show(ctx, |ui| {
                ui.label(RichText::new(msg).color(ui.visuals().warn_fg_color));
                if ui.button("close").clicked() {
                    clear_err = true;
                }
            });
        }
        if clear_err {
            update_err.send(None).ok();
        }
    })
    .map_err(|e| anyhow!("{}", e.to_string()))?;

    Ok(())
}
