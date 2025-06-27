use std::ops::Not;

#[cfg(target_os = "linux")]
use ::{anyhow::ensure, clap::Parser, libc::geteuid};
use anyhow::{Result, anyhow};
use eframe::egui::{
    Align, CentralPanel, Layout, ScrollArea, TopBottomPanel, ViewportBuilder,
    global_theme_preference_switch, vec2,
};
use eframe::{NativeOptions, egui};
use indexmap::IndexMap;
use iter_tools::Itertools;
use rfd::AsyncFileDialog;
use tokio::sync::watch;
use tokio::task::JoinHandle;

use crate::daemon::KillError;
use crate::modal::{ModalDisplay, ModalLevel};
use crate::ping::PingReceiver;

mod daemon;
mod fw;
mod modal;
mod ping;
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
            ensure!(unsafe { geteuid() == 0 }, "daemon not running as root");

            daemon::daemon_main(cli)?;
            return Ok(());
        }
    }

    let opts = NativeOptions {
        viewport: ViewportBuilder::default()
            .with_inner_size(vec2(300., 400.))
            .with_min_inner_size(vec2(300., 200.)),
        ..Default::default()
    };
    eframe::run_native(
        "ow2 server picker",
        opts,
        Box::new(|cc| Ok(Box::new(App::new(cc).unwrap()))),
    )
    .map_err(|e| anyhow!("{}", e.to_string()))
}

struct FileSelectionTask {
    /// Whether to start the daemon after receiving the file.
    start_daemon: bool,

    /// An associated join handle.
    handle: JoinHandle<Option<rfd::FileHandle>>,
}

struct RegionEntry {
    /// The region this
    region: prefixes::Region,

    /// Current ping to the main server.
    ping: ping::PingStatus,

    /// Whether the server is selected as one of the preferred.
    selected: bool,
}

struct App {
    /// Application's runtime.
    runtime: tokio::runtime::Runtime,

    /// Selected regions.
    ///
    /// It is keyed by region's key.
    region_states: IndexMap<String, RegionEntry>,

    /// Game executable selected in file dialog.
    game_exe: Option<rfd::FileHandle>,

    /// A receiver for the current file selection task.
    file_selection_task_rx: watch::Receiver<Option<FileSelectionTask>>,

    /// A sender for the current file selection task.
    file_selection_task_tx: watch::Sender<Option<FileSelectionTask>>,

    /// A receiver for the current modal display.
    modal_rx: watch::Receiver<Option<ModalDisplay>>,

    /// A sender for the current modal display.
    modal_tx: watch::Sender<Option<ModalDisplay>>,

    /// A receiver of the ping updates.
    ping_rx: Option<PingReceiver>,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Result<Self> {
        egui_extras::install_image_loaders(&cc.egui_ctx);

        cc.egui_ctx.style_mut(|style| {
            style.interaction.selectable_labels = false;
        });

        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_io()
            .enable_time()
            .build()?;

        let region_states = prefixes::load()
            .into_iter()
            .sorted_by_key(|region| region.name.clone())
            .map(|region| {
                (
                    region.key.clone(),
                    RegionEntry {
                        region,
                        ping: ping::PingStatus::Unknown,
                        selected: false,
                    },
                )
            })
            .collect::<IndexMap<_, _>>();

        let (file_selection_task_tx, file_selection_task_rx) =
            watch::channel(Option::<FileSelectionTask>::None);

        let (modal_tx, modal_rx) = watch::channel(Option::<ModalDisplay>::None);

        let ping_rx = ping::setup_pinger(
            &runtime,
            region_states
                .iter()
                .map(|(key, entry)| (key.clone(), entry.region.ping))
                .sorted()
                .collect_vec(),
        )
        .inspect_err(|_| {
            modal_tx
                .send(Some(ModalDisplay {
                    level: ModalLevel::Warning,
                    title: "Pings are unavailable".to_string(),
                    content: "Your system may not support this feature".to_string(),
                }))
                .expect("failed to send an error modal");
        })
        .ok();

        runtime.spawn({
            let mut fst_rx = file_selection_task_rx.clone();
            let mut m_rx = modal_rx.clone();
            let mut p_sub = ping_rx.as_ref().map(|rx| rx.resubscribe());

            let ctx = cc.egui_ctx.clone();

            async move {
                loop {
                    tokio::select! {
                        result = fst_rx.changed() => {
                            if result.is_err() { break }
                        },
                        result = m_rx.changed() => {
                            if result.is_err() { break }
                        },
                        result = p_sub.as_mut().unwrap().recv(), if p_sub.is_some() => {
                            if result.is_err() { break }
                        },
                    }

                    ctx.request_repaint();
                }
            }
        });

        Ok(Self {
            runtime,
            file_selection_task_rx,
            file_selection_task_tx,
            modal_rx,
            modal_tx,
            game_exe: None,
            region_states,
            ping_rx,
        })
    }

    fn run_exe_selection(&self, start_daemon: bool) {
        let handle = self.runtime.spawn(async move {
            AsyncFileDialog::new()
                .set_title("Find your Overwatch installation")
                .add_filter("Overwatch.exe", &["exe"])
                .pick_file()
                .await
        });

        self.file_selection_task_tx
            .send(Some(FileSelectionTask {
                start_daemon,
                handle,
            }))
            .expect("failed to send file selection task");
    }

