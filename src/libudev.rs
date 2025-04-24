// src/libudev.rs

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    fs,
    os::unix::fs::{MetadataExt, FileTypeExt},
};

pub fn get_device_info(devpath: &str) -> Option<HashMap<String, String>> {
    let dev_path = Path::new(devpath);

    // 1. 基础校验
    if !dev_path.exists() {
        return None;
    }
    let metadata = match fs::metadata(dev_path) {
        Ok(m) => m,
        Err(_) => return None,
    };
    if !metadata.file_type().is_char_device() {
        return None;
    }

    // 2. 获取设备号
    let (major, minor) = (libc::major(metadata.rdev()), libc::minor(metadata.rdev()));

    // 3. 构建sysfs路径
    let sys_path = PathBuf::from(format!("/sys/dev/char/{}:{}", major, minor));
    if !sys_path.exists() {
        return None;
    }

    // 4. 收集设备信息
    let mut info = HashMap::new();

    // 固定显示完整设备路径
    info.insert("DEVNAME".into(), devpath.to_string());

    // 从uevent文件获取信息（但保留我们的DEVNAME）
    if let Ok(uevent) = fs::read_to_string(sys_path.join("uevent")) {
        for line in uevent.lines() {
            if let Some((k, v)) = line.split_once('=') {
                // 不覆盖DEVNAME
                if k != "DEVNAME" {
                    info.insert(k.into(), v.into());
                }
            }
        }
    }

    // 5. 获取子系统信息
    if let Ok(subsystem) = fs::read_link(sys_path.join("subsystem")) {
        if let Some(name) = subsystem.file_name() {
            info.insert("SUBSYSTEM".into(), name.to_string_lossy().into_owned());
        }
    }

    // 6. 获取设备类型
    if let Ok(devtype) = fs::read_to_string(sys_path.join("type")) {
        info.insert("DEVTYPE".into(), devtype.trim().to_string());
    }

    // 获取驱动信息
    if let Ok(driver) = fs::read_link(sys_path.join("device/driver")) {
        if let Some(name) = driver.file_name() {
            info.insert("DRIVER".into(), name.to_string_lossy().into_owned());
        }
    }

    // 获取设备拓扑信息
    if let Ok(device_path) = fs::read_link(sys_path.join("device")) {
        info.insert("PHYSDEVPATH".into(), device_path.to_string_lossy().into_owned());
    }

    // 7. 添加权限信息
    info.insert("DEVMODE".into(), format!("{:o}", metadata.mode() & 0o777));

    Some(info)
}