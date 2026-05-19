use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use std::fmt::Debug;

pub trait ShellPlugin: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn version(&self) -> &str;
    fn execute(&self, args: &[String], context: &PluginContext) -> PluginResult;
    fn on_command(&self, command: &str) -> Option<String> { None }
    fn on_prompt(&self, prompt: &str) -> Option<String> { None }
}

#[derive(Debug, Clone)]
pub struct PluginContext {
    pub cwd: String,
    pub env_vars: HashMap<String, String>,
    pub aliases: HashMap<String, String>,
    pub home: PathBuf,
}

#[derive(Debug, Clone)]
pub struct PluginResult {
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
}

impl PluginResult {
    pub fn ok(output: impl Into<String>) -> Self {
        Self {
            success: true,
            output: output.into(),
            error: None,
        }
    }
    
    pub fn err(error: impl Into<String>) -> Self {
        Self {
            success: false,
            output: String::new(),
            error: Some(error.into()),
        }
    }
}

pub struct PluginManager {
    plugins: HashMap<String, Box<dyn ShellPlugin>>,
    plugins_dir: PathBuf,
    enabled: bool,
}

impl PluginManager {
    pub fn new(home: &std::path::Path) -> Self {
        let plugins_dir = home.join(".config/anssh/plugins");
        std::fs::create_dir_all(&plugins_dir).ok();
        
        let mut manager = Self {
            plugins: HashMap::new(),
            plugins_dir,
            enabled: true,
        };
        
        manager.load_builtin_plugins();
        manager.load_plugins_from_dir();
        
        manager
    }

    fn load_builtin_plugins(&mut self) {
        self.register_plugin(GitPlugin::new());
        self.register_plugin(WeatherPlugin::new());
        self.register_plugin(DateTimePlugin::new());
        self.register_plugin(CalcPlugin::new());
        self.register_plugin(AliasesPlugin::new());
    }

    pub fn register_plugin(&mut self, plugin: impl ShellPlugin + 'static) {
        let name = plugin.name().to_string();
        self.plugins.insert(name, Box::new(plugin));
    }

    fn load_plugins_from_dir(&mut self) {
        if let Ok(entries) = std::fs::read_dir(&self.plugins_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |e| e == "so") || path.extension().map_or(false, |e| e == "dylib") {
                    if let Some(name) = path.file_stem() {
                        let plugin = load_plugin_from_file(&path);
                        if let Some(p) = plugin {
                            self.plugins.insert(name.to_string_lossy().to_string(), Box::new(p));
                        }
                    }
                }
            }
        }
    }

    pub fn execute_plugin(&self, name: &str, args: &[String], context: &PluginContext) -> PluginResult {
        if let Some(plugin) = self.plugins.get(name) {
            plugin.execute(args, context)
        } else {
            PluginResult::err(format!("Plugin '{}' not found", name))
        }
    }

    pub fn list_plugins(&self) -> Vec<PluginInfo> {
        self.plugins
            .iter()
            .map(|(name, plugin)| PluginInfo {
                name: name.clone(),
                description: plugin.description().to_string(),
                version: plugin.version().to_string(),
            })
            .collect()
    }

    pub fn get_plugin(&self, name: &str) -> Option<&dyn ShellPlugin> {
        self.plugins.get(name).map(|p| p.as_ref())
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn plugins_dir(&self) -> &PathBuf {
        &self.plugins_dir
    }
}

fn load_plugin_from_file(_path: &std::path::Path) -> Option<ExternalPlugin> {
    None
}

#[derive(Debug, Clone)]
pub struct PluginInfo {
    pub name: String,
    pub description: String,
    pub version: String,
}

pub struct ExternalPlugin {
    name: String,
    description: String,
    version: String,
    path: PathBuf,
}

impl ExternalPlugin {
    pub fn new(name: &str, path: PathBuf) -> Self {
        Self {
            name: name.to_string(),
            description: format!("External plugin from {:?}", path),
            version: "1.0.0".to_string(),
            path,
        }
    }
}

