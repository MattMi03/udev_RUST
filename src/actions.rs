/// src/actions.rs
use nix::sys::stat::{makedev, mknod, Mode, SFlag};
use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;

use log::*;
use users::{get_group_by_name, get_user_by_name};

use crate::device::UEventDevice;
use crate::rules::matcher::Rule;

/// 替换字符串中的变量，比如 $DEVNAME、$ACTION
pub fn substitute_vars(input: &str, device: &UEventDevice) -> String {
    let mut result = input.to_string();

    let devnum_str = device.devnum().map(|n| n.to_string());
    let major_str = device.major().map(|n| n.to_string());
    let minor_str = device.minor().map(|n| n.to_string());
    let devtype = device.devtype();
    let kernel = device.kernel();
    let devnode = device.devnode();
    let devpath_str = device.devpath().to_str();
    let subsystem = Some(device.subsystem());

    let vars: Vec<(char, Option<&str>)> = vec![
        ('k', kernel),
        ('n', devnode),
        ('p', devpath_str),
        ('c', devtype),
        ('t', devtype),
        ('d', devnum_str.as_deref()),
        ('s', subsystem),
        ('m', major_str.as_deref()),
        ('r', minor_str.as_deref()),
    ];

    for (var_char, val_opt) in vars {
        if let Some(val) = val_opt {
            let pattern = format!("%{}", var_char);
            result = result.replace(&pattern, val);
        }
    }

    for (key, val) in device.properties() {
        let pattern = format!("${{{}}}", key);
        result = result.replace(&pattern, val);
    }

    result
}

pub fn create_device_node(
    devname: &str,
    device: &UEventDevice,
    rule: &Rule,
) -> std::io::Result<()> {
    let major = device.major().unwrap_or(0);
    let minor = device.minor().unwrap_or(0);

    let sflag = match device.devtype() {
        Some("disk") | Some("partition") => SFlag::S_IFBLK,
        _ => SFlag::S_IFCHR,
    };

    let test_dev_root = "/home/rust_udev/testdev";
    let full_path = format!("{}/{}", test_dev_root, devname);
    let path = Path::new(&full_path);

    fs::create_dir_all(path.parent().unwrap_or(Path::new("/dev"))).unwrap();

    let mode = Mode::from_bits(0o660).unwrap_or(Mode::empty());

    match mknod(path, sflag, mode, makedev(major.into(), minor.into())) {
        Ok(_) => info!("Created device node: {}", devname),
        Err(e) => {
            if e.to_string().contains("File exists") {
                info!("Device node already exists: {}", devname);
            } else {
                error!("Failed to create device node {}: {}", devname, e);
            }
        }
    }

    let _ = apply_mode(path, &rule.mode);
    let _ = apply_owner(path, &rule.owner);
    let _ = apply_group(path, &rule.group);

    Ok(())
}

pub fn apply_mode(dev_path: &Path, mode: &Option<String>) -> std::io::Result<()> {
    if let Some(mode_str) = mode {
        let mode_val = u32::from_str_radix(mode_str, 8)
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid mode"))?;
        info!("Applying mode {} to {:?}", mode_str, dev_path);
        fs::set_permissions(dev_path, fs::Permissions::from_mode(mode_val))?;
    } else {
        info!("No mode specified for {:?}", dev_path);
    }
    Ok(())
}

pub fn apply_owner(dev_path: &Path, owner: &Option<String>) -> std::io::Result<()> {
    if let Some(owner_name) = owner {
        if let Some(user) = get_user_by_name(owner_name) {
            info!("Applying owner {} to {:?}", owner_name, dev_path);
            nix::unistd::chown(dev_path, Some(user.uid().into()), None)?;
        } else {
            warn!("User '{}' not found", owner_name);
        }
    } else {
        info!("No owner specified for {:?}", dev_path);
    }
    Ok(())
}

pub fn apply_group(dev_path: &Path, group: &Option<String>) -> std::io::Result<()> {
    if let Some(group_name) = group {
        if let Some(group) = get_group_by_name(group_name) {
            info!("Applying group {} to {:?}", group_name, dev_path);
            nix::unistd::chown(dev_path, None, Some(group.gid().into()))?;
        } else {
            warn!("Group '{}' not found", group_name);
        }
    } else {
        info!("No group specified for {:?}", dev_path);
    }
    Ok(())
}

pub fn create_symlinks(
    dev_path: &Path,
    symlinks: &[String],
    device: &UEventDevice,
) -> std::io::Result<()> {
    for link in symlinks {
        info!("Creating symlink for: {}", link);
        let substituted = substitute_vars(link, device);
        info!("Substituted symlink path: {}", substituted);
        let link_path = PathBuf::from("/home/rust_udev/testdev").join(substituted);

        if link_path.exists() {
            fs::remove_file(&link_path)?;
        }

        info!("Creating symlink {:?} -> {:?}", link_path, dev_path);
        symlink(dev_path, link_path)?;
    }
    Ok(())
}

pub fn remove_device_node(dev_path: &Path) -> std::io::Result<()> {
    debug!("entering remove_device_node {:?}", dev_path);
    if dev_path.exists() {
        info!("Removing device node: {:?}", dev_path);
        fs::remove_file(dev_path)?;
    } else {
        warn!("Device node does not exist: {:?}", dev_path);
    }
    Ok(())
}

pub fn remove_symlinks(dev_path: &Path, symlink_dir: &Path) -> std::io::Result<()> {
    let dev_path_canon = dev_path.canonicalize()?;
    debug!("Scanning for symlinks pointing to {:?}", dev_path_canon);

    for entry in fs::read_dir(symlink_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.symlink_metadata()?.file_type().is_symlink() {
            match fs::read_link(&path) {
                Ok(target) => {
                    let resolved_target = path.parent().unwrap_or(symlink_dir).join(&target);
                    match resolved_target.canonicalize() {
                        Ok(canon_target) => {
                            if canon_target == dev_path_canon {
                                info!("Removing symlink {:?} -> {:?}", path, target);
                                fs::remove_file(&path)?;
                            }
                        }
                        Err(e) => {
                            warn!(
                                "Failed to canonicalize symlink target {:?}: {}",
                                resolved_target, e
                            );
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to read symlink {:?}: {}", path, e);
                }
            }
        }
    }

    Ok(())
}

pub fn run_commands(commands: &Vec<String>, device: &UEventDevice) -> std::io::Result<()> {
    let envs = device.properties();

    for cmd in commands {
        let output = Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .envs(envs)
            .output()?;

        if !output.status.success() {
            eprintln!("Command failed: {}", cmd);
            eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
        } else {
            println!(
                "Command output: {}",
                String::from_utf8_lossy(&output.stdout)
            );
        }
    }

    Ok(())
}
