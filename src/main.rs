use std::collections::HashMap;
use std::time::Duration;

#[cfg(target_os = "linux")]
use ::{anyhow::ensure, clap::Parser, libc::geteuid};
use anyhow::{Context, Result, anyhow};
use eframe::NativeOptions;
use eframe::egui::{
    Align, CentralPanel, Layout, RichText, ScrollArea, TopBottomPanel, ViewportBuilder,
    global_theme_preference_switch, vec2,
};
use notify_rust::Notification;
use rfd::AsyncFileDialog;

mod daemon;
mod fw;
mod prefixes;
mod util;
mod widgets;

fn main() -> Result<()> {
    #[cfg(target_os = "linux")]
    if let Ok(cli) = daemon::Cli::try_parse() {
        if cli.kill {
            daemon::kill()?;
            return Ok(());
        }

        if cli.daemon {
            ensure!(unsafe { geteuid() == 0 }, "not running as root");

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

    let mut file_pick_task: Option<tokio::task::JoinHandle<Option<rfd::FileHandle>>> = None;

    let prefixes = prefixes::load();
    let mut block_selection = prefixes
        .iter()
        .map(|v| (v.key.clone(), false))
        .collect::<HashMap<_, _>>();
    let mut err_state = None;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_io()
        .build()?;
    eframe::run_simple_native("OW2 Server Picker", opts, move |ctx, _fr| {
        ctx.style_mut(|style| {
            style.interaction.selectable_labels = false;
        });
        let file_pick_task = &mut file_pick_task;

        if file_pick_task.as_ref().is_some_and(|t| t.is_finished()) {
            let task = file_pick_task.take().unwrap();
            let file = runtime.block_on(task);
            if let Err(e) = file {
                err_state.replace(e.to_string());
            } else if let Some(file) = file.unwrap() {
                let selected = block_selection
                    .iter()
                    .filter_map(|(prefix, sel)| sel.then(|| prefix.clone()));

                eprintln!("starting daemon");
                let res = daemon::start(selected, file.path().to_string_lossy().to_string())
                    .context("failed to start daemon");
                if let Err(e) = res {
                    err_state.replace(e.to_string());
                }
                let res = Notification::new()
                    .appname("dropship-rs")
                    .body("If Overwatch is already running, it will need to be restarted for changes to take effect.")
                    .timeout(Duration::from_secs(8))
                    .show();
                if let Err(e) = res {
                    err_state.replace(e.to_string());
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
                    err_state.replace(e.to_string());
                }
            });
        });
        CentralPanel::default().show(ctx, |ui| {
            ui.label("select desired matchmaking regions");
            if let Some(err_state) = &err_state {
                ui.label(RichText::new(err_state).color(ui.visuals().warn_fg_color));
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