impl ShellPlugin for ExternalPlugin {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn version(&self) -> &str {
        &self.version
    }

    fn execute(&self, args: &[String], context: &PluginContext) -> PluginResult {
        let output = std::process::Command::new(&self.path)
            .args(args)
            .output();
        
        match output {
            Ok(out) => {
                if out.status.success() {
                    PluginResult::ok(String::from_utf8_lossy(&out.stdout))
                } else {
                    PluginResult::err(String::from_utf8_lossy(&out.stderr))
                }
            }
            Err(e) => PluginResult::err(e.to_string()),
        }
    }
}

pub struct GitPlugin;

impl GitPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl ShellPlugin for GitPlugin {
    fn name(&self) -> &str {
        "git"
    }

    fn description(&self) -> &str {
        "Git integration - shows branch and status in prompt"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn execute(&self, args: &[String], context: &PluginContext) -> PluginResult {
        if args.is_empty() || args[0] == "branch" {
            let cwd = std::path::Path::new(&context.cwd);
            let git_dir = cwd.join(".git");
            
            if git_dir.exists() {
                let head = cwd.join(".git").join("HEAD");
                if let Ok(content) = std::fs::read_to_string(head) {
                    if let Some(branch) = content.strip_prefix("ref: refs/heads/") {
                        return PluginResult::ok(branch.trim().to_string());
                    }
                }
            }
            return PluginResult::ok("".to_string());
        }
        
        PluginResult::err("Unknown git subcommand".to_string())
    }

    fn on_prompt(&self, _prompt: &str) -> Option<String> {
        let cwd = std::env::current_dir().unwrap_or_default();
        let git_dir = cwd.join(".git");
        
        if git_dir.exists() {
            let head = cwd.join(".git").join("HEAD");
            if let Ok(content) = std::fs::read_to_string(head) {
                if let Some(branch) = content.strip_prefix("ref: refs/heads/") {
                    let branch = branch.trim();
                    return Some(format!(" [\x1b[32m{}\x1b[0m]", branch));
                }
            }
        }
        None
    }
}

pub struct WeatherPlugin;

impl WeatherPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl ShellPlugin for WeatherPlugin {
    fn name(&self) -> &str {
        "weather"
    }

    fn description(&self) -> &str {
        "Shows current weather in prompt"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn execute(&self, args: &[String], _context: &PluginContext) -> PluginResult {
        let location = args.first().map(|s| s.as_str()).unwrap_or("auto");
        PluginResult::ok(format!("Weather for {}: Sunny, 72°F", location))
    }
}

pub struct DateTimePlugin;

impl DateTimePlugin {
    pub fn new() -> Self {
        Self
    }
}

impl ShellPlugin for DateTimePlugin {
    fn name(&self) -> &str {
        "datetime"
    }

    fn description(&self) -> &str {
        "Shows current date/time"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn execute(&self, args: &[String], _context: &PluginContext) -> PluginResult {
        let now = std::time::SystemTime::now();
        let datetime = now
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| {
                let secs = d.as_secs();
                let hours = (secs / 3600) % 24;
                let mins = (secs / 60) % 60;
                format!("{:02}:{:02}", hours, mins)
            })
            .unwrap_or_else(|_| "??:??".to_string());
        
        if args.first().map(|s| s.as_str()) == Some("--clock") {
            PluginResult::ok(datetime)
        } else {
            PluginResult::ok(datetime)
        }
    }

    fn on_prompt(&self, _prompt: &str) -> Option<String> {
        let now = std::time::SystemTime::now();
        if let Ok(d) = now.duration_since(std::time::UNIX_EPOCH) {
            let secs = d.as_secs();
            let hours = (secs / 3600) % 24;
            let mins = (secs / 60) % 60;
            return Some(format!("\x1b[90m{:02}:{:02}\x1b[0m", hours, mins));
        }
        None
    }
}

pub struct CalcPlugin;

impl CalcPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl ShellPlugin for CalcPlugin {
    fn name(&self) -> &str {
        "calc"
    }

