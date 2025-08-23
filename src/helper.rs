use crate::{config, RustyResult};
use owo_colors::OwoColorize;
use rustyline::completion::{Completer, Pair};
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use rustyline::{Context, Helper};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct AnsshHelper {
    rules: Vec<String>,
    ignore: HashSet<String>,
    bin_paths: Vec<PathBuf>,
    highlight_rules: Vec<(glob::Pattern, Arc<dyn Fn(&str) -> String + Send + Sync>)>,
}

impl AnsshHelper {
    pub fn new(complete_file: &PathBuf, highlight_path: &Path) -> Self {
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

        let highlight_rules = config::load_highlight_rules(highlight_path);

        Self {
            rules,
            ignore,
            bin_paths: vec![PathBuf::from("/usr/local/bin"), PathBuf::from("/bin")],
            highlight_rules,
        }
    }

    pub fn list_binaries(&self) -> Vec<String> {
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
            if rule.contains("$directory") || rule.contains("$file") {
                if let Ok(entries) = fs::read_dir(".") {
                    for e in entries.flatten() {
                        let name = match e.file_name().into_string() {
                            Ok(n) => n,
                            Err(_) => continue,
                        };

                        if self.ignore.contains(&name) {
                            continue;
                        }

                        if e.path().is_dir() && rule.contains("$directory") {
                            matches.push(Pair {
                                display: name.clone(),
                                replacement: name.clone(),
                            });
                        }

                        if e.path().is_file() && rule.contains("$file") {
                            matches.push(Pair {
                                display: name.clone(),
                                replacement: name.clone(),
                            });
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
