use std::fs;
use std::path::Path;

pub(crate) fn ensure_config_dir(home: &Path) {
    let config_dir = home.join(".config/anssh");
    if !config_dir.exists() {
        fs::create_dir_all(&config_dir).expect("Failed to create config directory");
    }
}

pub(crate) fn expand_path(arg: &str, cwd: &str, home: &Path) -> String {
    let mut path_str = arg.to_string();
    if path_str.starts_with("~") {
        path_str = crate::expansions::expand_tilde(&path_str, home);
    }
    let path = Path::new(&path_str);
    if path.is_absolute() || path.exists() {
        let cwd_path = Path::new(cwd);
        let abs_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            cwd_path.join(path)
        };

        match abs_path.canonicalize() {
            Ok(p) => p.to_str().unwrap().to_string(),
            Err(_) => abs_path.to_str().unwrap().to_string(),
        }
    } else {
        arg.to_string()
    }
}
