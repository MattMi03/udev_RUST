use crate::rules::matcher::Rule;
use log::*;
use regex::Regex;
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead};
use notify::{Watcher, RecommendedWatcher, RecursiveMode, EventKind};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use crossbeam::channel::{unbounded, Receiver};

#[allow(dead_code)]
#[derive(Debug)]
pub struct RuleManager {
    rules: Arc<Mutex<Vec<Rule>>>,
    watcher: RecommendedWatcher,
    paths: Vec<PathBuf>,
}

impl RuleManager {
    pub fn new(rule_paths: Vec<PathBuf>) -> Self {
        // 初始加载规则
        let rules = match load_all_rules(&rule_paths) {
            Ok(r) => Arc::new(Mutex::new(r)),
            Err(e) => {
                warn!("Failed to load initial rules: {}", e);
                Arc::new(Mutex::new(Vec::new()))
            }
        };

        let (tx, rx) = unbounded();

        let mut watcher = notify::recommended_watcher(move |res| {
            if let Ok(event) = res {
                tx.send(event).unwrap();
            }
        })
        .unwrap();

        for path in &rule_paths {
            watcher
                .watch(path, RecursiveMode::NonRecursive)
                .unwrap_or_else(|e| {
                    warn!("Failed to watch {}: {}", path.display(), e);
                });
        }

        let rules_clone = rules.clone();
        let paths_clone = rule_paths.clone();
        thread::spawn(move || {
            Self::reload_loop(rx, rules_clone, paths_clone);
        });

        Self {
            rules,
            watcher,
            paths: rule_paths,
        }
    }

    pub fn get_rules(&self) -> Arc<Mutex<Vec<Rule>>> {
        self.rules.clone()
    }

    fn reload_loop(rx: Receiver<notify::Event>, rules: Arc<Mutex<Vec<Rule>>>, paths: Vec<PathBuf>) {
        for event in rx {
            if matches!(event.kind, EventKind::Modify(_)) {
                info!("Rules directory modified, triggering reload...");
                match load_all_rules(&paths) {
                    Ok(new_rules) => {
                        *rules.lock().unwrap() = new_rules;
                        info!(
                            "Successfully reloaded {} rules",
                            rules.lock().unwrap().len()
                        );
                    }
                    Err(e) => warn!("Rule reload failed: {}", e),
                }
            }
        }
    }
}

fn load_all_rules<P: AsRef<Path>>(paths: &[P]) -> io::Result<Vec<Rule>> {
    let mut all_rules = Vec::new();
    for path in paths {
        let path = path.as_ref();
        if let Ok(rules) = parse_rules_file(path) {
            all_rules.extend(rules);
        }
    }
    Ok(all_rules)
}

pub fn parse_rules_file<P: AsRef<Path>>(path: P) -> io::Result<Vec<Rule>> {

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
                run: HashMap::new(),
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
                        ("RUN", "+=") => {
                            if let Some(action) = &rule.action {
                                rule.run.entry(action.clone()).or_default().push(val);
                            } else {
                                warn!(
                                    "RUN+=... found without ACTION==..., ignoring command: {}",
                                    val
                                );
                            }
                        }

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
