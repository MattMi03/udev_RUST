use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fmt;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq)]
pub enum DeviceAction {
    Add,
    Remove,
    Change,
    Bind,
    Unbind,
    Move,
    Online,
    Offline,
    Unknown(String),
}

impl FromStr for DeviceAction {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "add" => Self::Add,
            "remove" => Self::Remove,
            "change" => Self::Change,
            "bind" => Self::Bind,
            "unbind" => Self::Unbind,
            "move" => Self::Move,
            "online" => Self::Online,
            "offline" => Self::Offline,
            _ => Self::Unknown(s.to_string()),
        })
    }
}

#[derive(Debug)]
pub struct UEventDevice {
    action: DeviceAction,
    devpath: PathBuf,
    subsystem: String,
    devtype: Option<String>,
    kernel: Option<String>,
    
    major: Option<u32>,
    minor: Option<u32>,
    devnum: Option<u64>,

    seqnum: u64,
    timestamp: u64,

    properties: HashMap<String, String>,
    sysattrs: HashMap<String, String>,
}

impl UEventDevice {
    pub fn from_event(event: HashMap<String, String>) -> Option<Self> {
        let action_str = event.get("ACTION")?;
        let subsystem = event.get("SUBSYSTEM")?.clone();
        
        let devpath = Path::new(event.get("DEVPATH")?).to_path_buf();
        
        let major = event.get("MAJOR").and_then(|s| s.parse().ok());
        let minor = event.get("MINOR").and_then(|s| s.parse().ok());
        
        Some(Self {
            action: DeviceAction::from_str(action_str).ok()?,
            devpath,
            subsystem,
            devtype: event.get("DEVTYPE").cloned(),
            major,
            minor,
            kernel: event.get("KERNEL").cloned(),
            devnum: event.get("DEVNUM").and_then(|s| s.parse().ok()),
            seqnum: event.get("SEQNUM").and_then(|s| s.parse().ok()).unwrap_or(0),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .ok()?
                .as_secs(),
            properties: event.clone(),
            sysattrs: HashMap::new(),
        })
    }

    pub fn devnode(&self) -> Option<&str> {
        self.properties.get("DEVNAME").map(|s| s.as_str())
    }

    pub fn driver(&self) -> Option<&str> {
        self.properties.get("DRIVER").map(|s| s.as_str())
    }

    pub fn syspath(&self) -> PathBuf {
        Path::new("/sys").join(&self.devpath)
    }

    pub fn devpath(&self) -> &Path {
        &self.devpath
    }

    pub fn is_block_device(&self) -> bool {
        self.subsystem == "block"
    }

    pub fn is_usb_device(&self) -> bool {
        self.devtype.as_deref() == Some("usb_device")
    }

    pub fn action(&self) -> &DeviceAction {
        &self.action
    }

    pub fn subsystem(&self) -> &str {
        &self.subsystem
    }

    pub fn devtype(&self) -> Option<&str> {
        self.devtype.as_deref()
    }

    pub fn kernel(&self) -> Option<&str> {
        self.kernel.as_deref()
    }

    pub fn major(&self) -> Option<u32> {
        self.major
    }

    pub fn minor(&self) -> Option<u32> {
        self.minor
    }

    pub fn devnum(&self) -> Option<u64> {
        self.devnum
    }

    pub fn seqnum(&self) -> u64 {
        self.seqnum
    }

    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }

    pub fn properties(&self) -> &HashMap<String, String> {
        &self.properties
    }

    pub fn sysattrs(&self) -> &HashMap<String, String> {
        &self.sysattrs
    }
}

impl fmt::Display for UEventDevice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let devtype_str = self.devtype.as_deref().unwrap_or("null");
        let kernel_str = self.kernel.as_deref().unwrap_or("null");
        let major_str = self.major.map_or("null".to_string(), |n| n.to_string());
        let minor_str = self.minor.map_or("null".to_string(), |n| n.to_string());
        let devnum_str = self.devnum.map_or("null".to_string(), |n| n.to_string());
        let devnode_str = self.devnode().unwrap_or("null");
        let driver_str = self.driver().unwrap_or("null");

        let properties_str = if self.properties.is_empty() {
            "null".to_string()
        } else {
            self.properties.iter()
                .map(|(k, v)| format!("    {}={}", k, v))
                .collect::<Vec<_>>()
                .join("\n")
        };

        let sysattrs_str = if self.sysattrs.is_empty() {
            "null".to_string()
        } else {
            self.sysattrs.iter()
                .map(|(k, v)| format!("    {}={}", k, v))
                .collect::<Vec<_>>()
                .join("\n")
        };

        write!(
            f,
            "UEventDevice {{\n\
            \x20\x20seqnum:      {},\n\
            \x20\x20timestamp:  {},\n\
            \x20\x20action:     {:?},\n\
            \x20\x20subsystem:  {},\n\
            \x20\x20devtype:    {},\n\
            \x20\x20kernel:     {},\n\
            \x20\x20major:      {},\n\
            \x20\x20minor:      {},\n\
            \x20\x20devnum:     {},\n\
            \x20\x20devpath:    \"{}\",\n\
            \x20\x20devnode:    {},\n\
            \x20\x20driver:     {},\n\
            \x20\x20properties: {{\n{}\n\x20\x20}},\n\
            \x20\x20sysattrs:   {{\n{}\n\x20\x20}}\n}}",
            self.seqnum,
            self.timestamp,
            self.action,
            self.subsystem,
            devtype_str,
            kernel_str,
            major_str,
            minor_str,
            devnum_str,
            self.devpath.display(),
            devnode_str,
            driver_str,
            properties_str,
            sysattrs_str
        )
    }
}