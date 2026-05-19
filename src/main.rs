mod helper;
mod config;
mod executor;
mod utilities;
mod expansions;
mod themes;
mod plugins;

use dirs::home_dir;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::collections::HashMap;
use rustyline::config::Configurer;
use rustyline::error::ReadlineError;
use rustyline::{Editor, Result as RustyResult};
use signal_hook::consts::SIGINT;
use executor::JobManager;
use themes::{ThemeManager, ProfileManager};

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
        println!("    -t, --theme <name>  Set theme");
        println!("    -p, --profile <name>  Load profile");
    } else {
        println!("  help  Print help message");
        println!("  version  Print version");
        println!("  exit [int]  Exit with code, default: 0");
        println!("  jobs  List running jobs");
        println!("  fg [job_id]  Bring job to foreground");
        println!("  bg [job_id]  Send job to background");
        println!("  cd [dir]  Change directory");
        println!("  &  Execute command in background");
        println!("  theme [name]  Show or set theme");
        println!("  profile [name]  Show or switch profile");
        println!("  plugins  List loaded plugins");
    }
}

fn handle_builtin(
    first_word: &str,
    input: &str,
    job_manager: &JobManager,
    theme_manager: &mut ThemeManager,
    profile_manager: &mut ProfileManager,
    plugin_manager: &plugins::PluginManager,
) -> Option<BuiltinResult> {
    match first_word {
        "exit" => {
            let code = input.split_whitespace().nth(1).and_then(|s| s.parse::<i32>().ok()).unwrap_or(0);
            std::process::exit(code);
        },
        "version" => {
            print_version(false);
            return Some(BuiltinResult::Continue);
        },
        "help" => {
            print_help(false);
            return Some(BuiltinResult::Continue);
        },
        "jobs" => {
            let jobs = job_manager.get_jobs();
            if jobs.is_empty() {
                println!("no jobs");
            } else {
                for job in jobs {
                    match job.status {
                        executor::JobStatus::Running => {
                            let mark = if job.is_background { '+' } else { '-' };
                            println!("[{}]{} Running: {}", job.id, mark, job.command);
                        },
                        executor::JobStatus::Stopped => {
                            let mark = if job.is_background { '+' } else { '-' };
                            println!("[{}]{} Stopped: {}", job.id, mark, job.command);
                        },
                        executor::JobStatus::Completed(_) => {
                            println!("[{}] Done: {}", job.id, job.command);
                        },
                    }
                }
            }
            return Some(BuiltinResult::Continue);
        },
        "fg" => {
            if let Some(job_id) = input.split_whitespace().nth(1).and_then(|s| s.parse::<usize>().ok()) {
                if let Some(mut child) = job_manager.get_child(job_id) {
                    job_manager.update_job_status(job_id, executor::JobStatus::Running);
                    match child.wait() {
                        Ok(status) => {
                            job_manager.mark_completed(job_id, status);
                        },
                        Err(e) => {
                            eprintln!("anssh: fg: wait failed: {}", e);
                        }
                    }
                } else {
                    eprintln!("anssh: fg: job {} has finished", job_id);
                }
            } else {
                eprintln!("anssh: fg: job ID required");
            }
            return Some(BuiltinResult::Continue);
        },
        "bg" => {
            if let Some(job_id) = input.split_whitespace().nth(1).and_then(|s| s.parse::<usize>().ok()) {
                if job_manager.send_to_background(job_id) {
                    println!("[{}] {}", job_id, job_manager.get_job(job_id).map(|j| j.command).unwrap_or_default());
                } else {
                    eprintln!("anssh: bg: job {} is not stopped", job_id);
                }
            } else {
                eprintln!("anssh: bg: job ID required");
            }
            return Some(BuiltinResult::Continue);
        },
        "theme" => {
            if let Some(theme_name) = input.split_whitespace().nth(1) {
                if theme_manager.set_theme(theme_name) {
                    println!("Theme set to: {}", theme_name);
                } else {
                    eprintln!("anssh: theme '{}' not found", theme_name);
                    println!("Available themes: {}", theme_manager.list_themes().join(", "));
                }
            } else {
                println!("Current theme: {}", theme_manager.current().name);
            }
            return Some(BuiltinResult::Continue);
        },
        "profile" => {
            if let Some(profile_name) = input.split_whitespace().nth(1) {
                if profile_manager.set_active(profile_name) {
                    println!("Profile set to: {}", profile_name);
                } else {
                    eprintln!("anssh: profile '{}' not found", profile_name);
                    println!("Available profiles: {}", profile_manager.list_profiles().join(", "));
                }
            } else {
                println!("Current profile: {}", profile_manager.active().name);
            }
            return Some(BuiltinResult::Continue);
        },
        "plugins" => {
            let plugins_list = plugin_manager.list_plugins();
            if plugins_list.is_empty() {
                println!("No plugins loaded");
            } else {
                println!("Loaded plugins:");
                for plugin in plugins_list {
                    println!("  {} (v{}) - {}", plugin.name, plugin.version, plugin.description);
                }
            }
            return Some(BuiltinResult::Continue);
        },
        _ => None,
    }
}

