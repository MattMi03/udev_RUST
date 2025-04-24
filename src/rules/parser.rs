use crate::rules::matcher::Rule;
use regex::Regex;
use std::fs::File;
use std::io::{self, BufRead};
use std::path::Path;

pub fn parse_rules_file<P: AsRef<Path>>(path: P) -> io::Result<Vec<Rule>> {
    // 循环读取目录下的所有文件

    let mut rules = Vec::new();

    let kv_re = Regex::new(
        r#"(?P<key>[A-Z_]+|ENV\{.*?\}|ATTR\{.*?\}|OPTIONS)(?P<op>==|\+=|\=)(?P<val>".*?")"#,
    )
    .unwrap();

    let path = path.as_ref();

    let mut entries: Vec<_> = path.read_dir()?.filter_map(Result::ok).collect();

    entries.sort_by_key(|entry| {
        entry
            .file_name()
            .to_string_lossy()
            .split(|c: char| !c.is_digit(10))
            .filter_map(|s| s.parse::<u32>().ok())
            .next()
            .unwrap_or(0)
    });

    // println!("entries: {:?}", entries);

    for entry in entries {
        // println!("sub_path: {:?}", entry.path());

        let file = File::open(entry.path())?;
        let reader = io::BufReader::new(file);

        // 常用字段的正则

        for line in reader.lines().flatten() {
            let line = line.trim();
            if line.starts_with('#') || line.is_empty() {
                continue;
            }

            let mut rule = Rule {
                action: None,
                kernel: None,
                subsystem: None,
                driver: None,
                devpath: None,
                tag: None,
                attr: Vec::new(),
                env_vars: Vec::new(),
                name: None,
                symlink: Vec::new(),
                owner: None,
                group: None,
                mode: None,
                run: Vec::new(),
                program: None,
                label: None,
                goto: None,
                ignore_device: false,
                last_rule: false,
            };

            for cap in kv_re.captures_iter(line) {
                let raw_key = &cap["key"];
                let op = &cap["op"];
                let val = cap["val"].trim_matches('"').to_string();

                if raw_key.starts_with("ENV{") {
                    let key = raw_key.trim_start_matches("ENV{").trim_end_matches('}');
                    rule.env_vars.push((key.to_string(), val));
                } else if raw_key.starts_with("ATTR{") {
                    let key = raw_key.trim_start_matches("ATTR{").trim_end_matches('}');
                    rule.attr.push((key.to_string(), val));
                } else {
                    match (raw_key, op) {
                        ("ACTION", "==") => rule.action = Some(val),
                        ("KERNEL", "==") => rule.kernel = Some(val),
                        ("SUBSYSTEM", "==") => rule.subsystem = Some(val),
                        ("DRIVER", "==") => rule.driver = Some(val),
                        ("DEVPATH", "==") => rule.devpath = Some(val),
                        ("TAG", "==") => rule.tag = Some(val),
                        ("NAME", "==") => rule.name = Some(val),
                        ("SYMLINK", "+=") => rule.symlink.push(val),
                        ("OWNER", "=") => rule.owner = Some(val),
                        ("GROUP", "=") => rule.group = Some(val),
                        ("MODE", "=") => rule.mode = Some(val),
                        ("RUN", "+=") => rule.run.push(val),
                        ("PROGRAM", "==") => rule.program = Some(val),
                        ("LABEL", "=") => rule.label = Some(val),
                        ("GOTO", "=") => rule.goto = Some(val),
                        ("OPTIONS", "+=") => {
                            if val == "ignore_device" {
                                rule.ignore_device = true;
                            } else if val == "last_rule" {
                                rule.last_rule = true;
                            }
                        }
                        _ => {}
                    }
                }
            }

            rules.push(rule);
        }
    }

    Ok(rules)
}

#[cfg(test)]
mod tests {
    use super::*;

    // // 示例规则文件内容

    // 99-custom.rules
    // SUBSYSTEM=="usb", ACTION=="add", DEVTYPE=="usb_device", \
    // MODE="0660", OWNER="root", GROUP="plugdev", \
    // SYMLINK+="my_usb_device", \
    // RUN+="/bin/echo USB inserted"

    // 100-custom.rules
    // SUBSYSTEM=="usb", ACTION=="remove", DEVTYPE=="usb_device", OWNER="root", GROUP="plugdev", SYMLINK+="my_usb_device", RUN+="/bin/echo USB remove",  ENV{ID_BUS}=="bluetooth"
    #[test]
    fn test_parse_rules_file() {
        let rules = parse_rules_file("rules").unwrap();

        assert_eq!(rules.len(), 2);
        let rule = &rules[0];
        assert_eq!(rule.subsystem.as_deref(), Some("usb"));
        assert_eq!(rule.action.as_deref(), Some("add"));
        assert_eq!(rule.devpath.as_deref(), None);
        assert_eq!(rule.kernel.as_deref(), None);
        assert_eq!(rule.attr.len(), 0);
        assert_eq!(rule.env_vars.len(), 0);
        assert_eq!(rule.name.as_deref(), None);
        assert_eq!(rule.symlink.len(), 1);
        assert_eq!(rule.symlink[0], "my_usb_device");
        assert_eq!(rule.owner.as_deref(), Some("root"));
        assert_eq!(rule.group.as_deref(), Some("plugdev"));
        assert_eq!(rule.mode.as_deref(), Some("0660"));
        assert_eq!(rule.run.len(), 1);
        assert_eq!(rule.run[0], "/bin/echo USB inserted");
        assert_eq!(rule.program.as_deref(), None);
        assert_eq!(rule.label.as_deref(), None);
        assert_eq!(rule.goto.as_deref(), None);
        assert_eq!(rule.ignore_device, false);
        assert_eq!(rule.last_rule, false);

        let rule = &rules[1];
        assert_eq!(rule.subsystem.as_deref(), Some("usb"));
        assert_eq!(rule.action.as_deref(), Some("remove"));
        assert_eq!(rule.devpath.as_deref(), None);
        assert_eq!(rule.kernel.as_deref(), None);
        assert_eq!(rule.attr.len(), 0);
        assert_eq!(rule.env_vars.len(), 1);
        assert_eq!(
            rule.env_vars[0],
            ("ID_BUS".to_string(), "bluetooth".to_string())
        );
        assert_eq!(rule.name.as_deref(), None);
        assert_eq!(rule.symlink.len(), 1);
        assert_eq!(rule.symlink[0], "my_usb_device");
        assert_eq!(rule.owner.as_deref(), Some("root"));
        assert_eq!(rule.group.as_deref(), Some("plugdev"));
        assert_eq!(rule.mode.as_deref(), None);
        assert_eq!(rule.run.len(), 1);
        assert_eq!(rule.run[0], "/bin/echo USB remove");
        assert_eq!(rule.program.as_deref(), None);
        assert_eq!(rule.label.as_deref(), None);
        assert_eq!(rule.goto.as_deref(), None);
        assert_eq!(rule.ignore_device, false);
        assert_eq!(rule.last_rule, false);
    }
}
