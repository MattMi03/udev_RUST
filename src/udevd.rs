// src/udevd.rs

use std::collections::HashMap;
use std::io;
use std::os::fd::AsRawFd;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use nix::poll::{poll, PollFd, PollFlags};
use std::path::{Path, PathBuf};

use crate::actions::*;
use crate::monitor::UEventMonitor;
use crate::rules::matcher::Rule;
use crate::rules::parser::RuleManager;
use log::*;

const POLL_TIMEOUT: i32 = 100;

pub fn start_udevd() -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting udevd daemon...");

    let rule_paths = vec![Path::new("rules").to_path_buf()]; // 将 &Path 转换为 PathBuf
    let rule_manager = RuleManager::new(rule_paths); // 初始化 RuleManager

    let monitor = UEventMonitor::new()?; // Netlink 监视器
    let poll_fd = PollFd::new(monitor.as_raw_fd(), PollFlags::POLLIN);

    loop {
        match poll(&mut [poll_fd], POLL_TIMEOUT) {
            Ok(0) => continue,
            Ok(_) => match monitor.receive_event() {
                Ok(event) => {
                    let rules = rule_manager.get_rules(); // 获取最新的规则
                    process_event(event, rules);
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


fn process_event(event: HashMap<String, String>, rules: Arc<Mutex<Vec<Rule>>>) {
    rayon::spawn(move || {
        let rules = rules.lock().unwrap();  // 锁定规则
        let mut matched = false;

        if event.get("DEVTYPE") != Some(&"usb_device".to_string()) {
            return;
        }

        if !matches!(event.get("ACTION"), Some(action) if ["add", "remove", "change", "bind", "unbind"].contains(&action.as_str())) {
            return;
        }

        info!("Processing event: {:?}", event);

        for rule in &*rules {
            debug!("Checking rule: {:?}", rule);
            if rule.matches(&event) {
                matched = true;
                execute_rule_actions(rule, &event);
                break;
            }
        }

        if !matched {
            warn!("No rules matched for event: {:?}", event);
        }

        println!("---------------------------------------------------------------")
    });
}


pub fn execute_rule_actions(rule: &Rule, event: &HashMap<String, String>) {
    info!("Executing rule actions for rule: {:?}", rule);

    let action = event.get("ACTION").map(|s| s.as_str());

    if let Some(devname) = event.get("DEVNAME") {
        let dev_path = PathBuf::from("/home/rust_udev/testdev").join(devname);

        match action {
            Some("add") => {
                if let Err(e) = create_device_node(devname, event, rule) {
                    error!("Failed to create device node {}: {}", devname, e);
                    return;
                }

                if let Err(e) = create_symlinks(&dev_path, &rule.symlink, event) {
                    warn!("Failed to create symlink(s): {}", e);
                }

                if let Some(cmds) = rule.run.get("add") {
                    if let Err(e) = run_commands(cmds, event) {
                        warn!("Failed to execute bind run commands: {}", e);
                    }
                }
            }

            Some("remove") => {
                if let Some(devname) = event.get("DEVNAME") {
                    let dev_path = PathBuf::from("/home/rust_udev/testdev").join(devname);
                    let symlink_dir = Path::new("/home/rust_udev/testdev");

                    if let Err(e) = remove_symlinks(&dev_path, symlink_dir) {
                        warn!("Failed to remove symlinks: {}", e);
                    }
                }

                if let Err(e) = remove_device_node(&dev_path) {
                    warn!("Failed to remove device node {}: {}", devname, e);
                }

                if let Some(cmds) = rule.run.get("remove") {
                    if let Err(e) = run_commands(cmds, event) {
                        warn!("Failed to execute bind run commands: {}", e);
                    }
                }
            }

            Some("change") => {
                if let Err(e) = apply_mode(&dev_path, &rule.mode) {
                    warn!("Failed to re-apply mode: {}", e);
                }

                if let Err(e) = apply_owner(&dev_path, &rule.owner) {
                    warn!("Failed to re-apply owner: {}", e);
                }

                if let Err(e) = apply_group(&dev_path, &rule.group) {
                    warn!("Failed to re-apply group: {}", e);
                }
            }

            Some("bind") => {
                if let Err(e) = apply_mode(&dev_path, &rule.mode) {
                    warn!("Failed to re-apply mode: {}", e);
                }

                if let Err(e) = apply_owner(&dev_path, &rule.owner) {
                    warn!("Failed to re-apply owner: {}", e);
                }

                if let Err(e) = apply_group(&dev_path, &rule.group) {
                    warn!("Failed to re-apply group: {}", e);
                }

                if let Err(e) = create_symlinks(&dev_path, &rule.symlink, event) {
                    warn!("Failed to create symlink(s): {}", e);
                }

                if let Some(cmds) = rule.run.get("bind") {
                    if let Err(e) = run_commands(cmds, event) {
                        warn!("Failed to execute bind run commands: {}", e);
                    }
                }
            }

            Some("unbind") => {
                if let Some(devname) = event.get("DEVNAME") {
                    let dev_path = PathBuf::from("/home/rust_udev/testdev").join(devname);
                    let symlink_dir = Path::new("/home/rust_udev/testdev");

                    if let Err(e) = remove_symlinks(&dev_path, symlink_dir) {
                        warn!("Failed to remove symlinks: {}", e);
                    }
                }

                if let Some(cmds) = rule.run.get("unbind") {
                    if let Err(e) = run_commands(cmds, event) {
                        warn!("Failed to execute unbind run commands: {}", e);
                    }
                }
            }

            Some(other) => {
                warn!("Unsupported ACTION '{}'", other);
            }

            None => {
                warn!("No ACTION in event, skipping rule execution.");
            }
        }
    } else {
        warn!("No DEVNAME in event, cannot execute rule actions.");
    }
}
