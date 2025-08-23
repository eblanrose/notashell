use std::collections::HashMap;
use owo_colors::OwoColorize;
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
                    "red" => Arc::new(|s: &str| s.red().to_string()),
                    "green" => Arc::new(|s: &str| s.green().to_string()),
                    "blue" => Arc::new(|s: &str| s.blue().to_string()),
                    "yellow" => Arc::new(|s: &str| s.yellow().to_string()),
                    "orange" => Arc::new(|s: &str| s.fg_rgb::<255, 165, 0>().to_string()),
                    "cyan" => Arc::new(|s: &str| s.cyan().to_string()),
                    "magenta" => Arc::new(|s: &str| s.magenta().to_string()),
                    "white" => Arc::new(|s: &str| s.white().to_string()),
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

    let colors: Vec<(&str, fn(&str) -> String)> = vec![
        ("$blue", |s: &str| s.blue().to_string()),
        ("$green", |s: &str| s.green().to_string()),
        ("$red", |s: &str| s.red().to_string()),
        ("$yellow", |s: &str| s.yellow().to_string()),
        ("$orange", |s: &str| s.fg_rgb::<255, 165, 0>().to_string()),
        ("$purple", |s: &str| s.fg_rgb::<128, 0, 128>().to_string()),
        ("$cyan", |s: &str| s.cyan().to_string()),
        ("$pink", |s: &str| s.fg_rgb::<255, 105, 180>().to_string()),
        ("$gray", |s: &str| s.fg_rgb::<128, 128, 128>().to_string()),
        ("$white", |s: &str| s.white().to_string()),
        ("$clear", |_| "\x1b[0m".to_string()),
    ];

    let mut result = String::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == '$' && i + 1 < chars.len() {
            let mut matched = false;
            for (key, color_fn) in &colors {
                let key_chars: Vec<char> = key.chars().collect();
                if i + key_chars.len() <= chars.len() && &chars[i..i + key_chars.len()] == key_chars.as_slice() {
                    let key_len = key_chars.len();
                    if *key == "$clear" {
                        result.push_str(&color_fn(""));
                        i += key_len;
                    } else {
                        let word_start = i + key_len;
                        let mut word_end = word_start;
                        while word_end < chars.len() {
                            if chars[word_end] == ' ' || chars[word_end] == '\n' || chars[word_end] == '\t' || chars[word_end] == '$' {
                                break;
                            }
                            word_end += 1;
                        }
                        let byte_start = s.chars().take(word_start).map(|c| c.len_utf8()).sum::<usize>();
                        let byte_end = s.chars().take(word_end).map(|c| c.len_utf8()).sum::<usize>();
                        let word = if word_start < chars.len() { &s[byte_start..byte_end] } else { "" };
                        result.push_str(&color_fn(word));
                        i = word_end;
                    }
                    matched = true;
                    break;
                }
            }
            if !matched {
                result.push('$');
                i += 1;
            }
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }

    result + "\x1b[0m"
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