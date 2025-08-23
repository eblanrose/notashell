use std::collections::HashMap;
use std::{fs, thread};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use glob::glob;

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

pub(crate) fn run_script(
    path: &Path,
    home: &Path,
    aliases: &HashMap<String, String>,
    env_vars: &HashMap<String, String>,
    args: &[String],
) {
    if let Ok(contents) = fs::read_to_string(path) {
        for line in contents.lines() {
            let mut line = line.trim().to_string();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            for (k, v) in env_vars {
                line = line.replace(&format!("${}", k), v);
            }
            for (i, arg) in args.iter().enumerate() {
                line = line.replace(&format!("${}", i + 1), arg);
            }
            if let Some(first_word) = line.split_whitespace().next() {
                if let Some(alias_value) = aliases.get(first_word) {
                    let rest: String = line[first_word.len()..].trim().to_string();
                    line = if rest.is_empty() {
                        alias_value.clone()
                    } else {
                        format!("{} {}", alias_value, rest)
                    };
                }
            }
            execute_command_line(&line, home, aliases, env_vars);
        }
    } else {
        eprintln!("anssh: failed to read script file {:?}", path);
    }
}


// ------------------ execution command line ------------------
pub(crate) fn execute_command_line(line: &str, home: &Path, aliases: &HashMap<String,String>, env_vars: &HashMap<String,String>) {
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


