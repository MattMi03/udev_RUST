// src/udevd.rs

use std::collections::HashMap;
use std::io;
use std::os::fd::AsRawFd;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use nix::poll::{poll, PollFd, PollFlags};
use std::path::PathBuf;
// use nix::unistd::{chown, Gid, Uid};

use crate::actions::*;
use crate::monitor::UEventMonitor;
use crate::rules::matcher::Rule;
use crate::rules::parser::parse_rules_file;
use log::{error, info, warn};

const POLL_TIMEOUT: i32 = 100; // milliseconds

pub fn start_udevd() -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting udevd daemon...");

    let rules = Arc::new(Mutex::new(parse_rules_file("rules")?));
    let monitor = UEventMonitor::new()?;
    let poll_fd = PollFd::new(monitor.as_raw_fd(), PollFlags::POLLIN);

    loop {
        match poll(&mut [poll_fd], POLL_TIMEOUT) {
            Ok(0) => continue,
            Ok(_) => match monitor.receive_event() {
                Ok(event) => process_event(event, Arc::clone(&rules)),
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
        let rules = rules.lock().unwrap();
        let mut matched = false;

        if event.get("SUBSYSTEM") != Some(&"usb".to_string()) {
            return;
        }

        info!("Processing event: {:?}", event);

        for rule in &*rules {
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

// pub struct Rule {
//     // 基本字段匹配
//     pub action: Option<String>,
//     pub kernel: Option<String>,
//     pub subsystem: Option<String>,
//     pub driver: Option<String>,
//     pub devpath: Option<String>,

//     // 属性和环境变量匹配
//     pub sysfs_attrs: Vec<(String, String)>, // SYSFS{key}=="value"
//     pub env_vars: Vec<(String, String)>,    // ENV{key}=="value"

//     // 文件创建控制
//     pub name: Option<String>,  // NAME="xxx"
//     pub symlink: Vec<String>,  // SYMLINK+="foo"
//     pub owner: Option<String>, // OWNER="user"
//     pub group: Option<String>, // GROUP="plugdev"
//     pub mode: Option<String>,  // MODE="0660"

//     // 运行操作
//     pub run: Vec<String>,        // RUN+="/bin/foo"
//     pub program: Option<String>, // PROGRAM=="/usr/bin/foo"

//     // 内部跳转控制
//     pub label: Option<String>, // LABEL="mylabel"
//     pub goto: Option<String>,  // GOTO="mylabel"

//     // 其他标志
//     pub ignore_device: bool, // OPTIONS+="ignore_device"
//     pub last_rule: bool,     // OPTIONS+="last_rule"
// }

pub fn execute_rule_actions(rule: &Rule, event: &HashMap<String, String>) {
    let action = event.get("ACTION").map(|s| s.as_str());

    if let Some(devname) = event.get("DEVNAME") {
        let dev_path = PathBuf::from("/home/rust_udev/testdev").join(devname);

        match action {
            Some("add") => {
                if let Err(e) = create_device_node(devname, event, rule) {
                    error!("Failed to create device node {}: {}", devname, e);
                    return;
                }

                if let Err(e) = apply_mode(&dev_path, &rule.mode) {
                    warn!("Failed to apply mode: {}", e);
                }

                if let Err(e) = apply_owner(&dev_path, &rule.owner) {
                    warn!("Failed to apply owner: {}", e);
                }

                if let Err(e) = apply_group(&dev_path, &rule.group) {
                    warn!("Failed to apply group: {}", e);
                }

                if let Err(e) = create_symlinks(&dev_path, &rule.symlink, event) {
                    warn!("Failed to create symlink(s): {}", e);
                }

                if let Err(e) = run_commands(&rule.run, event) {
                    warn!("Failed to execute run commands: {}", e);
                }
            }

            Some("remove") => {
                // 这里只是做符号链接删除，节点不一定手动删（udev 默认由内核管理）
                for link in &rule.symlink {
                    let path = PathBuf::from("/home/rust_udev/testdev").join(link);
                    if path.exists() {
                        if let Err(e) = std::fs::remove_file(&path) {
                            warn!("Failed to remove symlink {:?}: {}", path, e);
                        }
                    }
                }
            }

            Some("change") => {
                // change 事件一般会重新设置权限/属主/属组
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
