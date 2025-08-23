mod helper;
mod config;
mod executor;

use dirs::home_dir;
use rustyline::error::ReadlineError;
use rustyline::{
    Editor, Result as RustyResult,
};
use signal_hook::consts::SIGINT;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::fs;

// ------------------- Main -------------------
fn main() {
    let home = home_dir().unwrap_or_else(|| PathBuf::from("."));
    config::ensure_config_dir(&home);
    let rc_path = home.join(".anssh_rc");
    let history_path = home.join(".anssh_history");
    let prompt_path = home.join(".config/anssh/prompt");
    let complete_path = home.join(".config/anssh/complete");
    let highlight_path = home.join(".config/anssh/highlight_rules");

    let helper = helper::AnsshHelper::new(&complete_path, &highlight_path);
    let mut env_vars: HashMap<String, String> = std::env::vars().collect();
    let mut aliases: HashMap<String, String> = HashMap::new();

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

    let mut rl = Editor::new().expect("Failed to create Editor");
    rl.set_helper(Some(helper));
    let _ = rl.load_history(&history_path);

    // ------------------ checking for arguments like -c ------------------
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 3 && args[1] == "-c" {
        let command_line = args[2..].join(" ");
        executor::execute_command_line(&command_line, &home, &aliases, &env_vars);
        return;
    }

    // ------------------ cycle ------------------
    let term = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(SIGINT, Arc::clone(&term)).expect("Failed to register SIGINT");

    loop {
        owo_colors::set_override(true);
        let template = fs::read_to_string(&prompt_path).unwrap_or_else(|e| {
            eprintln!("Warning: Failed to read prompt file: {}", e);
            "anssh> ".to_string()
        });
        let prompt_str = config::parse_prompt_theme(&template, "");

        term.store(false, Ordering::Relaxed);

        let readline = rl.readline(&prompt_str);
        let mut input = match readline {
            Ok(line) => line.trim().to_string(),
            Err(ReadlineError::Interrupted) => continue,
            Err(ReadlineError::Eof) => break,
            Err(e) => {
                eprintln!("Error reading line: {}", e);
                break;
            }
        };

        if input.is_empty() || input.starts_with('#') {
            continue;
        }

        rl.add_history_entry(input.clone()).expect("Failed to add history entry");


        if input.starts_with("exit") {
            let parts: Vec<&str> = input.split_whitespace().collect();
            let code = if parts.len() > 1 {
                parts[1].parse::<i32>().unwrap_or(0)
            } else {
                0
            };
            std::process::exit(code);
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

        let first_word = input.split_whitespace().next().unwrap_or("");
        if first_word.ends_with(".anssh") && Path::new(first_word).is_file() {
            let mut parts = input.split_whitespace();
            let script_path = parts.next().unwrap();
            let args: Vec<String> = parts.map(|s| s.to_string()).collect();
            executor::run_script(Path::new(script_path), &home, &aliases, &env_vars, &args);
            continue;
        }

        executor::execute_command_line(&input, &home, &aliases, &env_vars);
    }

    let _ = rl.save_history(&history_path);
}
