use std::collections::{HashMap, HashSet};
use std::{fs, thread};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use dirs::home_dir;
use glob::glob;
use owo_colors::OwoColorize;
use rustyline::{
    Editor, completion::{Completer, Pair}, highlight::Highlighter, Context, Result as RustyResult, Helper,
    hint::Hinter, validate::Validator,
};
use rustyline::error::ReadlineError;
use signal_hook::consts::SIGINT;

// ------------------- Helper (Completer + Highlighter + Hinter + Validator) -------------------
struct AnsshHelper {
    rules: Vec<String>,
    ignore: HashSet<String>,
    bin_paths: Vec<PathBuf>,
    highlight_rules: Vec<(glob::Pattern, Arc<dyn Fn(&str) -> String + Send + Sync>)>,
}

impl AnsshHelper {
    fn new(complete_file: &PathBuf, highlight_path: &Path) -> Self {
        let mut rules = Vec::new();
        let mut ignore = HashSet::new();
        if let Ok(lines) = fs::read_to_string(complete_file) {
            for line in lines.lines() {
                let line = line.trim();
                if line.starts_with('!') {
                    ignore.insert(line[1..].to_string());
                } else {
                    rules.push(line.to_string());
                }
            }
        }

        let highlight_rules = load_highlight_rules(highlight_path);

        Self {
            rules,
            ignore,
            bin_paths: vec![PathBuf::from("/usr/local/bin"), PathBuf::from("/bin")],
            highlight_rules,
        }
    }

    fn list_binaries(&self) -> Vec<String> {
        let mut bins = Vec::new();
        for dir in &self.bin_paths {
            if let Ok(entries) = fs::read_dir(dir) {
                for e in entries.flatten() {
                    if e.path().is_file() {
                        if let Some(name) = e.file_name().to_str() {
                            bins.push(name.to_string());
                        }
                    }
                }
            }
        }
        bins
    }
}

