mod helper;
mod config;

use dirs::home_dir;
use glob::glob;
use rustyline::error::ReadlineError;
use rustyline::{
    Editor, Result as RustyResult,
};
use signal_hook::consts::SIGINT;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::{fs, thread};

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

    // ------------------ checking for arguments like -c ------------------
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 3 && args[1] == "-c" {
        let command_line = args[2..].join(" ");
        execute_command_line(&command_line, &home, &aliases, &env_vars);
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

        execute_command_line(&input, &home, &aliases, &env_vars);
    }

    let _ = rl.save_history(&history_path);
}
fn expand_subshells(arg: &str, aliases: &HashMap<String,String>, env_vars: &HashMap<String,String>) -> String {
    let mut result = String::new();
    let mut chars = arg.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '$' && chars.peek() == Some(&'(') {
            chars.next();
            let mut depth = 1;
            let mut cmd = String::new();

            while let Some(&ch) = chars.peek() {
                chars.next();
                if ch == '(' { depth += 1; }
                if ch == ')' { depth -= 1; }
                if depth == 0 { break; }
                cmd.push(ch);
            }

            let cmd = expand_subshells(cmd.trim(), aliases, env_vars);

            let output = Command::new(std::env::current_exe().unwrap().as_os_str())
                .arg("-c")
                .arg(&cmd)
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_else(|_| "".to_string());

            result.push_str(&output);
        } else {
            result.push(c);
        }
    }

    result
}

// ------------------ execution command line ------------------
fn execute_command_line(line: &str, home: &Path, aliases: &HashMap<String,String>, env_vars: &HashMap<String,String>) {
    let commands: Vec<String> = line.split('|').map(|s| s.trim().to_string()).collect();
    let mut previous_process: Option<Child> = None;

    for (i, cmd) in commands.iter().enumerate() {
        let cmd_parts: Vec<String> = shell_words::split(cmd).unwrap_or_default();
        if cmd_parts.is_empty() {
            continue;
        }
        let cmd_name = &cmd_parts[0];
        let mut args: Vec<String> = cmd_parts[1..].to_vec();

        for arg in &mut args {
            *arg = expand_subshells(arg, aliases, env_vars);
            if arg.starts_with("~") {
                *arg = arg.replacen("~", home.to_str().unwrap(), 1);
            }
        }

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
            Ok(c) => previous_process = Some(c),
            Err(e) => {
                eprintln!("anssh: {}", e);
                previous_process = None;
                break;
            }
        }
    }

    if let Some(mut last) = previous_process {
        while let Ok(None) = last.try_wait() {
            thread::sleep(std::time::Duration::from_millis(100));
        }
    }
}
