#[cfg(target_os = "linux")]
use std::fs;

#[cfg(target_os = "linux")]
use ::{
    anyhow::Context,
    libc::{SIGINT, geteuid},
};
use anyhow::Result;
use iter_tools::Itertools;

use crate::{fw, prefixes};

#[cfg(target_os = "linux")]
#[derive(clap::Parser)]
pub struct Cli {
    /// run as a daemon to add ow2 processes to the proper cgroup
    #[arg(long)]
    pub daemon: bool,

    /// kill the running daemon
    #[arg(long)]
    pub kill: bool,

    #[arg(long)]
    pub game_path: String,

    /// prefixes to block in daemon mode (ex: 10.0.0.0/24)
    pub prefixes: Vec<String>,
}

#[cfg(target_os = "linux")]
#[tokio::main]
pub async fn daemon_main(cli: Cli) -> Result<()> {
    let prefixes = prefixes::load();
    fw::start(
        prefixes
            .iter()
            .filter_map(|v| cli.prefixes.contains(&v.key).then(|| v.prefixes.clone()))
            .flatten()
            .collect_vec(),
        cli.game_path,
    )
    .await
}

#[cfg(target_os = "windows")]
pub fn kill() -> Result<()> {
    fw::stop()
}

#[cfg(target_os = "linux")]
pub fn kill() -> Result<()> {
    if unsafe { geteuid() == 0 } {
        fw::stop()?;

        let my_exe_path = fs::read_link("/proc/self/exe")?;

        let procs_readdir = std::fs::read_dir("/proc")?;
        for proc in procs_readdir {
            let proc = proc?;

            let Ok(proc_pid) = i32::from_str_radix(&proc.file_name().to_string_lossy(), 10) else {
                continue;
            };

            let Ok(cmd) = std::fs::read_to_string(proc.path().join("cmd")) else {
                continue;
            };
            if !cmd.contains("--daemon") {
                continue;
            }

            let exe_path = proc.path().join("exe");
            let Ok(exe_path) =
                fs::read_link(&exe_path).context(exe_path.clone().display().to_string())
            else {
                continue;
            };
            if exe_path == my_exe_path {
                unsafe { libc::kill(proc_pid, SIGINT) };
            }
        }
    } else {
        std::process::Command::new("/usr/bin/env")
            .arg("pkexec")
            .arg(std::env::current_exe()?)
            .arg("--kill")
            .spawn()?;
    }

    Ok(())
}

#[cfg(target_os = "windows")]
#[tokio::main]
pub async fn start(selected: impl Iterator<Item = String>, game_path: String) -> Result<()> {
    let selected = selected.collect_vec();
    let block_list = prefixes::load();
    fw::start(
        block_list
            .iter()
            .filter_map(|v| selected.contains(&v.key).then(|| v.prefixes.clone()))
            .flatten()
            .collect_vec(),
        game_path,
    )
    .await?;

    Ok(())
}

#[cfg(target_os = "linux")]
pub fn start(selected: impl Iterator<Item = String>, game_path: String) -> Result<()> {
    std::process::Command::new("/usr/bin/env")
        .arg("pkexec")
        .arg(std::env::current_exe()?)
        .arg("--daemon")
        .arg("--game-path")
        .arg(game_path)
        .args(selected)
        .spawn()?;

    Ok(())
}
