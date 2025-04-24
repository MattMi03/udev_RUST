use std::collections::HashMap;
use rust_udev::rules::parser::parse_rules_file;
use rust_udev::rules::matcher::Rule;

pub fn test_rule_match() {
    // 模拟一个 uevent 事件
    let mut event = HashMap::new();
    event.insert("SUBSYSTEM".into(), "tty".into());
    event.insert("KERNEL".into(), "ttyUSB0".into());
    event.insert("DRIVER".into(), "usbserial".into());
    event.insert("ACTION".into(), "add".into());

    // 解析规则文件
    match parse_rules_file("rules/99-custom.rules") {
        Ok(rules) => {
            let mut matched = false;
            for rule in &rules {
                if rule.matches(&event) {
                    println!("✅ Rule matched: {:?}", rule);
                    matched = true;
                }
            }
            if !matched {
                println!("❌ No rules matched the test event.");
            }
        }
        Err(e) => {
            eprintln!("⚠️ Failed to parse rules file: {}", e);
        }
    }
}
