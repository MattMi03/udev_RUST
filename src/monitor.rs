// src/monitor.rs
use nix::sys::socket::{
    socket, bind, recv, AddressFamily, SockType, SockFlag,
    NetlinkAddr, MsgFlags, SockProtocol
};
use nix::unistd::close;
use std::io;
use std::os::unix::io::{RawFd, AsRawFd};
use std::collections::HashMap;
use log::{info, warn, error};

pub struct UEventMonitor {
    fd: RawFd,
}

#[allow(dead_code)]
impl UEventMonitor {
    pub fn new() -> io::Result<Self> {
        let protocol = SockProtocol::NetlinkKObjectUEvent;

        let fd = socket(
            AddressFamily::Netlink,
            SockType::Raw,
            SockFlag::empty(),
            Some(protocol)
        ).map_err(|e| {
            error!("Socket creation failed: {}", e);
            io::Error::new(io::ErrorKind::Other, format!("socket error: {e}"))
        })?;

        let addr = NetlinkAddr::new(0, 1);
        bind(fd, &addr).map_err(|e| {
            error!("Socket binding failed: {}", e);
            io::Error::new(io::ErrorKind::Other, format!("bind error: {e}"))
        })?;

        info!("UEvent monitor initialized");
        Ok(Self { fd })
    }

    pub fn receive_event(&self) -> io::Result<HashMap<String, String>> {
        let mut buf = [0u8; 4096];

        match recv(self.fd, &mut buf, MsgFlags::empty()) {
            Ok(size) if size > 0 => {
                let msg = String::from_utf8_lossy(&buf[..size]);
                let mut event_map = HashMap::new();

                for field in msg.split('\0') {
                    if let Some((k, v)) = field.split_once('=') {
                        // println!("Key: {}, Value: {}", k, v);
                        event_map.insert(k.to_string(), v.to_string());
                    }
                }

                Ok(event_map)
            },
            Ok(_) => {
                warn!("Empty packet received");
                Err(io::ErrorKind::WouldBlock.into())
            },
            Err(e) if e == nix::errno::Errno::EAGAIN => {
                Err(io::ErrorKind::WouldBlock.into())
            },
            Err(e) => {
                error!("Receive error: {}", e);
                Err(io::Error::new(io::ErrorKind::Other, format!("recv error: {e}")))
            }
        }
    }
}

impl Drop for UEventMonitor {
    fn drop(&mut self) {
        if let Err(e) = close(self.fd) {
            error!("Failed to close socket: {}", e);
        }
    }
}

impl AsRawFd for UEventMonitor {
    fn as_raw_fd(&self) -> RawFd {
        self.fd
    }
}