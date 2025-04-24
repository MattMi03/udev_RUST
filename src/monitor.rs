use nix::sys::socket::{
    socket, bind, recv, AddressFamily, SockType, SockFlag,
    NetlinkAddr, MsgFlags, SockProtocol
};
use std::io;
use std::os::unix::io::RawFd;

fn create_uevent_socket() -> std::io::Result<RawFd> {
    // 构造 Netlink 协议类型（关键修正）
    let protocol = SockProtocol::NetlinkKObjectUEvent; // 使用枚举常量代替魔数 15

    let fd = socket(
        AddressFamily::Netlink,
        SockType::Datagram,
        SockFlag::empty(),
        Some(protocol) // 必须用 Some 包裹协议类型
    ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("socket error: {e}")))?;

    // 绑定到组1接收广播
    let addr = NetlinkAddr::new(0, 1);
    bind(fd, &addr)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("bind error: {e}")))?;

    Ok(fd)
}

// start_monitor函数保持不变

/// 开始监听 udev uevent
pub fn start_monitor() -> io::Result<()> {
    let fd = create_uevent_socket()?;

    println!("🔌 Listening for udev events... (Ctrl+C to stop)");

    loop {
        let mut buf = [0u8; 4096];
        let size = recv(fd, &mut buf, MsgFlags::empty())
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("recv error: {e}")))?;

        if size > 0 {
            // 打印接收到的原始字节
            println!("📥 Raw uevent received: {:?}", &buf[..size]);

            // 你可以选择在这里尝试解析消
            let msg = String::from_utf8_lossy(&buf[..size]);
            println!("<UNK> Monitor received:");
            for field in msg.split('\0') {
                if !field.is_empty() {
                    println!("KeyValue: {}", field);
                }
            }
            println!("=========================================");
        }
    }
}
