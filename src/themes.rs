use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Theme {
    pub name: String,
    pub prompt: PromptTheme,
    pub syntax: SyntaxTheme,
    pub colors: ColorScheme,
}

#[derive(Debug, Clone)]
pub struct PromptTheme {
    pub template: String,
    pub user_color: String,
    pub host_color: String,
    pub cwd_color: String,
    pub git_color: String,
    pub normal_color: String,
}

#[derive(Debug, Clone)]
pub struct SyntaxTheme {
    pub command_color: String,
    pub arg_color: String,
    pub string_color: String,
    pub variable_color: String,
    pub operator_color: String,
    pub comment_color: String,
    pub error_color: String,
    pub path_color: String,
}

#[derive(Debug, Clone)]
pub struct ColorScheme {
    pub foreground: String,
    pub background: String,
    pub cursor: String,
    pub selection: String,
    pub status_ok: String,
    pub status_warn: String,
    pub status_err: String,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            prompt: PromptTheme::default(),
            syntax: SyntaxTheme::default(),
            colors: ColorScheme::default(),
        }
    }
}

impl Default for PromptTheme {
    fn default() -> Self {
        Self {
            template: "{user}@{host}:{cwd}$ ".to_string(),
            user_color: "green".to_string(),
            host_color: "cyan".to_string(),
            cwd_color: "blue".to_string(),
            git_color: "yellow".to_string(),
            normal_color: "white".to_string(),
        }
    }
}

impl Default for SyntaxTheme {
    fn default() -> Self {
        Self {
            command_color: "cyan".to_string(),
            arg_color: "white".to_string(),
            string_color: "green".to_string(),
            variable_color: "yellow".to_string(),
            operator_color: "magenta".to_string(),
            comment_color: "gray".to_string(),
            error_color: "red".to_string(),
            path_color: "orange".to_string(),
        }
    }
}

impl Default for ColorScheme {
    fn default() -> Self {
        Self {
            foreground: "white".to_string(),
            background: "black".to_string(),
            cursor: "white".to_string(),
            selection: "blue".to_string(),
            status_ok: "green".to_string(),
            status_warn: "yellow".to_string(),
            status_err: "red".to_string(),
        }
    }
}

impl Theme {
    pub fn from_file(path: &std::path::Path) -> Option<Self> {
        let content = std::fs::read_to_string(path).ok()?;
        Self::parse(&content)
    }

