use glob::glob;
use std::collections::HashMap;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::{fs, io::Write};
use crate::utilities;
use crate::expansions;
use std::sync::{Arc, Mutex};
use std::io::{Seek, SeekFrom};

#[derive(Debug, Clone)]
pub struct Job {
    pub id: usize,
    pub command: String,
    pub status: JobStatus,
    pub is_background: bool,
}

impl Job {
    pub fn new(id: usize, command: String, is_background: bool) -> Self {
        Self {
            id,
            command,
            status: JobStatus::Running,
            is_background,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum JobStatus {
    Running,
    Stopped,
    Completed(std::process::ExitStatus),
}

#[derive(Clone)]
pub struct JobManager {
    jobs: Arc<Mutex<Vec<Job>>>,
    next_id: Arc<Mutex<usize>>,
}

impl JobManager {
    pub fn new() -> Self {
        Self {
            jobs: Arc::new(Mutex::new(Vec::new())),
            next_id: Arc::new(Mutex::new(1)),
        }
    }

    pub fn add_job(&self, command: String, _child: Option<Child>, is_background: bool) -> usize {
        let mut id_guard = self.next_id.lock().unwrap();
        *id_guard += 1;
        let job_id = *id_guard;
        drop(id_guard);

        let job = Job::new(job_id, command, is_background);

        let mut jobs = self.jobs.lock().unwrap();
        jobs.retain(|j| j.status == JobStatus::Running);
        jobs.push(job);
        job_id
    }

    pub fn get_jobs(&self) -> Vec<Job> {
        let jobs = self.jobs.lock().unwrap();
        jobs.clone()
    }

    pub fn get_job(&self, id: usize) -> Option<Job> {
        let jobs = self.jobs.lock().unwrap();
        jobs.iter().find(|j| j.id == id).cloned()
    }

    pub fn get_background_jobs(&self) -> Vec<Job> {
        let jobs = self.jobs.lock().unwrap();
        jobs.iter().filter(|j| j.is_background && j.status == JobStatus::Running).cloned().collect()
    }

    pub fn update_job_status(&self, id: usize, status: JobStatus) {
        let mut jobs = self.jobs.lock().unwrap();
        if let Some(job) = jobs.iter_mut().find(|j| j.id == id) {
            job.status = status;
        }
    }

    pub fn mark_completed(&self, id: usize, status: std::process::ExitStatus) {
        let mut jobs = self.jobs.lock().unwrap();
        if let Some(job) = jobs.iter_mut().find(|j| j.id == id) {
            job.status = JobStatus::Completed(status);
        }
    }

    pub fn remove_job(&self, id: usize) {
        let mut jobs = self.jobs.lock().unwrap();
        jobs.retain(|j| j.id != id);
    }

    pub fn bring_to_foreground(&self, id: usize) -> Option<Child> {
        let mut jobs = self.jobs.lock().unwrap();
        if let Some(job) = jobs.iter_mut().find(|j| j.id == id) {
            job.is_background = false;
            job.status = JobStatus::Running;
        }
        drop(jobs);

        let mut child_map = CHILDREN.lock().unwrap();
        child_map.remove(&id)
    }

    pub fn send_to_background(&self, id: usize) -> bool {
        let mut jobs = self.jobs.lock().unwrap();
        if let Some(job) = jobs.iter_mut().find(|j| j.id == id) {
            if job.status == JobStatus::Stopped {
                job.is_background = true;
                job.status = JobStatus::Running;
                return true;
            }
        }
        false
    }

    pub fn get_child(&self, id: usize) -> Option<Child> {
        let mut child_map = CHILDREN.lock().unwrap();
        child_map.remove(&id)
    }
}

lazy_static::lazy_static! {
    pub static ref CHILDREN: Mutex<std::collections::HashMap<usize, Child>> = Mutex::new(std::collections::HashMap::new());
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

pub fn run_script(
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
            execute_command_line(&line, home, aliases, env_vars, None);
        }
    } else {
        eprintln!("anssh: failed to read script file {:?}", path);
    }
}

pub fn execute_command_line(
    line: &str, 
    home: &Path, 
    aliases: &HashMap<String,String>, 
    env_vars: &HashMap<String,String>,
    job_manager: Option<&JobManager>,
) {
    let commands: Vec<String> = line.split('|').map(|s| s.trim().to_string()).collect();
    let mut previous_process: Option<Child> = None;
    let cwd = std::env::current_dir().unwrap_or_default().display().to_string();
    let mut current_job_id: Option<usize> = None;

    for (i, cmd) in commands.iter().enumerate() {
        let cmd_parts: Vec<String> = shell_words::split(cmd).unwrap_or_default();
        if cmd_parts.is_empty() {
            continue;
        }
        
        let mut args: Vec<String> = cmd_parts[1..].to_vec();

        for arg in &mut args {
            *arg = crate::expansions::expand_subshells(arg, aliases, env_vars);
            *arg = utilities::expand_path(&*arg, &*cwd, home);
        }

        let mut expanded_args = Vec::new();
        for arg in &args {
            if arg.contains('{') {
                let brace_expanded = expansions::expand_braces(arg);
                for expanded in brace_expanded {
                    if expanded.contains('*') {
                        for entry in glob(&expanded).unwrap().flatten() {
                            expanded_args.push(entry.to_string_lossy().to_string());
                        }
                    } else {
                        expanded_args.push(expanded);
                    }
                }
            } else if arg.contains('*') {
                for entry in glob(arg).unwrap().flatten() {
                    expanded_args.push(entry.to_string_lossy().to_string());
                }
            } else {
                expanded_args.push(arg.clone());
            }
        }
        args = expanded_args;
        
        // Pass command name + args to redirection parser
        let mut all_tokens = vec![cmd_parts[0].clone()];
        all_tokens.extend(args);
        let (command_tokens, redirection) = expansions::Redirection::parse(&all_tokens);
        if command_tokens.is_empty() {
            continue;
        }
        
        let mut command = Command::new(&command_tokens[0]);
        command.args(&command_tokens[1..]);
        command.envs(env_vars);

        if let Some(input_file) = &redirection.input {
            if let Ok(file) = fs::File::open(input_file) {
                command.stdin(Stdio::from(file));
            }
        } else if let Some((delimiter, content)) = &redirection.here_doc {
            let temp_file = tempfile::tempfile().unwrap();
            let mut buffer = std::io::BufWriter::new(temp_file);
            if let Err(e) = writeln!(buffer, "{}", content) {
                eprintln!("Here document error: {}", e);
            }
            buffer.flush().unwrap();
            let mut temp_file = buffer.into_inner().unwrap();
            temp_file.seek(SeekFrom::Start(0)).unwrap();
            command.stdin(Stdio::from(temp_file));
        } else {
            let mut stdin_pipe = None;
            if let Some(prev) = previous_process.as_ref() {
                if let Some(stdout) = prev.stdout.as_ref() {
                    use std::os::unix::io::{FromRawFd, AsRawFd};
                    let fd = stdout.as_raw_fd();
                    stdin_pipe = Some(unsafe { Stdio::from_raw_fd(fd) });
                }
            }
            if let Some(pipe) = stdin_pipe {
                command.stdin(pipe);
            } else if i > 0 {
                command.stdin(Stdio::inherit());
            }
        }

        if let Some(output_file) = &redirection.output {
            if let Ok(file) = fs::File::create(output_file) {
                command.stdout(Stdio::from(file));
            } else {
                eprintln!("anssh: cannot open output file: {}", output_file);
            }
        } else if let Some(append_file) = &redirection.append {
            if let Ok(file) = fs::OpenOptions::new().write(true).create(true).append(true).open(append_file) {
                command.stdout(Stdio::from(file));
            } else {
                eprintln!("anssh: cannot open append file: {}", append_file);
            }
        } else if i == commands.len() - 1 && current_job_id.is_none() {
            command.stdout(Stdio::inherit());
        } else {
            command.stdout(Stdio::piped());
        }

        if let Some(error_file) = &redirection.error {
            if let Ok(file) = fs::File::create(error_file) {
                command.stderr(Stdio::from(file));
            } else {
                eprintln!("anssh: cannot open error file: {}", error_file);
            }
        }

let child = command.spawn();
        match child {
            Ok(c) => {
                let is_bg = i == commands.len() - 1 && previous_process.is_none() && current_job_id.is_none();
                if let Some(jm) = job_manager {
                    let id = jm.add_job(line.to_string(), None, is_bg);
                    current_job_id = Some(id);
                }
                match is_bg {
                    true => {},
                    false => { previous_process = Some(c); }
                }
            },
            Err(e) => {
                eprintln!("anssh: {}", e);
                previous_process = None;
                break;
            }
        }
    }

    if let Some(mut last) = previous_process {
        if current_job_id.is_none() {
            if let Ok(status) = last.wait() {
                if let Some(jid) = current_job_id {
                    let mut children = CHILDREN.lock().unwrap();
                    children.remove(&jid);
                }
                if job_manager.is_some() {
                    if let Some(jid) = current_job_id {
                        job_manager.unwrap().update_job_status(jid, JobStatus::Completed(status));
                    }
                }
            }
        }
    }
}

pub fn wait_for_job(job_id: usize) -> Option<std::process::ExitStatus> {
    let mut children = CHILDREN.lock().unwrap();
    if let Some(mut child) = children.remove(&job_id) {
        loop {
            match child.wait() {
                Ok(status) => return Some(status),
                Err(_) => return None,
            }
        }
    }
    None
}