mod helper;
mod config;
mod executor;
mod utilities;

use dirs::home_dir;
use rustyline::config::Configurer;
use rustyline::error::ReadlineError;
use rustyline::{
    Editor, Result as RustyResult,
};
use signal_hook::consts::SIGINT;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
fn print_version(if_colon: bool) {
    if if_colon {
        println!("anssh v{}:", env!("CARGO_PKG_VERSION"));
    } else {
        println!("anssh v{}", env!("CARGO_PKG_VERSION"));
    }
}

fn print_help(if_out: bool) {
    print_version(true);
    if if_out {
        println!("  Usage:");
        println!("    anssh [Options]");
        println!("  Options:");
        println!("    -h, --help  Print help message");
        println!("    -v, --version  Print version");
        println!("    -nc, --norc  Not loading rc");
    } else {
        println!("  help  Print help message");
        println!("  version  Print version");
        println!("  exit [int]  Exit with code, default: 0");
    }
}


// Main
fn main() {
    // flags
    let mut norc = false;

    // variables
    let home = home_dir().unwrap_or_else(|| PathBuf::from("."));
    let mut cwd = std::env::current_dir().unwrap_or_default().display().to_string();

    utilities::ensure_config_dir(&home);
    let rc_path = home.join(".anssh_rc");
    let history_path = home.join(".anssh_history");
    let prompt_path = home.join(".config/anssh/prompt");
    let complete_path = home.join(".config/anssh/complete");
    let highlight_path = home.join(".config/anssh/highlight_rules");

    let helper = helper::AnsshHelper::new(&complete_path, &highlight_path);
    let mut env_vars: HashMap<String, String> = std::env::vars().collect();
    let mut aliases: HashMap<String, String> = HashMap::new();

    // checking for arguments like -c
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 3 && (args[1] == "-c" || args[1] == "--command") {
        let command_line = args[2..].join(" ");
        executor::execute_command_line(&command_line, &home, &aliases, &env_vars);
        return;
    } else if args.len() >= 2 && (args[1] == "-nc" || args[1] == "--norc") {
        norc = true;
    } else if args.len() >= 2 && (args[1] == "-v" || args[1] == "--version") {
        print_version(false);
        return;
    } else if args.len() >= 2 && (args[1] == "-h" || args[1] == "--help") {
        print_help(true);
        return;
    } else if args.len() >= 2 {
        print_help(true);
        return;
    }

    if !norc {
        config::read_rc(rc_path, &mut env_vars, &mut aliases);
    }

    let mut rl = Editor::new().expect("Failed to create Editor");
    rl.set_helper(Some(helper));
    let _ = rl.load_history(&history_path);
    rl.set_auto_add_history(true);
    rl.set_completion_type(rustyline::CompletionType::Circular);

    let term = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(SIGINT, Arc::clone(&term)).expect("Failed to register SIGINT");

    // cycle
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

        let mut parts = input.split_whitespace();
        let first_word = parts.next().unwrap_or("");

        match first_word {
            "exit" => {
                let code = parts.next().and_then(|s| s.parse::<i32>().ok()).unwrap_or(0);
                std::process::exit(code);
            },
            "version" => {
                print_version(false);
                continue;
            },
            "help" => {
                print_help(false);
                continue;
            },
            _ => {}
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

        let mut parts = input.split_whitespace();
        let first_word = parts.next().unwrap_or("");
        let path = Path::new(first_word);
        if first_word == "cd" {
            let target = parts.next().unwrap_or(".");
            let target_path = utilities::expand_path(target, &cwd, &home);
            let path = Path::new(&target_path);
            if let Err(e) = std::env::set_current_dir(path) {
                eprintln!("anssh: cd: {}: {}", target, e);
            } else {
                cwd = std::env::current_dir().unwrap().to_str().unwrap().to_string();
            }
            continue;
        }

        if path.is_dir() {
            if let Err(e) = std::env::set_current_dir(path) {
                eprintln!("Failed to enter directory {}: {}", first_word, e);
            }
            continue;
        }

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
