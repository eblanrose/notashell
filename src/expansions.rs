use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::process::Command;

pub fn expand_subshells(arg: &str, aliases: &HashMap<String,String>, env_vars: &HashMap<String,String>) -> String {
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

pub struct Redirection {
    pub input: Option<String>,
    pub output: Option<String>,
    pub append: Option<String>,
    pub error: Option<String>,
    pub here_doc: Option<(String, String)>,
}

impl Redirection {
    pub fn parse(tokens: &[String]) -> (Vec<String>, Self) {
        let mut command_tokens = Vec::new();
        let mut redirection = Self {
            input: None,
            output: None,
            append: None,
            error: None,
            here_doc: None,
        };
        
        let mut i = 0;
        while i < tokens.len() {
            let token = &tokens[i];
            
            if token.starts_with('<') && token.len() > 1 && &token[1..] == "<<" {
                if i + 1 < tokens.len() {
                    let delimiter = tokens[i + 1].to_string();
                    let mut content = String::new();
                    let mut line = String::new();
                    
                    loop {
                        if let Ok(n) = io::stdin().read_line(&mut line) {
                            if n == 0 {
                                break;
                            }
                            if line.trim() == delimiter {
                                break;
                            }
                            content.push_str(&line);
                            line.clear();
                        } else {
                            break;
                        }
                    }
                    
                    redirection.here_doc = Some((delimiter, content));
                    i += 2;
                    continue;
                }
            } else if token.starts_with('>') && token.len() > 1 && &token[1..] == ">" {
                if token.len() > 2 {
                    redirection.append = Some(token[2..].to_string());
                } else if i + 1 < tokens.len() {
                    redirection.append = Some(tokens[i + 1].to_string());
                    i += 1;
                }
            } else if token.starts_with('>') && token.len() > 1 {
                redirection.output = Some(token[1..].to_string());
            } else if token == ">" {
                if i + 1 < tokens.len() {
                    redirection.output = Some(tokens[i + 1].to_string());
                    i += 1;
                }
            } else if token == ">>" {
                if i + 1 < tokens.len() {
                    redirection.append = Some(tokens[i + 1].to_string());
                    i += 1;
                }
            } else if token == "2>" {
                if i + 1 < tokens.len() {
                    redirection.error = Some(tokens[i + 1].to_string());
                    i += 1;
                }
            } else if token == "<" {
                if i + 1 < tokens.len() {
                    redirection.input = Some(tokens[i + 1].to_string());
                    i += 1;
                }
            } else if token == "<<" {
                if i + 1 < tokens.len() {
                    let delimiter = tokens[i + 1].to_string();
                    let mut content = String::new();
                    let mut line = String::new();
                    
                    loop {
                        if let Ok(n) = io::stdin().read_line(&mut line) {
                            if n == 0 {
                                break;
                            }
                            if line.trim() == delimiter {
                                break;
                            }
                            content.push_str(&line);
                            line.clear();
                        } else {
                            break;
                        }
                    }
                    
                    redirection.here_doc = Some((delimiter, content));
                    i += 2;
                    continue;
                }
            } else {
                command_tokens.push(token.clone());
            }
            
            i += 1;
        }
        
        (command_tokens, redirection)
    }
}

pub fn expand_braces(pattern: &str) -> Vec<String> {
    let mut results = vec![pattern.to_string()];
    let mut depth = 0;
    let mut start = None;
    
    for (i, c) in pattern.char_indices() {
        match c {
            '{' if depth == 0 => {
                start = Some(i);
            },
            '{' => {
                depth += 1;
            },
            '}' if depth > 0 => {
                depth -= 1;
            },
            '}' if depth == 0 && start.is_some() => {
                let content = &pattern[start.unwrap() + 1..i];
                let prefix = &pattern[..start.unwrap()];
                let suffix = &pattern[i + 1..];
                
                let mut new_results = Vec::new();
                let results_copy = results.clone();
                for result in results_copy {
                    if content.contains(',') {
                        for part in content.split(',') {
                            new_results.push(format!("{}{}{}", prefix, part, suffix));
                        }
                    } else {
                        new_results.push(result.replace(&pattern[start.unwrap()..=i], content));
                    }
                }
                
                if new_results.is_empty() {
                    results.push(pattern.to_string());
                } else {
                    results = new_results;
                }
                
                start = None;
            },
            _ => {}
        }
    }
    
    results
}

pub fn expand_tilde(pattern: &str, home: &Path) -> String {
    let mut result = String::new();
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0;
    
    while i < chars.len() {
        if chars[i] == '~' && (i == 0 || chars.get(i - 1).map_or(true, |c| c.is_whitespace()) || chars.get(i - 1) == Some(&'/') || chars.get(i - 1) == Some(&':')) {
            if i + 1 < chars.len() {
                match chars[i + 1] {
                    '+' => {
                        let cwd = std::env::current_dir().unwrap_or_default();
                        result.push_str(&cwd.to_string_lossy());
                        i += 2;
                    },
                    '-' => {
                        if let Ok(prev_dir) = std::env::var("OLDPWD") {
                            result.push_str(&prev_dir);
                        } else {
                            result.push('~');
                            result.push('-');
                            i += 2;
                            continue;
                        }
                        i += 2;
                    },
                    '/' | '\0' => {
                        result.push_str(&home.to_string_lossy());
                        i += 2;
                    },
                    _ if chars[i + 1].is_alphanumeric() || chars[i + 1] == '_' || chars[i + 1] == '-' => {
                        let mut user_name = String::new();
                        let mut j = i + 1;
                        
                        while j < chars.len() && (chars[j].is_alphanumeric() || chars[j] == '_' || chars[j] == '-') {
                            user_name.push(chars[j]);
                            j += 1;
                        }
                        
                        if let Ok(user_home) = std::env::var(format!("HOME_{}", user_name)) {
                            result.push_str(&user_home);
                        } else if let Ok(passwd_content) = std::fs::read_to_string("/etc/passwd") {
                            for line in passwd_content.lines() {
                                let parts: Vec<&str> = line.split(':').collect();
                                if parts.len() >= 6 && parts[0] == user_name {
                                    result.push_str(parts[5]);
                                    break;
                                }
                            }
                            if result.ends_with(&format!("~{}", user_name)) {
                                result.push_str(&home.to_string_lossy());
                            }
                        } else {
                            result.push_str(&home.to_string_lossy());
                        }
                        
                        i = j;
                    },
                    _ => {
                        result.push('~');
                        i += 1;
                    }
                }
            } else {
                result.push_str(&home.to_string_lossy());
                i += 1;
            }
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }
    
    result
}