    pub fn parse(content: &str) -> Option<Self> {
        let mut theme = Theme::default();
        
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim();
                
                match key {
                    "name" => theme.name = value.to_string(),
                    "prompt.template" => theme.prompt.template = value.to_string(),
                    "prompt.user_color" => theme.prompt.user_color = value.to_string(),
                    "prompt.host_color" => theme.prompt.host_color = value.to_string(),
                    "prompt.cwd_color" => theme.prompt.cwd_color = value.to_string(),
                    "prompt.git_color" => theme.prompt.git_color = value.to_string(),
                    "prompt.normal_color" => theme.prompt.normal_color = value.to_string(),
                    "syntax.command" => theme.syntax.command_color = value.to_string(),
                    "syntax.arg" => theme.syntax.arg_color = value.to_string(),
                    "syntax.string" => theme.syntax.string_color = value.to_string(),
                    "syntax.variable" => theme.syntax.variable_color = value.to_string(),
                    "syntax.operator" => theme.syntax.operator_color = value.to_string(),
                    "syntax.comment" => theme.syntax.comment_color = value.to_string(),
                    "syntax.error" => theme.syntax.error_color = value.to_string(),
                    "syntax.path" => theme.syntax.path_color = value.to_string(),
                    "color.fg" => theme.colors.foreground = value.to_string(),
                    "color.bg" => theme.colors.background = value.to_string(),
                    "color.cursor" => theme.colors.cursor = value.to_string(),
                    "color.selection" => theme.colors.selection = value.to_string(),
                    "color.status_ok" => theme.colors.status_ok = value.to_string(),
                    "color.status_warn" => theme.colors.status_warn = value.to_string(),
                    "color.status_err" => theme.colors.status_err = value.to_string(),
                    _ => {}
                }
            }
        }
        
        Some(theme)
    }

    pub fn to_file(&self, path: &std::path::Path) -> std::io::Result<()> {
        let mut content = String::new();
        content.push_str(&format!("# ansSH Theme: {}\n\n", self.name));
        content.push_str(&format!("name={}\n", self.name));
        content.push_str("\n# Prompt Settings\n");
        content.push_str(&format!("prompt.template={}\n", self.prompt.template));
        content.push_str(&format!("prompt.user_color={}\n", self.prompt.user_color));
        content.push_str(&format!("prompt.host_color={}\n", self.prompt.host_color));
        content.push_str(&format!("prompt.cwd_color={}\n", self.prompt.cwd_color));
        content.push_str(&format!("prompt.git_color={}\n", self.prompt.git_color));
        content.push_str(&format!("prompt.normal_color={}\n", self.prompt.normal_color));
        content.push_str("\n# Syntax Highlighting\n");
        content.push_str(&format!("syntax.command={}\n", self.syntax.command_color));
        content.push_str(&format!("syntax.arg={}\n", self.syntax.arg_color));
        content.push_str(&format!("syntax.string={}\n", self.syntax.string_color));
        content.push_str(&format!("syntax.variable={}\n", self.syntax.variable_color));
        content.push_str(&format!("syntax.operator={}\n", self.syntax.operator_color));
        content.push_str(&format!("syntax.comment={}\n", self.syntax.comment_color));
        content.push_str(&format!("syntax.error={}\n", self.syntax.error_color));
        content.push_str(&format!("syntax.path={}\n", self.syntax.path_color));
        content.push_str("\n# Color Scheme\n");
        content.push_str(&format!("color.fg={}\n", self.colors.foreground));
        content.push_str(&format!("color.bg={}\n", self.colors.background));
        content.push_str(&format!("color.cursor={}\n", self.colors.cursor));
        content.push_str(&format!("color.selection={}\n", self.colors.selection));
        content.push_str(&format!("color.status_ok={}\n", self.colors.status_ok));
        content.push_str(&format!("color.status_warn={}\n", self.colors.status_warn));
        content.push_str(&format!("color.status_err={}\n", self.colors.status_err));
        
        std::fs::write(path, content)
    }
}

pub struct ThemeManager {
    themes: HashMap<String, Theme>,
    current_theme: Theme,
    themes_dir: std::path::PathBuf,
}

impl ThemeManager {
    pub fn new(home: &std::path::Path) -> Self {
        let themes_dir = home.join(".config/anssh/themes");
        std::fs::create_dir_all(&themes_dir).ok();
        
        let mut manager = Self {
            themes: HashMap::new(),
            current_theme: Theme::default(),
            themes_dir,
        };
        
        manager.load_builtin_themes();
        manager.load_themes_from_dir();
        
        if let Some(default_theme) = manager.themes.get("default") {
            manager.current_theme = default_theme.clone();
        }
        
        manager
    }

