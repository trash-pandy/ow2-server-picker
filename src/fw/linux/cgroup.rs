use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::Result;
use nix::mount::MsFlags;

pub const NET_CLS_CLASSID: u32 = 0x1b854c;

const NET_CLS_DIR: &str = "/sys/fs/cgroup/net_cls";

pub struct CGroup {
    game_cgroup: PathBuf,
}

impl CGroup {
    pub fn new() -> Result<Self> {
        let root_cgroup = create_cgroup()?;
        let game_cgroup = root_cgroup.join("ow2serverpicker");
        if !fs::exists(&game_cgroup)? {
            fs::create_dir(&game_cgroup)?;
        }

        let classid_path = game_cgroup.join("net_cls.classid");
        write_string(NET_CLS_CLASSID, classid_path)?;

        Ok(Self { game_cgroup })
    }

    pub fn add(&self, pid: i32) -> Result<()> {
        write_string(pid, self.game_cgroup.join("cgroup.procs"))
    }
}

fn create_cgroup() -> Result<PathBuf> {
    if let Some(path) = find_net_cls_mount() {
        return Ok(path);
    }

    if !std::fs::exists(NET_CLS_DIR)? {
        fs::create_dir(NET_CLS_DIR)?;
    }

    nix::mount::mount(
        Some("net_cls"),
        NET_CLS_DIR,
        Some("cgroup"),
        MsFlags::empty(),
        Some("net_cls"),
    )?;

    Ok(NET_CLS_DIR.into())
}

fn find_net_cls_mount() -> Option<PathBuf> {
    fs::read_to_string("/proc/mounts")
        .ok()?
        .lines()
        .find_map(|line| {
            let mut parts = line.split(' ');
            let _device_type = parts.next()?;
            let mount_path = parts.next()?;
            let filesystem_type = parts.next()?;
            let mount_options = parts.next()?;

            if filesystem_type != "cgroup" {
                return None;
            }

            if !mount_options.split(',').any(|v| v == "net_cls") {
                return None;
            }

            Some(mount_path.into())
        })
}

fn write_string(content: impl ToString, path: impl AsRef<Path>) -> Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    file.write_all(content.to_string().as_bytes())
        .map_err(Into::into)
}
