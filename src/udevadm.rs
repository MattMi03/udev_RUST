// src/udevadm.rs

use crate::libudev::get_device_info;
use log::{info, error};

#[derive(Debug)]
pub enum UdevadmError {
    DeviceNotFound(String),
    IoError(String, std::io::Error),
    SysfsError(String),
}

impl std::fmt::Display for UdevadmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UdevadmError::DeviceNotFound(path) => write!(f, "Device not found: {}", path),
            UdevadmError::IoError(path, err) => write!(f, "IO Error on {}: {}", path, err),
            UdevadmError::SysfsError(path) => write!(f, "Error accessing sysfs for {}", path),
        }
    }
}

pub fn udevadm_info(device_path: &str) -> Result<(), UdevadmError> {
    info!("udevadm device path: {}", device_path);
    match get_device_info(device_path) {
        Some(info) => {
            for (key, value) in info {
                info!("{}={}", key, value);
            }
        }
        None => {
            error!("Device not found: {}", device_path);
            return Err(UdevadmError::DeviceNotFound(device_path.to_string()));
        }
    }

    Ok(())
}

pub fn udevadm_cli(device_path: &str) -> Result<(), UdevadmError> {
    udevadm_info(device_path)
}