    fn load_builtin_themes(&mut self) {
        self.themes.insert("default".to_string(), Theme::default());
        
        let hacker_theme = Theme {
            name: "hacker".to_string(),
            prompt: PromptTheme {
                template: "[$green{user}$clear@$cyan{host}$clear]$gray:$reset$cwd$clear> ".to_string(),
                user_color: "green".to_string(),
                host_color: "cyan".to_string(),
                cwd_color: "green".to_string(),
                git_color: "yellow".to_string(),
                normal_color: "gray".to_string(),
            },
            syntax: SyntaxTheme {
                command_color: "green".to_string(),
                arg_color: "white".to_string(),
                string_color: "yellow".to_string(),
                variable_color: "cyan".to_string(),
                operator_color: "magenta".to_string(),
                comment_color: "gray".to_string(),
                error_color: "red".to_string(),
                path_color: "orange".to_string(),
            },
            colors: ColorScheme {
                foreground: "green".to_string(),
                background: "black".to_string(),
                cursor: "green".to_string(),
                selection: "darkgreen".to_string(),
                status_ok: "green".to_string(),
                status_warn: "yellow".to_string(),
                status_err: "red".to_string(),
            },
        };
        self.themes.insert("hacker".to_string(), hacker_theme);
        
        let ocean_theme = Theme {
            name: "ocean".to_string(),
            prompt: PromptTheme {
                template: "$blue{user}$clear@$cyan{host}$clear:$blue{cwd}$clear$ $cyan➜$clear ".to_string(),
                user_color: "blue".to_string(),
                host_color: "cyan".to_string(),
                cwd_color: "blue".to_string(),
                git_color: "yellow".to_string(),
                normal_color: "white".to_string(),
            },
            syntax: SyntaxTheme {
                command_color: "cyan".to_string(),
                arg_color: "white".to_string(),
                string_color: "green".to_string(),
                variable_color: "yellow".to_string(),
                operator_color: "magenta".to_string(),
                comment_color: "gray".to_string(),
                error_color: "red".to_string(),
                path_color: "cyan".to_string(),
            },
            colors: ColorScheme {
                foreground: "white".to_string(),
                background: "blue".to_string(),
                cursor: "white".to_string(),
                selection: "darkblue".to_string(),
                status_ok: "green".to_string(),
                status_warn: "yellow".to_string(),
                status_err: "red".to_string(),
            },
        };
        self.themes.insert("ocean".to_string(), ocean_theme);
        
        let sunset_theme = Theme {
            name: "sunset".to_string(),
            prompt: PromptTheme {
                template: "{user}@💻 {cwd} ".to_string(),
                user_color: "orange".to_string(),
                host_color: "magenta".to_string(),
                cwd_color: "yellow".to_string(),
                git_color: "green".to_string(),
                normal_color: "white".to_string(),
            },
            syntax: SyntaxTheme {
                command_color: "orange".to_string(),
                arg_color: "white".to_string(),
                string_color: "cyan".to_string(),
                variable_color: "yellow".to_string(),
                operator_color: "magenta".to_string(),
                comment_color: "gray".to_string(),
                error_color: "red".to_string(),
                path_color: "cyan".to_string(),
            },
            colors: ColorScheme {
                foreground: "white".to_string(),
                background: "black".to_string(),
                cursor: "yellow".to_string(),
                selection: "darkyellow".to_string(),
                status_ok: "green".to_string(),
                status_warn: "orange".to_string(),
                status_err: "red".to_string(),
            },
        };
        self.themes.insert("sunset".to_string(), sunset_theme);
    }

    fn load_themes_from_dir(&mut self) {
        if let Ok(entries) = std::fs::read_dir(&self.themes_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |e| e == "theme") {
                    if let Some(theme) = Theme::from_file(&path) {
                        self.themes.insert(theme.name.clone(), theme);
                    }
                }
            }
        }
    }

    pub fn get_theme(&self, name: &str) -> Option<&Theme> {
        self.themes.get(name)
    }

    pub fn set_theme(&mut self, name: &str) -> bool {
        if let Some(theme) = self.themes.get(name) {
            self.current_theme = theme.clone();
            true
        } else {
            false
        }
    }

    pub fn save_theme(&self, name: &str) -> std::io::Result<std::path::PathBuf> {
        let path = self.themes_dir.join(format!("{}.theme", name));
        if let Some(theme) = self.themes.get(name) {
            theme.to_file(&path)?;
        }
        Ok(path)
    }

    pub fn list_themes(&self) -> Vec<String> {
        let mut names: Vec<_> = self.themes.keys().cloned().collect();
        names.sort();
        names
    }

    pub fn current(&self) -> &Theme {
        &self.current_theme
    }
}

#[derive(Debug, Clone)]
pub struct Profile {
    pub name: String,
    pub aliases: HashMap<String, String>,
    pub env_vars: HashMap<String, String>,
    pub theme: Option<String>,
    pub prompt_template: Option<String>,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            aliases: HashMap::new(),
            env_vars: HashMap::new(),
            theme: None,
            prompt_template: None,
        }
    }
}

impl Profile {
    pub fn from_file(path: &std::path::Path) -> Option<Self> {
        let content = std::fs::read_to_string(path).ok()?;
        Self::parse(&content)
    }