impl Completer for AnsshHelper {
    type Candidate = Pair;
    fn complete(
        &self,
        line: &str,
        _pos: usize,
        _ctx: &Context<'_>,
    ) -> RustyResult<(usize, Vec<Pair>)> {
        let mut matches = vec![];
        let tokens: Vec<&str> = line.split_whitespace().collect();
        if tokens.is_empty() {
            return Ok((0, matches));
        }
        let last = tokens.last().unwrap();

        for rule in &self.rules {
            if rule.contains("$directory") {
                if let Ok(entries) = fs::read_dir(".") {
                    for e in entries.flatten() {
                        if e.path().is_dir() {
                            if let Some(name) = e.file_name().to_str() {
                                matches.push(Pair {
                                    display: name.to_string(),
                                    replacement: name.to_string(),
                                });
                            }
                        }
                    }
                }
            }
            if rule.contains("$file") {
                if let Ok(entries) = fs::read_dir(".") {
                    for e in entries.flatten() {
                        if e.path().is_file() {
                            if let Some(name) = e.file_name().to_str() {
                                matches.push(Pair {
                                    display: name.to_string(),
                                    replacement: name.to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }

        if matches.is_empty() {
            for bin in self.list_binaries() {
                if !self.ignore.contains(&bin) && bin.starts_with(last) {
                    matches.push(Pair {
                        display: bin.clone(),
                        replacement: bin.clone(),
                    });
                }
            }
        }

        Ok((0, matches))
    }
}

impl Highlighter for AnsshHelper {
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> std::borrow::Cow<'l, str> {
        let mut output = String::new();
        let mut last_pos = 0;

        for (i, c) in line.char_indices() {
            if c == ' ' {
                let token = &line[last_pos..i];
                if !token.is_empty() {
                    if token == "|" || token == ">" || token == "<" {
                        output.push_str(&token.blue().to_string());
                    } else if token.starts_with('-') {
                        output.push_str(&token.yellow().to_string());
                    } else {
                        let mut highlighted = false;
                        for (pattern, func) in &self.highlight_rules {
                            if pattern.matches(token) {
                                output.push_str(&func(token));
                                highlighted = true;
                                break;
                            }
                        }
                        if !highlighted {
                            output.push_str(token);
                        }
                    }
                }
                output.push(' ');
                last_pos = i + 1;
            }
        }

        if last_pos < line.len() {
            let token = &line[last_pos..];
            if token == "|" || token == ">" || token == "<" {
                output.push_str(&token.blue().to_string());
            } else if token.starts_with('-') {
                output.push_str(&token.yellow().to_string());
            } else {
                let mut highlighted = false;
                for (pattern, func) in &self.highlight_rules {
                    if pattern.matches(token) {
                        output.push_str(&func(token));
                        highlighted = true;
                        break;
                    }
                }
                if !highlighted {
                    output.push_str(token);
                }
            }
        }

        std::borrow::Cow::Owned(output)
    }

    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(
        &'s self,
        prompt: &'p str,
        _default: bool,
    ) -> std::borrow::Cow<'p, str> {
        std::borrow::Cow::Borrowed(prompt)
    }
}

impl Hinter for AnsshHelper {
    type Hint = String;
    fn hint(&self, _line: &str, _pos: usize, _ctx: &Context<'_>) -> Option<Self::Hint> {
        None // No hinting needed for this shell
    }
}

impl Validator for AnsshHelper {
    fn validate(&self, _ctx: &mut rustyline::validate::ValidationContext) -> RustyResult<rustyline::validate::ValidationResult> {
        Ok(rustyline::validate::ValidationResult::Valid(None)) // Always valid input
    }
}

impl Helper for AnsshHelper {}

// ------------------- Highlight args -------------------
fn load_highlight_rules(path: &Path) -> Vec<(glob::Pattern, Arc<dyn Fn(&str) -> String + Send + Sync>)> {
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
fn parse_prompt_theme(template: &str, command: &str) -> String {
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
fn ensure_config_dir(home: &Path) {
    let config_dir = home.join(".config/anssh");
    if !config_dir.exists() {
        fs::create_dir_all(&config_dir).expect("Failed to create config directory");
    }
}

// ------------------- Main -------------------
fn main() {
    let home = home_dir().unwrap_or_else(|| PathBuf::from("."));
    ensure_config_dir(&home);
    let rc_path = home.join(".anssh_rc");
    let history_path = home.join(".anssh_history");
    let prompt_path = home.join(".config/anssh/prompt");
    let complete_path = home.join(".config/anssh/complete");
    let highlight_path = home.join(".config/anssh/highlight_rules");

    let helper = AnsshHelper::new(&complete_path, &highlight_path);
    let mut env_vars: HashMap<String, String> = std::env::vars().collect();
    let mut aliases: HashMap<String, String> = HashMap::new();

    if let Ok(contents) = fs::read_to_string(&rc_path) {
        for line in contents.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim();
                if key.starts_with("alias ") {
                    if let Some(alias_name) = key.strip_prefix("alias ") {
                        let mut val = value.to_string();
                        if (val.starts_with('"') && val.ends_with('"'))
                            || (val.starts_with('\'') && val.ends_with('\''))
                        {
                            val = val[1..val.len()-1].to_string();
                        }
                        aliases.insert(alias_name.to_string(), val);
                    }
                } else {
                    env_vars.insert(key.to_string(), value.to_string());
                    unsafe { std::env::set_var(key, value); }
                }
            }
        }
    }

    let mut rl = Editor::new().expect("Failed to create Editor");
    rl.set_helper(Some(helper));
    let _ = rl.load_history(&history_path);

    let term = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(SIGINT, Arc::clone(&term)).expect("Failed to register SIGINT");

    loop {
        owo_colors::set_override(true);
        let template = fs::read_to_string(&prompt_path).unwrap_or_else(|e| {
            eprintln!("Warning: Failed to read prompt file: {}", e);
            "anssh> ".to_string()
        });
        let prompt_str = parse_prompt_theme(&template, "");

        term.store(false, Ordering::Relaxed);

        let readline = rl.readline(&prompt_str);
        let mut input = match readline {
            Ok(line) => line.trim().to_string(),
            Err(ReadlineError::Interrupted) => {
                continue;
            }
            Err(ReadlineError::Eof) => {
                break;
            }
            Err(e) => {
                eprintln!("Error reading line: {}", e);
                break;
            }
        };

        if input.is_empty() {
            continue;
        }

        rl.add_history_entry(input.clone()).expect("Failed to add history entry");

        if input == "exit" {
            break;
        }

        for (k, v) in &env_vars {
            input = input.replace(&format!("${}", k), v);
        }

        if let Some(first_word) = input.split_whitespace().next() {
            if let Some(alias_value) = aliases.get(first_word) {
                let rest: String = input[first_word.len()..].trim().to_string();
                input = if rest.is_empty() {
                    alias_value.clone()
                } else {
                    format!("{} {}", alias_value, rest)
                };
            }
        }

        let commands: Vec<String> = input.split('|').map(|s| s.trim().to_string()).collect();
        let mut previous_process: Option<Child> = None;

        for (i, cmd) in commands.iter().enumerate() {
            let cmd_parts: Vec<String> = cmd.split_whitespace().map(|s| s.to_string()).collect();
            if cmd_parts.is_empty() {
                continue;
            }
            let cmd_name = &cmd_parts[0];
            let mut args: Vec<String> = cmd_parts[1..].to_vec();

            let mut expanded_args = Vec::new();
            for arg in &args {
                if arg.contains('*') {
                    for entry in glob(arg).unwrap().flatten() {
                        expanded_args.push(entry.to_string_lossy().to_string());
                    }
                } else {
                    expanded_args.push(arg.clone());
                }
            }
            args = expanded_args;

            let mut command = Command::new(cmd_name);
            command.args(&args);

            if let Some(prev) = previous_process {
                command.stdin(prev.stdout.unwrap());
            }
            if i == commands.len() - 1 {
                command.stdout(Stdio::inherit());
            } else {
                command.stdout(Stdio::piped());
            }

            let child = command.spawn();
            match child {
                Ok(c) => {
                    previous_process = Some(c);
                }
                Err(e) => {
                    eprintln!("anssh: {}", e);
                    previous_process = None;
                    break;
                }
            }
        }

        if let Some(mut last) = previous_process {
            while !term.load(Ordering::Relaxed) {
                match last.try_wait() {
                    Ok(Some(_)) => break,
                    Ok(None) => {
                        thread::sleep(std::time::Duration::from_millis(100));
                    }
                    Err(e) => {
                        eprintln!("Error waiting for process: {}", e);
                        break;
                    }
                }
            }
            if term.load(Ordering::Relaxed) {
                let _ = last.kill();
            }
        }
    }

    let _ = rl.save_history(&history_path);
}
