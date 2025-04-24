use nix::sys::stat::{makedev, mknod, Mode, SFlag};
use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;

use log::*;
use users::{get_group_by_name, get_user_by_name};

use crate::rules::matcher::Rule;

/// 替换字符串中的变量，比如 $DEVNAME、$ACTION
pub fn substitute_vars(input: &str, event: &HashMap<String, String>) -> String {
    let mut result = input.to_string();
    for (key, val) in event {
        let pattern = format!("${{{}}}", key);
        result = result.replace(&pattern, val);
    }
    result
}

pub fn create_device_node(devname: &str, event: &HashMap<String, String>, rule: &Rule) -> std::io::Result<()> {
    let major = event
        .get("MAJOR")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    let minor = event
        .get("MINOR")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);

    let sflag = match event.get("DEVTYPE").map(|s| s.as_str()) {
        Some("disk") | Some("partition") => SFlag::S_IFBLK,
        _ => SFlag::S_IFCHR,
    };

    let test_dev_root = "/home/rust_udev/testdev";
    let full_path = format!("{}/{}", test_dev_root, devname);
    let path = Path::new(&full_path);

    fs::create_dir_all(path.parent().unwrap_or(Path::new("/dev"))).unwrap();

    let mode = Mode::from_bits(0o660).unwrap_or(Mode::empty());

    // 实际创建节点
    match mknod(path, sflag, mode, makedev(major, minor)) {
        Ok(_) => info!("Created device node: {}", devname),
        Err(e) => {
            if e.to_string().contains("File exists") {
                info!("Device node already exists: {}", devname);
            } else {
                error!("Failed to create device node {}: {}", devname, e);
            }
        }
    }

    // 设备节点创建成功后，立即设置权限
    let _ = apply_mode(path, &rule.mode);
    let _ = apply_owner(path, &rule.owner);
    let _ = apply_group(path, &rule.group);

    Ok(())
}

/// 设置设备权限 MODE
pub fn apply_mode(dev_path: &Path, mode: &Option<String>) -> std::io::Result<()> {
    if let Some(mode_str) = mode {
        let mode_val = u32::from_str_radix(mode_str, 8)
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid mode"))?;
        info!("Applying mode {} to {:?}", mode_str, dev_path);
        fs::set_permissions(dev_path, fs::Permissions::from_mode(mode_val))?;
    }
    Ok(())
}

/// 设置设备属主 OWNER
pub fn apply_owner(dev_path: &Path, owner: &Option<String>) -> std::io::Result<()> {
    if let Some(owner_name) = owner {
        if let Some(user) = get_user_by_name(owner_name) {
            info!("Applying owner {} to {:?}", owner_name, dev_path);
            nix::unistd::chown(dev_path, Some(user.uid().into()), None)?;
        } else {
            warn!("User '{}' not found", owner_name);
        }
    }
    Ok(())
}

/// 设置设备属组 GROUP
pub fn apply_group(dev_path: &Path, group: &Option<String>) -> std::io::Result<()> {
    if let Some(group_name) = group {
        if let Some(group) = get_group_by_name(group_name) {
            info!("Applying group {} to {:?}", group_name, dev_path);
            nix::unistd::chown(dev_path, None, Some(group.gid().into()))?;
        } else {
            warn!("Group '{}' not found", group_name);
        }
    }
    Ok(())
}

/// 创建符号链接 SYMLINK+=
pub fn create_symlinks(
    dev_path: &Path,
    symlinks: &[String],
    event: &HashMap<String, String>,
) -> std::io::Result<()> {
    for link in symlinks {
        let substituted = substitute_vars(link, event);
        let link_path = PathBuf::from("/home/rust_udev/testsymlink").join(substituted);

        if link_path.exists() {
            fs::remove_file(&link_path)?;
        }

        info!("Creating symlink {:?} -> {:?}", link_path, dev_path);
        symlink(dev_path, link_path)?;
    }
    Ok(())
}

/// 执行命令 RUN+=
pub fn run_commands(commands: &[String], event: &HashMap<String, String>) -> std::io::Result<()> {
    for cmd in commands {
        let substituted = substitute_vars(cmd, event);
        info!("Running command: {}", substituted);
        let status = Command::new("sh")
            .arg("-c")
            .arg(&substituted)
            .envs(event)
            .spawn()?
            .wait()?;

        if !status.success() {
            warn!(
                "Command '{}' exited with status: {:?}",
                substituted,
                status.code()
            );
        }
    }
    Ok(())
}
