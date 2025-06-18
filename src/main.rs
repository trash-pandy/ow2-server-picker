use std::collections::HashMap;
#[cfg(target_os = "linux")]
use std::time::Duration;

#[cfg(target_os = "linux")]
use ::{anyhow::ensure, clap::Parser, libc::geteuid, notify_rust::Notification};
#[cfg(target_os = "windows")]
use ::windows::{
    Win32::{
        Foundation::RPC_E_CHANGED_MODE,
        System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance},
        UI::Shell::{IUserNotification, UserNotification},
    },
    core::HSTRING,
};
use anyhow::{Context, Result, anyhow};
use eframe::NativeOptions;
use eframe::egui::{
    Align, CentralPanel, Layout, RichText, ScrollArea, TopBottomPanel, ViewportBuilder,
    global_theme_preference_switch, vec2,
};
use rfd::AsyncFileDialog;
use tokio::task::JoinHandle;

#[cfg(target_os = "windows")]
use crate::fw::ComDrop;

mod daemon;
mod fw;
mod prefixes;
mod util;
mod widgets;

fn main() -> Result<()> {
    #[cfg(target_os = "linux")]
    if let Ok(cli) = daemon::Cli::try_parse() {
        if cli.kill {
            ensure!(unsafe { geteuid() == 0 }, "not authorized");

            daemon::kill()?;
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
    let mut more_than_once = 0;
    let mut start_clicked = 0;
    eframe::run_simple_native("OW2 Server Picker", opts, move |ctx, _fr| {
        ctx.style_mut(|style| {
            style.interaction.selectable_labels = false;
        });

        if file_pick_task.as_ref().is_some_and(|t| t.is_finished()) {
            let task = file_pick_task.take().unwrap();
            let file = runtime.block_on(task);
            more_than_once += 1;
            if more_than_once == 2 {
                unreachable!("notification path")
            }
            if let Err(e) = file {
                update_err.send(Some(e.to_string())).ok();
            } else if let Some(file) = file.unwrap() {
                let selected = block_selection
                    .iter()
                    .filter_map(|(prefix, sel)| sel.then(|| prefix.clone()));

                eprintln!("starting daemon");
                let res = daemon::start(selected, file.path().to_string_lossy().to_string())
                    .context("failed to start daemon");
                if let Err(e) = res {
                    update_err.send(Some(e.to_string())).ok();
                }
                #[cfg(target_os = "linux")]
                {
                    let res = Notification::new()
                        .appname("ow2-server-picker")
                        .auto_icon()
                        .body("If Overwatch is already running, it will need to be restarted for changes to take effect.")
                        .timeout(Duration::from_secs(8))
                        .show();
                    if let Err(e) = res {
                        update_err.send(Some(e.to_string()));
                    }
                }
                #[cfg(target_os = "windows")]
                {
                    let update_err = update_err.clone();
                    runtime.spawn(async move {
                        if let Err(e) = notification("If Overwatch is already running, it will need to be restarted for changes to take effect.") {
                            update_err.send(Some(e.to_string())).ok();
                        }

                        fn notification(content: impl Into<String>) -> Result<()> {
                            unsafe {
                                let com = ComDrop::init();
                                if com.0 != RPC_E_CHANGED_MODE {
                                    com.0.ok()?;
                                }
                                let notification: IUserNotification = CoCreateInstance(&UserNotification, None, CLSCTX_INPROC_SERVER)?;
                                notification.SetBalloonInfo(&HSTRING::from("ow2-server-picker"), &HSTRING::from(content.into()), 0)?;
                                notification.SetBalloonRetry(8000, 0, 0)?;
                                notification.Show(None, 8000)?;
                            }
                            Ok(())
                        }
                    });
                }
            }
        }
        TopBottomPanel::bottom("bottom panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                global_theme_preference_switch(ui);
                let result = ui
                    .with_layout(Layout::right_to_left(Align::Center), |ui| -> Result<()> {
                        if ui.small_button("start").clicked() {
                            daemon::kill().context("failed to kill daemon")?;

                            start_clicked += 1;
                            println!("start was clicked {start_clicked} times");

                            if file_pick_task.is_none() {
                                let ctx = ui.ctx().clone();
                                let task = runtime.spawn(async move {
                                    let res = AsyncFileDialog::new()
                                        .set_title("Find your Overwatch installation")
                                        .add_filter("Overwatch.exe", &["exe"])
                                        .pick_file()
                                        .await;
                                    ctx.request_repaint();
                                    eprintln!("done");
                                    res
                                });
                                file_pick_task.replace(task);
                                more_than_once = 0;
                            }
                        }
                        if ui.small_button("disable").clicked() {
                            daemon::kill().context("failed to kill daemon")?;
                        }

                        Ok(())
                    })
                    .inner;

                if let Err(e) = result {
                    eprintln!("{e:#?}");
                    update_err.send(Some(e.to_string())).ok();
                }
            });
        });
        CentralPanel::default().show(ctx, |ui| {
            ui.label("select desired matchmaking regions");
            if let Some(msg) = &*read_err.borrow() {
                ui.label(RichText::new(msg).color(ui.visuals().warn_fg_color));
            }
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
    })
    .map_err(|e| anyhow!("{}", e.to_string()))?;

    Ok(())
}