    fn handle_file_picker_task(&mut self) {
        let completed = self
            .file_selection_task_rx
            .borrow()
            .as_ref()
            .is_some_and(|t| t.handle.is_finished());

        if completed {
            let task = self.file_selection_task_tx.send_replace(None).unwrap();
            let file = self.runtime.block_on(task.handle);
            if let Err(e) = file {
                self.modal_tx
                    .send(Some(ModalDisplay {
                        level: ModalLevel::Error,
                        title: "Unable to read the file selection".to_string(),
                        content: e.to_string(),
                    }))
                    .expect("failed to send modal");
            } else if let Some(file) = file.unwrap() {
                self.game_exe = Some(file);

                if task.start_daemon {
                    self.start_daemon();
                }
            }
        }
    }

    fn handle_ping_updates(&mut self) {
        if let Some(rx) = self.ping_rx.as_mut() {
            while let Ok(ping::PingUpdate(key, status)) = rx.try_recv() {
                self.region_states.get_mut(&key).map_or_else(
                    || panic!("failed to retrieve region {key} for ping update"),
                    |region| region.ping = status.clone(),
                );
            }
        }
    }

    fn start_daemon(&self) {
        let any_selected = self.region_states.iter().any(|(_, state)| state.selected);

        if !any_selected {
            self.modal_tx
                .send(Some(ModalDisplay {
                    level: ModalLevel::Error,
                    title: "No regions selected".to_string(),
                    content: "Please select at least one region".to_string(),
                }))
                .expect("failed to send modal");

            return;
        }

        if self.stop_daemon(true).is_err() {
            return;
        }

        let blocked_regions = self
            .region_states
            .iter()
            .filter(|&(_, entry)| entry.selected.not())
            .map(|(key, _)| key.clone());

        let game_exe = self
            .game_exe
            .as_ref()
            .unwrap()
            .path()
            .to_string_lossy()
            .to_string();

        if let Err(e) = daemon::start(blocked_regions, game_exe) {
            self.modal_tx
                .send(Some(ModalDisplay {
                    level: ModalLevel::Error,
                    title: "Cannot enable blocking".to_string(),
                    content: format!("Failed to activate blocking due to an error:\n\n{e}"),
                }))
                .expect("failed to send an error modal");
        } else {
            self.modal_tx
                .send(Some({
                    ModalDisplay {
                        level: ModalLevel::Success,
                        title: "Server list updated".to_string(),
                        content: "Restart Overwatch to avoid connection issues.".to_string(),
                    }
                }))
                .expect("failed to send a success modal");
        }
    }

    fn stop_daemon(&self, silent: bool) -> Result<()> {
        if let Err(e) = daemon::kill() {
            if silent && let KillError::Refused = e {
                return Ok(());
            }
            self.modal_tx
                .send(Some(ModalDisplay {
                    level: ModalLevel::Error,
                    title: "Cannot disable blocking".to_string(),
                    content: format!("Failed to deactivate blocking due to an error:\n{e}"),
                }))
                .expect("failed to send an error modal");

            return Err(anyhow!("{}", e.to_string()));
        } else if !silent {
            self.modal_tx
                .send(Some({
                    ModalDisplay {
                        level: ModalLevel::Success,
                        title: "Server blocking disabled".to_string(),
                        content: "Restart Overwatch for the changes to apply.".to_string(),
                    }
                }))
                .expect("failed to send a success modal");
        }

        Ok(())
    }

    fn on_game_path_btn_click(&self) {
        self.run_exe_selection(false);
    }

    fn on_disable_btn_click(&self) {
        let _ = self.stop_daemon(false);
    }

    fn on_enable_btn_click(&self) {
        if self.game_exe.is_none() {
            return self.run_exe_selection(true);
        }

        self.start_daemon();
    }

    fn render_bottom_bar(&self, ctx: &egui::Context) {
        TopBottomPanel::bottom("bottom panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                global_theme_preference_switch(ui);

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.small_button("enable").clicked() {
                        self.on_enable_btn_click();
                    }
                    if ui.small_button("disable").clicked() {
                        self.on_disable_btn_click();
                    }
                    if ui.small_button("select game path").clicked() {
                        self.on_game_path_btn_click();
                    }
                })
            })
        });
    }

    fn render_central_panel(&mut self, ctx: &egui::Context) {
        CentralPanel::default().show(ctx, |ui| {
            ui.label("select desired matchmaking regions");
            ui.separator();
            ScrollArea::vertical().show(ui, |ui| {
                for (_, entry) in self.region_states.iter_mut() {
                    let widget = widgets::prefix_widget(
                        ui,
                        &entry.region.name,
                        &entry.region.code,
                        entry.selected,
                        &entry.ping,
                    );

                    if widget.clicked() {
                        entry.selected = !entry.selected;
                    }
                }
            });
        });
    }

    fn render_modal(&mut self, ctx: &egui::Context) {
        let mut clear_modal = false;

        if let Some(msg) = &*self.modal_rx.borrow() {
            modal::show_modal(ctx, msg, || clear_modal = true);
        }

        if clear_modal {
            self.modal_tx.send(None).ok();
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        self.handle_file_picker_task();
        self.handle_ping_updates();

        self.render_bottom_bar(ctx);
        self.render_central_panel(ctx);
        self.render_modal(ctx);
    }
}
