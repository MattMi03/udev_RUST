use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

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
    pub fn matches(&self, event: &std::collections::HashMap<String, String>) -> bool {
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
            if event.get("ACTION") != Some(action) {
                return false;
            }
        }

        if let Some(subsystem) = &self.subsystem {
            if event.get("SUBSYSTEM") != Some(subsystem) {
                return false;
            }
        }

        if let Some(kernel) = &self.kernel {
            if event.get("KERNEL") != Some(kernel) {
                return false;
            }
        }

        if let Some(devpath) = &self.devpath {
            if event.get("DEVPATH") != Some(devpath) {
                return false;
            }
        }

        if let Some(driver) = &self.driver {
            if event.get("DRIVER") != Some(driver) {
                return false;
            }
        }

        if let Some(tag) = &self.tag {
            if event.get("TAG") != Some(tag) {
                return false;
            }
        }

        for (key, value) in &self.env_vars {
            if let Some(env_value) = event.get(key) {
                if env_value != value {
                    return false;
                }
            } else {
                return false;
            }
        }

        if let Some(sys_path) = event.get("DEVPATH") {
            for (key, value) in &self.attr {
                let mut attr_path = PathBuf::from("/sys");
                attr_path.push(sys_path);
                attr_path.push(key);

                match fs::read_to_string(&attr_path) {
                    Ok(content) => {
                        let content = content.trim();
                        if content != value {
                            return false;
                        }
                    }
                    Err(_) => return false,
                }
            }
        } else if !self.attr.is_empty() {
            return false;
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rule_matches() {
        let rule = Rule {
            tag: None,
            action: Some("add".to_string()),
            kernel: Some("sda".to_string()),
            subsystem: Some("block".to_string()),
            driver: None,
            devpath: None,
            attr: Vec::new(),
            env_vars: Vec::new(),
            name: None,
            symlink: Vec::new(),
            owner: None,
            group: None,
            mode: None,
            run: HashMap::new(),
            program: None,
            label: None,
            goto: None,
            ignore_device: false,
            last_rule: false,
        };

        let mut event = std::collections::HashMap::new();
        event.insert("ACTION".to_string(), "add".to_string());
        event.insert("SUBSYSTEM".to_string(), "block".to_string());
        event.insert("KERNEL".to_string(), "sda".to_string());

        assert!(rule.matches(&event));

        event.remove("ACTION");
        assert!(!rule.matches(&event));
    }
}
