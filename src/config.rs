use owo_colors::OwoColorize;
use std::fs;
use std::path::Path;
use std::sync::Arc;

// ------------------- Highlight args -------------------
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

// ------------------- Prompt -------------------
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

// ------------------- Config Directory -------------------
pub(crate) fn ensure_config_dir(home: &Path) {
    let config_dir = home.join(".config/anssh");
    if !config_dir.exists() {
        fs::create_dir_all(&config_dir).expect("Failed to create config directory");
    }
}