    pub fn parse(content: &str) -> Option<Self> {
        let mut profile = Profile::default();
        
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            
            if line.starts_with("alias ") {
                if let Some(rest) = line.strip_prefix("alias ") {
                    if let Some((name, value)) = rest.split_once('=') {
                        profile.aliases.insert(name.trim().to_string(), value.trim().to_string());
                    }
                }
            } else if line.starts_with("export ") {
                let line = line.strip_prefix("export ").unwrap();
                if let Some(eq_pos) = line.find('=') {
                    let (key, value) = line.split_at(eq_pos);
                    let key = key.trim();
                    let value = value[1..].trim().trim_matches('"').trim_matches('\'');
                    profile.env_vars.insert(key.to_string(), value.to_string());
                }
            } else if line.starts_with("theme=") {
                profile.theme = Some(line.strip_prefix("theme=").unwrap().trim().to_string());
            } else if line.starts_with("prompt=") {
                profile.prompt_template = Some(line.strip_prefix("prompt=").unwrap().trim().to_string());
            } else if line.starts_with("[profile]") {
                continue;
            } else if let Some((key, value)) = line.split_once('=') {
                if key == "name" {
                    profile.name = value.trim().to_string();
                }
            }
        }
        
        Some(profile)
    }

    pub fn to_file(&self, path: &std::path::Path) -> std::io::Result<()> {
        let mut content = String::new();
        content.push_str("[profile]\n");
        content.push_str(&format!("name={}\n\n", self.name));
        
        if let Some(ref theme) = self.theme {
            content.push_str(&format!("theme={}\n\n", theme));
        }
        
        if let Some(ref prompt) = self.prompt_template {
            content.push_str(&format!("prompt={}\n\n", prompt));
        }
        
        content.push_str("# Environment Variables\n");
        for (key, value) in &self.env_vars {
            content.push_str(&format!("export {}={}\n", key, value));
        }
        
        content.push_str("\n# Aliases\n");
        for (name, value) in &self.aliases {
            content.push_str(&format!("alias {}={}\n", name, value));
        }
        
        std::fs::write(path, content)
    }
}

pub struct ProfileManager {
    profiles: HashMap<String, Profile>,
    active_profile: Profile,
    profiles_dir: std::path::PathBuf,
}

impl ProfileManager {
    pub fn new(home: &std::path::Path) -> Self {
        let profiles_dir = home.join(".config/anssh/profiles");
        std::fs::create_dir_all(&profiles_dir).ok();
        
        let mut manager = Self {
            profiles: HashMap::new(),
            active_profile: Profile::default(),
            profiles_dir,
        };
        
        manager.load_profiles();
        
        if let Some(default_profile) = manager.profiles.get("default") {
            manager.active_profile = default_profile.clone();
        }
        
        manager
    }

    fn load_profiles(&mut self) {
        if let Ok(entries) = std::fs::read_dir(&self.profiles_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |e| e == "profile") {
                    if let Some(profile) = Profile::from_file(&path) {
                        self.profiles.insert(profile.name.clone(), profile);
                    }
                }
            }
        }
        
        if !self.profiles.contains_key("default") {
            self.profiles.insert("default".to_string(), Profile::default());
        }
    }

    pub fn get_profile(&self, name: &str) -> Option<&Profile> {
        self.profiles.get(name)
    }

    pub fn set_active(&mut self, name: &str) -> bool {
        if let Some(profile) = self.profiles.get(name) {
            self.active_profile = profile.clone();
            true
        } else {
            false
        }
    }

    pub fn save_profile(&self, name: &str) -> std::io::Result<std::path::PathBuf> {
        let path = self.profiles_dir.join(format!("{}.profile", name));
        if let Some(profile) = self.profiles.get(name) {
            profile.to_file(&path)?;
        }
        Ok(path)
    }

    pub fn create_profile(&self, name: &str) -> std::io::Result<std::path::PathBuf> {
        let profile = Profile {
            name: name.to_string(),
            ..Default::default()
        };
        let path = self.profiles_dir.join(format!("{}.profile", name));
        profile.to_file(&path)?;
        Ok(path)
    }

    pub fn delete_profile(&self, name: &str) -> std::io::Result<()> {
        if name == "default" {
            return Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "Cannot delete default profile"));
        }
        let path = self.profiles_dir.join(format!("{}.profile", name));
        std::fs::remove_file(path)
    }

    pub fn list_profiles(&self) -> Vec<String> {
        let mut names: Vec<_> = self.profiles.keys().cloned().collect();
        names.sort();
        names
    }

    pub fn active(&self) -> &Profile {
        &self.active_profile
    }
}