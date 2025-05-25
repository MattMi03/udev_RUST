// src/rules/matcher.rs

use std::collections::HashMap;

use crate::device::UEventDevice;

#[derive(Debug, Clone)]
pub struct Rule {
    // 基本字段匹配
    pub action: Option<String>,
    pub kernel: Option<String>,
    pub subsystem: Option<String>,
    pub driver: Option<String>,
    pub devpath: Option<String>,
    pub tag: Option<String>,

    // 属性和环境变量匹配
    pub attr: Vec<(String, String)>,
    pub env_vars: Vec<(String, String)>,

    // 文件创建控制
    pub name: Option<String>,
    pub symlink: Vec<String>,
    pub owner: Option<String>,
    pub group: Option<String>,
    pub mode: Option<String>,

    // 运行操作
    pub run: HashMap<String, Vec<String>>,
    pub program: Option<String>,

    // 内部跳转控制
    pub label: Option<String>,
    pub goto: Option<String>,

    // 其他标志
    pub ignore_device: bool,
    pub last_rule: bool,
}

impl Rule {
    pub fn matches(&self, device: &UEventDevice) -> bool {
        let has_conditions = self.action.is_some()
            || self.subsystem.is_some()
            || self.kernel.is_some()
            || self.devpath.is_some()
            || self.driver.is_some()
            || self.tag.is_some()
            || !self.env_vars.is_empty()
            || !self.attr.is_empty();

        if !has_conditions {
            return false;
        }

        if let Some(action) = &self.action {
            let dev_action = format!("{:?}", device.action()).to_lowercase();
            if dev_action != action.to_lowercase() {
                return false;
            }
        }

        if let Some(subsystem) = &self.subsystem {
            if device.subsystem().to_lowercase() != subsystem.to_lowercase() {
                return false;
            }
        }

        if let Some(kernel) = &self.kernel {
            if device.kernel().map_or(true, |k| k.to_lowercase() != kernel.to_lowercase()) {
                return false;
            }
        }

        if let Some(devpath) = &self.devpath {
            if device.devpath().to_string_lossy().to_lowercase() != devpath.to_lowercase() {
                return false;
            }
        }

        if let Some(driver) = &self.driver {
            if device.driver().map_or(true, |d| d.to_lowercase() != driver.to_lowercase()) {
                return false;
            }
        }

        if let Some(tag) = &self.tag {
            if device.properties().get("TAG").map_or(true, |t| t.to_lowercase() != tag.to_lowercase()) {
                return false;
            }
        }

        for (key, value) in &self.env_vars {
            if device.properties().get(key).map_or(true, |v| v != value) {
                return false;
            }
        }

        let sys_path = device.syspath();
        for (key, value) in &self.attr {
            let attr_path = sys_path.join(key);
            match std::fs::read_to_string(&attr_path) {
                Ok(content) => {
                    if content.trim() != value {
                        return false;
                    }
                }
                Err(_) => return false,
            }
        }

        true
    }
}