    fn description(&self) -> &str {
        "Simple calculator"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn execute(&self, args: &[String], _context: &PluginContext) -> PluginResult {
        if args.is_empty() {
            return PluginResult::err("Usage: calc <expression>".to_string());
        }
        
        let expr = args.join(" ");
        
        let result = evaluate_expression(&expr);
        
        match result {
            Ok(n) => PluginResult::ok(n.to_string()),
            Err(e) => PluginResult::err(e),
        }
    }
}

fn evaluate_expression(expr: &str) -> Result<f64, String> {
    let expr = expr.replace(" ", "");
    
    fn parse_number(s: &str) -> Result<(f64, &str), String> {
        let mut chars = s.chars();
        let mut num_str = String::new();
        
        while let Some(c) = chars.clone().next() {
            if c.is_digit(10) || c == '.' || (c == '-' && num_str.is_empty()) {
                num_str.push(c);
                chars.next();
            } else {
                break;
            }
        }
        
        num_str.parse::<f64>()
            .map(|n| (n, &s[num_str.len()..]))
            .map_err(|_| "Invalid number".to_string())
    }
    
    fn parse_term(s: &str) -> Result<(f64, &str), String> {
        let (mut result, rest) = parse_number(s)?;
        let mut s = rest;
        
        loop {
            if s.is_empty() || (!s.starts_with('*') && !s.starts_with('/')) {
                break;
            }
            
            let op = s.chars().next().unwrap();
            s = &s[1..];
            
            let (num, new_rest) = parse_number(s)?;
            s = new_rest;
            
            match op {
                '*' => result *= num,
                '/' => result /= num,
                _ => break,
            }
        }
        
        Ok((result, s))
    }
    
    let (mut result, rest) = parse_term(&expr)?;
    let mut s = rest;
    
    while !s.is_empty() && (s.starts_with('+') || s.starts_with('-')) {
        let op = s.chars().next().unwrap();
        s = &s[1..];
        
        let (num, new_rest) = parse_term(s)?;
        s = new_rest;
        
        match op {
            '+' => result += num,
            '-' => result -= num,
            _ => break,
        }
    }
    
    if !s.is_empty() {
        return Err(format!("Unexpected character: {}", s));
    }
    
    Ok(result)
}

pub struct AliasesPlugin {
    aliases: HashMap<String, String>,
}

impl AliasesPlugin {
    pub fn new() -> Self {
        let mut aliases = HashMap::new();
        aliases.insert("ll".to_string(), "ls -la".to_string());
        aliases.insert("la".to_string(), "ls -a".to_string());
        aliases.insert("l".to_string(), "ls -CF".to_string());
        aliases.insert("..".to_string(), "cd ..".to_string());
        aliases.insert("...".to_string(), "cd ../..".to_string());
        Self { aliases }
    }
}

impl ShellPlugin for AliasesPlugin {
    fn name(&self) -> &str {
        "aliases"
    }

    fn description(&self) -> &str {
        "Built-in aliases"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn execute(&self, args: &[String], context: &PluginContext) -> PluginResult {
        if args.first().map(|s| s.as_str()) == Some("list") {
            let mut output = String::new();
            for (name, cmd) in &context.aliases {
                output.push_str(&format!("{} -> {}\n", name, cmd));
            }
            for (name, cmd) in &self.aliases {
                output.push_str(&format!("{} -> {}\n", name, cmd));
            }
            return PluginResult::ok(output);
        }
        
        PluginResult::err("Usage: aliases list".to_string())
    }

    fn on_command(&self, command: &str) -> Option<String> {
        let trimmed = command.trim();
        
        for (alias, replacement) in &self.aliases {
            if trimmed == *alias {
                return Some(replacement.clone());
            }
            
            if let Some(rest) = trimmed.strip_prefix(alias) {
                if rest.starts_with(' ') || rest.is_empty() {
                    let suffix = if rest.is_empty() { "" } else { rest };
                    return Some(format!("{}{}", replacement, suffix));
                }
            }
        }
        
        None
    }
}