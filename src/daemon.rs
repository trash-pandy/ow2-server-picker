use std::io;
#[cfg(target_os = "linux")]
use std::result;

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
            .filter(|v| cli.prefixes.contains(&v.key))
            .flat_map(|v| v.prefixes.clone())
            .collect_vec(),
        cli.game_path,
    )
    .await
}

#[cfg(target_os = "windows")]
pub fn kill() -> Result<(), KillError> {
    Ok(fw::stop()?)
}

#[cfg(target_os = "linux")]
pub fn kill() -> result::Result<(), KillError> {
    use std::io::Write;
    use std::os::linux::net::SocketAddrExt;
    use std::os::unix::net::{SocketAddr, UnixStream};

    let mut stream = UnixStream::connect_addr(&SocketAddr::from_abstract_name(fw::SOCKET_NAME)?)
        .map_err(|e| {
            if e.kind() == io::ErrorKind::ConnectionRefused {
                KillError::Refused
            } else {
                KillError::IoError(e)
            }
        })?;
    stream.write_all(b"kill")?;

    Ok(())
}

#[derive(thiserror::Error, Debug)]
pub enum KillError {
    #[error("failed to communicate with the process: {0}")]
    IoError(#[from] io::Error),
    #[error("{0}")]
    Anyhow(#[from] anyhow::Error),
    #[error("connection was refused, the daemon is likely not running")]
    Refused,
}

#[cfg(target_os = "windows")]
#[tokio::main]
pub async fn start(block_list: impl Iterator<Item = String>, game_path: String) -> Result<()> {
    let block_list = block_list.collect_vec();
    let all_prefixes = prefixes::load();
    fw::start(
        all_prefixes
            .iter()
            .filter_map(|v| block_list.contains(&v.key).then(|| v.prefixes.clone()))
            .flatten()
            .collect_vec(),
        game_path,
    )
    .await?;

    Ok(())
}

#[cfg(target_os = "linux")]
pub fn start(block_list: impl Iterator<Item = String>, game_path: String) -> Result<()> {
    let block_list = block_list.collect_vec();
    std::process::Command::new("/usr/bin/env")
        .arg("pkexec")
        .arg(std::env::current_exe()?)
        .arg("--daemon")
        .arg("--game-path")
        .arg(game_path)
        .args(block_list)
        .spawn()?;

    Ok(())
}
