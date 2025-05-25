// src/libudev.rs

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    fs,
    os::unix::fs::{MetadataExt, FileTypeExt},
};

pub fn get_device_info(devpath: &str) -> Option<HashMap<String, String>> {
    let dev_path = Path::new(devpath);
    
    if !dev_path.exists() {
        return None;
    }

    let mut info = HashMap::new();

    if let Ok(metadata) = fs::metadata(dev_path) {
        if metadata.file_type().is_char_device() {

            let (major, minor) = (libc::major(metadata.rdev()), libc::minor(metadata.rdev()));
            let sys_path = PathBuf::from(format!("/sys/dev/char/{}:{}", major, minor));
            if !sys_path.exists() {
                return None;
            }

            info.insert("DEVNAME".into(), devpath.to_string());

            if let Ok(uevent) = fs::read_to_string(sys_path.join("uevent")) {
                for line in uevent.lines() {
                    if let Some((k, v)) = line.split_once('=') {
                        if k != "DEVNAME" {
                            info.insert(k.into(), v.into());
                        }
                    }
                }
            }

            if let Ok(subsystem) = fs::read_link(sys_path.join("subsystem")) {
                if let Some(name) = subsystem.file_name() {
                    info.insert("SUBSYSTEM".into(), name.to_string_lossy().into_owned());
                }
            }

            if let Ok(devtype) = fs::read_to_string(sys_path.join("type")) {
                info.insert("DEVTYPE".into(), devtype.trim().to_string());
            }

            if let Ok(driver) = fs::read_link(sys_path.join("device/driver")) {
                if let Some(name) = driver.file_name() {
                    info.insert("DRIVER".into(), name.to_string_lossy().into_owned());
                }
            }

            if let Ok(device_path) = fs::read_link(sys_path.join("device")) {
                info.insert("PHYSDEVPATH".into(), device_path.to_string_lossy().into_owned());
            }

            info.insert("DEVMODE".into(), format!("{:o}", metadata.mode() & 0o777));
            return Some(info);
        }
    }

    let uevent_path = dev_path.join("uevent");
    if uevent_path.exists() {
        if let Ok(content) = fs::read_to_string(uevent_path) {
            for line in content.lines() {
                if let Some((k, v)) = line.split_once('=') {
                    info.insert(k.to_string(), v.to_string());
                }
            }
        }

        if let Ok(subsystem) = fs::read_link(dev_path.join("subsystem")) {
            if let Some(name) = subsystem.file_name() {
                info.insert("SUBSYSTEM".into(), name.to_string_lossy().into_owned());
            }
        }

        return Some(info);
    }

    None
}
