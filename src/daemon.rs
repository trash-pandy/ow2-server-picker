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
    use std::io::Write;
    use std::os::linux::net::SocketAddrExt;
    use std::os::unix::net::{SocketAddr, UnixStream};

    let mut stream = UnixStream::connect_addr(&SocketAddr::from_abstract_name(fw::SOCKET_NAME)?)?;
    stream.write(b"kill")?;

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
