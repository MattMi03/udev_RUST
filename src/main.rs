mod monitor;
use rust_udev::udevd::start_udevd;
use rust_udev::udevadm::udevadm_cli;
use clap::{Command};
use log::{info, error};

fn run_udevadm() {
    // 处理 udevadm 子命令的逻辑
    let matches = Command::new("rust_udev")
        .version("1.0")
        .about("udev-like system in Rust")
        .subcommand(
            Command::new("udevadm")
                .about("udevadm utility for device management")
                .arg(
                    clap::Arg::new("path")
                        .help("The device path to query")
                        .required(true)
                        .value_parser(clap::value_parser!(String))
                        .long("path")
                        .short('p'),
                ),
        )
        .get_matches();

    if let Some(("udevadm", sub_matches)) = matches.subcommand() {
        if let Some(device_path) = sub_matches.get_one::<String>("path") {
            // 执行 udevadm 子命令并处理结果
            match udevadm_cli(device_path) {
                Ok(_) => {
                    info!("Successfully executed udevadm command for device {}", device_path);
                }
                Err(e) => {
                    error!("Error while running udevadm command: {}", e);
                }
            }
        }
    }
}

fn start_udevd_daemon() {
    // 启动守护进程
    info!("Starting udevd daemon...");
    if let Err(e) = start_udevd() {
        error!("Failed to start udevd daemon: {}", e);
    } else {
        info!("udevd daemon started successfully.");
    }
}

fn main() {
    // 初始化日志记录
    env_logger::init();
    info!("🚀 Starting rust_udev system...");

    // 如果有命令行输入子命令，就执行 udevadm，否则启动守护进程
    if std::env::args().any(|arg| arg == "udevadm") {
        run_udevadm();
    } else {
        start_udevd_daemon();
    }
}