enum BuiltinResult {
    Continue,
    Break,
}

fn expand_input(input: &str, env_vars: &HashMap<String, String>, aliases: &HashMap<String, String>) -> String {
    let mut result = input.to_string();
    
    for (k, v) in env_vars {
        result = result.replace(&format!("${}", k), v);
    }
    
    if let Some(first_word) = result.split_whitespace().next() {
        if let Some(alias_value) = aliases.get(first_word) {
            let rest = result[first_word.len()..].trim().to_string();
            result = if rest.is_empty() {
                alias_value.clone()
            } else {
                format!("{} {}", alias_value, rest)
            };
        }
    }
    
    result
}

fn handle_cd(target: &str, home: &Path, cwd: &str) -> Option<String> {
    let target_path = utilities::expand_path(target, cwd, home);
    let path = Path::new(&target_path);
    if let Err(e) = std::env::set_current_dir(path) {
        eprintln!("anssh: cd: {}: {}", target, e);
        None
    } else {
        Some(std::env::current_dir().unwrap().display().to_string())
    }
}

fn main() {
    let mut norc = false;
    let mut initial_theme = None;
    let mut initial_profile = None;
    
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let mut cwd = std::env::current_dir().unwrap_or_default().display().to_string();
    let job_manager = Arc::new(JobManager::new());
    
    let mut theme_manager = ThemeManager::new(&home);
    let mut profile_manager = ProfileManager::new(&home);
    let plugin_manager = plugins::PluginManager::new(&home);

    let rc_path = home.join(".anssh_rc");
    let history_path = home.join(".anssh_history");
    let prompt_path = home.join(".config/anssh/prompt");
    let complete_path = home.join(".config/anssh/complete");
    let highlight_path = home.join(".config/anssh/highlight_rules");

    utilities::ensure_config_dir(&home);

    if !prompt_path.exists() {
        if let Some(parent) = prompt_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let default_prompt = "{user}@{host}:{cwd}$ ";
        let _ = fs::write(&prompt_path, default_prompt);
    }
    
    if !complete_path.exists() {
        if let Some(parent) = complete_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = fs::write(&complete_path, "");
    }
    
    if !highlight_path.exists() {
        if let Some(parent) = highlight_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let default_highlight = "*.rs:$green\n*.md:$yellow\n";
        let _ = fs::write(&highlight_path, default_highlight);
    }

    let mut env_vars: HashMap<String, String> = std::env::vars().collect();
    let mut aliases: HashMap<String, String> = HashMap::new();

    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 3 && (args[1] == "-c" || args[1] == "--command") {
        let mut command_line = args[2..].join(" ");
        let is_bg = command_line.trim().ends_with('&');
        if is_bg {
            command_line = command_line.trim_end_matches('&').trim().to_string();
        }
        let expanded = expand_input(&command_line, &env_vars, &aliases);
        
        let first_word = expanded.split_whitespace().next().unwrap_or("");
        
        if let Some(result) = handle_builtin(&first_word, &expanded, &job_manager, &mut theme_manager, &mut profile_manager, &plugin_manager) {
            match result {
                BuiltinResult::Continue => return,
                BuiltinResult::Break => return,
            }
        }
        
        let jm = job_manager.clone();
        executor::execute_command_line(&expanded, &home, &aliases, &env_vars, Some(&jm));
        return;
    } else if args.len() >= 3 && (args[1] == "-t" || args[1] == "--theme") {
        initial_theme = Some(args[2].clone());
    } else if args.len() >= 3 && (args[1] == "-p" || args[1] == "--profile") {
        initial_profile = Some(args[2].clone());
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

    if let Some(theme_name) = initial_theme {
        theme_manager.set_theme(&theme_name);
    }
    
    if let Some(profile_name) = initial_profile {
        profile_manager.set_active(&profile_name);
    }
    
    let mut rl = Editor::new().expect("Failed to create Editor");
    let helper = helper::AnsshHelper::new(&complete_path, &highlight_path);
    rl.set_helper(Some(helper));
    let _ = rl.load_history(&history_path);
    rl.set_auto_add_history(true);
    rl.set_completion_type(rustyline::CompletionType::Circular);

    let term = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(SIGINT, Arc::clone(&term)).expect("Failed to register SIGINT");

    loop {
        let template = fs::read_to_string(&prompt_path).unwrap_or_else(|_| {
            "anssh> ".to_string()
        });
        
        let current_cwd = std::env::current_dir().unwrap_or_default().display().to_string();
        let user = std::env::var("USER").unwrap_or_else(|_| "user".to_string());
        let host = hostname::get()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        
        let mut prompt = template.replace("{user}", &user);
        prompt = prompt.replace("{cwd}", &current_cwd);
        prompt = prompt.replace("{host}", &host);
        
        prompt = prompt.replace("$blue", "\x1b[34m");
        prompt = prompt.replace("$green", "\x1b[32m");
        prompt = prompt.replace("$red", "\x1b[31m");
        prompt = prompt.replace("$yellow", "\x1b[33m");
        prompt = prompt.replace("$cyan", "\x1b[36m");
        prompt = prompt.replace("$pink", "\x1b[38;5;205m");
        prompt = prompt.replace("$orange", "\x1b[38;5;214m");
        prompt = prompt.replace("$purple", "\x1b[35m");
        prompt = prompt.replace("$clear", "\x1b[0m");
        prompt = prompt.replace("$white", "\x1b[37m");
        prompt = prompt.replace("$gray", "\x1b[90m");
        
        let prompt_str = prompt.to_string();

        term.store(false, Ordering::Relaxed);

        let readline = rl.readline(&prompt_str);
        
        let input = match readline {
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

        let first_word = input.split_whitespace().next().unwrap_or("");

        if let Some(result) = handle_builtin(&first_word, &input, &job_manager, &mut theme_manager, &mut profile_manager, &plugin_manager) {
            match result {
                BuiltinResult::Continue => continue,
                BuiltinResult::Break => break,
            }
        }

        if first_word == "cd" {
            let target = input.split_whitespace().nth(1).unwrap_or("~");
            if let Some(new_cwd) = handle_cd(target, &home, &cwd) {
                cwd = new_cwd;
            }
            continue;
        }

        let expanded = expand_input(&input, &env_vars, &aliases);

        if expanded.ends_with(".anssh") && Path::new(&expanded).is_file() {
            let mut parts = expanded.split_whitespace();
            let script_path = parts.next().unwrap();
            let args: Vec<String> = parts.map(|s| s.to_string()).collect();
            executor::run_script(Path::new(script_path), &home, &aliases, &env_vars, &args);
            continue;
        }

        if Path::new(&expanded).is_dir() {
            let _ = handle_cd(&expanded, &home, &cwd);
            continue;
        }

        let is_background = expanded.trim().ends_with('&');
        let final_input = if is_background {
            expanded.trim_end_matches('&').trim().to_string()
        } else {
            expanded.clone()
        };

        rl.add_history_entry(final_input.clone()).expect("Failed to add history entry");

        if is_background {
            let job_id = job_manager.add_job(final_input.clone(), None, true);
            println!("[{}] {}", job_id, final_input);
            
            let home_clone = home.clone();
            let aliases_clone = aliases.clone();
            let env_vars_clone = env_vars.clone();
            let input_clone = final_input.clone();
            let jm = job_manager.clone();
            
            std::thread::spawn(move || {
                executor::execute_command_line(&input_clone, &home_clone, &aliases_clone, &env_vars_clone, Some(&jm));
            });
        } else {
            executor::execute_command_line(&final_input, &home, &aliases, &env_vars, Some(&job_manager));
        }
        use std::io::Write;
        let _ = std::io::stdout().flush();
    }

    let _ = rl.save_history(&history_path);
}
