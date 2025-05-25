// src/udevd.rs

use std::io;
use std::os::fd::AsRawFd;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use nix::poll::{poll, PollFd, PollFlags};
use std::path::{Path, PathBuf};

use crate::actions::*;
use crate::device::{DeviceAction, UEventDevice};
use crate::monitor::UEventMonitor;
use crate::rules::matcher::Rule;
use crate::rules::parser::RuleManager;
use log::*;

const POLL_TIMEOUT: i32 = 100;

pub fn start_udevd() -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting udevd daemon...");

    let rule_paths = vec![Path::new("/home/rust_udev/rust_udev/rules/").to_path_buf()];
    let rule_manager = RuleManager::new(rule_paths); 

    let monitor = UEventMonitor::new()?;
    let poll_fd = PollFd::new(monitor.as_raw_fd(), PollFlags::POLLIN);

    loop {
        match poll(&mut [poll_fd], POLL_TIMEOUT) {
            Ok(0) => continue,
            Ok(_) => match monitor.receive_event() {
                Ok(event_map) => {
                    if let Some(device) = UEventDevice::from_event(event_map) {
                        let rules = rule_manager.get_rules();
                        process_event(device, rules);
                    } else {
                        warn!("Failed to parse event into UEventDevice");
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => continue,
                Err(e) => return Err(Box::new(e)),
            },
            Err(e) => {
                error!("Poll error: {}", e);
                thread::sleep(Duration::from_millis(1000));
            }
        }
    }
}

fn process_event(device: UEventDevice, rules: Arc<Mutex<Vec<Rule>>>) {
    rayon::spawn(move || {
        let rules = rules.lock().unwrap();
        let mut matched = false;

        if !device.is_usb_device() { return; }

        info!("Processing event: {}", device);

        for rule in &*rules {
            debug!("Checking rule: {:?}", rule);
            if rule.matches(&device) {
                matched = true;
                execute_rule_actions(rule, &device);
                break;
            }
        }

        if !matched {
            warn!("No rules matched for device: {}", device);
        }

        println!("---------------------------------------------------------------")
    });
}

pub fn execute_rule_actions(rule: &Rule, device: &UEventDevice) {
    info!("Executing rule actions for rule: {:?}", rule);

    let action = match device.action() {
        DeviceAction::Add => Some("add"),
        DeviceAction::Remove => Some("remove"),
        DeviceAction::Change => Some("change"),
        DeviceAction::Bind => Some("bind"),
        DeviceAction::Unbind => Some("unbind"),
        _ => None,
    };

    if let Some(devname) = device.devnode() {
        let dev_path = PathBuf::from("/home/rust_udev/testdev").join(devname);

        match action {
            Some("add") => {
                if let Err(e) = create_device_node(devname, device, rule) {
                    error!("Failed to create device node {}: {}", devname, e);
                    return;
                }
                if let Err(e) = create_symlinks(&dev_path, &rule.symlink, device) {
                    warn!("Failed to create symlink(s): {}", e);
                }
                if let Some(cmds) = rule.run.get("add") {
                    if let Err(e) = run_commands(cmds, device) {
                        warn!("Failed to execute add run commands: {}", e);
                    }
                }
            }
            Some("remove") => {
                let symlink_dir = Path::new("/home/rust_udev/testdev");

                if let Err(e) = remove_symlinks(&dev_path, symlink_dir) {
                    warn!("Failed to remove symlinks: {}", e);
                }

                if let Err(e) = remove_device_node(&dev_path) {
                    warn!("Failed to remove device node {}: {}", devname, e);
                }

                if let Some(cmds) = rule.run.get("remove") {
                    if let Err(e) = run_commands(cmds, device) {
                        warn!("Failed to execute remove run commands: {}", e);
                    }
                }
            }
            Some("change") | Some("bind") => {
                if let Err(e) = apply_mode(&dev_path, &rule.mode) {
                    warn!("Failed to re-apply mode: {}", e);
                }
                if let Err(e) = apply_owner(&dev_path, &rule.owner) {
                    warn!("Failed to re-apply owner: {}", e);
                }
                if let Err(e) = apply_group(&dev_path, &rule.group) {
                    warn!("Failed to re-apply group: {}", e);
                }
                if action == Some("bind") {
                    if let Err(e) = create_symlinks(&dev_path, &rule.symlink, device) {
                        warn!("Failed to create symlink(s): {}", e);
                    }
                    if let Some(cmds) = rule.run.get("bind") {
                        if let Err(e) = run_commands(cmds, device) {
                            warn!("Failed to execute bind run commands: {}", e);
                        }
                    }
                }
            }
            Some("unbind") => {
                let symlink_dir = Path::new("/home/rust_udev/testdev");
                if let Err(e) = remove_symlinks(&dev_path, symlink_dir) {
                    warn!("Failed to remove symlinks: {}", e);
                }
                if let Some(cmds) = rule.run.get("unbind") {
                    if let Err(e) = run_commands(cmds, device) {
                        warn!("Failed to execute unbind run commands: {}", e);
                    }
                }
            }
            Some(other) => {
                warn!("Unsupported ACTION '{}'", other);
            }
            None => {
                warn!("No supported ACTION in device, skipping rule execution.");
            }
        }
    } else {
        warn!("No DEVNAME in device, cannot execute rule actions.");
    }
}
