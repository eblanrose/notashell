use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub(crate) fn load_highlight_rules(path: &Path) -> Vec<(glob::Pattern, Arc<dyn Fn(&str) -> String + Send + Sync>)> {
    let mut rules = Vec::new();
    if let Ok(lines) = fs::read_to_string(path) {
        for line in lines.lines() {
            let line = line.trim();
            if let Some((pattern, color)) = line.split_once(':') {
                let pattern = pattern.trim();
                let color = color.trim().to_lowercase();
                let func: Arc<dyn Fn(&str) -> String + Send + Sync> = match color.as_str() {
                    "red" => Arc::new(|s: &str| format!("\x1b[31m{}\x1b[0m", s)),
                    "green" => Arc::new(|s: &str| format!("\x1b[32m{}\x1b[0m", s)),
                    "blue" => Arc::new(|s: &str| format!("\x1b[34m{}\x1b[0m", s)),
                    "yellow" => Arc::new(|s: &str| format!("\x1b[33m{}\x1b[0m", s)),
                    "orange" => Arc::new(|s: &str| format!("\x1b[38;5;214m{}\x1b[0m", s)),
                    "cyan" => Arc::new(|s: &str| format!("\x1b[36m{}\x1b[0m", s)),
                    "magenta" => Arc::new(|s: &str| format!("\x1b[35m{}\x1b[0m", s)),
                    "white" => Arc::new(|s: &str| format!("\x1b[37m{}\x1b[0m", s)),
                    _ => Arc::new(|s: &str| s.to_string()),
                };
                if let Ok(pat) = glob::Pattern::new(pattern) {
                    rules.push((pat, func));
                }
            }
        }
    }
    rules
}
pub(crate) fn parse_prompt_theme(template: &str, command: &str) -> String {
    let mut s = template.trim_end_matches('\n').to_string();
    let cwd = std::env::current_dir().unwrap_or_default().display().to_string();
    let user = std::env::var("USER").unwrap_or_else(|_| "user".to_string());
    let host = hostname::get()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    s = s.replace("{user}", &user);
    s = s.replace("{cwd}", &cwd);
    s = s.replace("{command}", command);
    s = s.replace("{host}", &host);

    let colors: Vec<(&str, &str)> = vec![
        ("$blue", "\x1b[34m"),
        ("$green", "\x1b[32m"),
        ("$red", "\x1b[31m"),
        ("$yellow", "\x1b[33m"),
        ("$orange", "\x1b[38;5;214m"),
        ("$purple", "\x1b[35m"),
        ("$cyan", "\x1b[36m"),
        ("$pink", "\x1b[38;5;205m"),
        ("$gray", "\x1b[90m"),
        ("$white", "\x1b[37m"),
        ("$clear", "\x1b[0m"),
    ];

    for (key, code) in colors {
        s = s.replace(key, code);
    }
    
    s
}

pub(crate) fn read_rc(rc_path: PathBuf, env_vars: &mut HashMap<String, String>, aliases: &mut HashMap<String, String>) {
    if let Ok(contents) = fs::read_to_string(rc_path) {
        for line in contents.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let line = line.strip_prefix("export ").unwrap_or(line).trim();

            if let Some(eq_pos) = line.find('=') {
                let (key, value) = line.split_at(eq_pos);
                let key = key.trim();
                let mut value = value[1..].trim();

                if (value.starts_with('"') && value.ends_with('"'))
                    || (value.starts_with('\'') && value.ends_with('\''))
                {
                    value = &value[1..value.len() - 1];
                }

                let mut expanded_value = String::new();
                let mut chars = value.chars().peekable();
                while let Some(c) = chars.next() {
                    if c == '$' {
                        let mut var_name = String::new();
                        while let Some(&ch) = chars.peek() {
                            if ch.is_alphanumeric() || ch == '_' {
                                var_name.push(ch);
                                chars.next();
                            } else {
                                break;
                            }
                        }
                        if let Some(val) = env_vars.get(&var_name) {
                            expanded_value.push_str(val);
                        }
                    } else {
                        expanded_value.push(c);
                    }
                }

                if key.starts_with("alias ") {
                    if let Some(alias_name) = key.strip_prefix("alias ") {
                        aliases.insert(alias_name.to_string(), expanded_value);
                    }
                } else {
                    env_vars.insert(key.to_string(), expanded_value.clone());
                    unsafe { std::env::set_var(key, expanded_value); }
                }
            }
        }
    }
}