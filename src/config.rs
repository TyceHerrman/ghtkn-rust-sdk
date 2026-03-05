//! Configuration management for ghtkn.
//!
//! Handles reading and validating YAML configuration files for GitHub App
//! authentication. The configuration specifies one or more GitHub Apps that
//! can be used for token generation via OAuth device flow.

use std::collections::HashSet;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::Error;

/// Top-level configuration containing a list of GitHub Apps.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Config {
    pub apps: Vec<App>,
}

/// A single GitHub App configuration entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct App {
    pub name: String,
    pub client_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub git_owner: String,
}

impl Config {
    /// Validate the configuration.
    ///
    /// Rules:
    /// - Must have at least one app
    /// - Each app must have non-empty `name` and `client_id`
    /// - App names must be unique
    /// - App `git_owner` values must be unique (when set)
    pub fn validate(&self) -> crate::Result<()> {
        if self.apps.is_empty() {
            return Err(Error::Config("apps is required".into()));
        }

        let mut names = HashSet::new();
        let mut owners = HashSet::new();

        for app in &self.apps {
            app.validate()?;

            if !names.insert(&app.name) {
                return Err(Error::Config(format!(
                    "app name must be unique: {}",
                    app.name
                )));
            }

            if !app.git_owner.is_empty() && !owners.insert(&app.git_owner) {
                return Err(Error::Config(format!(
                    "app git_owner must be unique: {}",
                    app.git_owner
                )));
            }
        }

        Ok(())
    }
}

impl App {
    /// Validate a single app entry.
    fn validate(&self) -> crate::Result<()> {
        if self.name.is_empty() {
            return Err(Error::Config("name is required".into()));
        }
        if self.client_id.is_empty() {
            return Err(Error::Config("client_id is required".into()));
        }
        Ok(())
    }
}

/// Read and parse a YAML configuration file.
///
/// Returns `Ok(None)` if the path is empty (matches Go SDK behavior where an
/// empty config path is a no-op). Returns an error if the file cannot be opened
/// or contains invalid YAML.
pub fn read(path: impl AsRef<std::path::Path>) -> crate::Result<Option<Config>> {
    let path = path.as_ref();
    if path.as_os_str().is_empty() {
        return Ok(None);
    }
    let contents = std::fs::read_to_string(path)
        .map_err(|e| Error::Config(format!("open a configuration file: {e}")))?;
    let cfg: Config = serde_yaml::from_str(&contents)
        .map_err(|e| Error::Config(format!("decode a configuration file as YAML: {e}")))?;
    Ok(Some(cfg))
}

/// Return the default configuration file path for ghtkn.
///
/// Platform-specific resolution:
/// - **Windows**: `%APPDATA%\ghtkn\ghtkn.yaml`
/// - **Linux/macOS**: `$XDG_CONFIG_HOME/ghtkn/ghtkn.yaml`, falling back to
///   `$HOME/.config/ghtkn/ghtkn.yaml`
///
/// The `get_env` closure allows injecting environment variable lookups for
/// testing without touching the real environment.
pub fn get_path<F>(get_env: F, os: &str) -> crate::Result<PathBuf>
where
    F: Fn(&str) -> Option<String>,
{
    if os == "windows" {
        if let Some(app_data) = get_env("APPDATA")
            && !app_data.is_empty()
        {
            return Ok(PathBuf::from(app_data).join("ghtkn").join("ghtkn.yaml"));
        }
        return Err(Error::Config("APPDATA is required on Windows".into()));
    }

    // Linux / macOS
    if let Some(xdg) = get_env("XDG_CONFIG_HOME")
        && !xdg.is_empty()
    {
        return Ok(PathBuf::from(xdg).join("ghtkn").join("ghtkn.yaml"));
    }
    if let Some(home) = get_env("HOME")
        && !home.is_empty()
    {
        return Ok(PathBuf::from(home)
            .join(".config")
            .join("ghtkn")
            .join("ghtkn.yaml"));
    }
    Err(Error::Config(
        "XDG_CONFIG_HOME or HOME is required on Linux and macOS".into(),
    ))
}

/// Select an app from the configuration.
///
/// Priority (matches Go SDK exactly):
/// 1. If `owner` is non-empty, search for an app whose `git_owner` matches.
///    If found, return it. If not found, **fall through** to the next check.
/// 2. If `key` is empty, return the first app in the list.
/// 3. If `key` is non-empty, search for an app whose `name` matches.
///    Return it if found, otherwise return `None`.
pub fn select_app<'a>(cfg: &'a Config, key: &str, owner: &str) -> Option<&'a App> {
    if cfg.apps.is_empty() {
        return None;
    }

    if !owner.is_empty()
        && let Some(app) = cfg.apps.iter().find(|a| a.git_owner == owner)
    {
        return Some(app);
    }

    if key.is_empty() {
        return Some(&cfg.apps[0]);
    }

    cfg.apps.iter().find(|a| a.name == key)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use super::*;

    // ---------------------------------------------------------------
    // Config::validate
    // ---------------------------------------------------------------

    #[test]
    fn validate_valid_single_app() {
        let cfg = Config {
            apps: vec![App {
                name: "test-app".into(),
                client_id: "xxx".into(),
                git_owner: String::new(),
            }],
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn validate_valid_multiple_apps() {
        let cfg = Config {
            apps: vec![
                App {
                    name: "app1".into(),
                    client_id: "xxx".into(),
                    git_owner: String::new(),
                },
                App {
                    name: "app2".into(),
                    client_id: "yyy".into(),
                    git_owner: String::new(),
                },
            ],
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn validate_empty_apps() {
        let cfg = Config { apps: vec![] };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn validate_app_empty_name() {
        let cfg = Config {
            apps: vec![App {
                name: String::new(),
                client_id: "xxx".into(),
                git_owner: String::new(),
            }],
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn validate_app_empty_client_id() {
        let cfg = Config {
            apps: vec![App {
                name: "app".into(),
                client_id: String::new(),
                git_owner: String::new(),
            }],
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn validate_app_both_empty() {
        let cfg = Config {
            apps: vec![App {
                name: String::new(),
                client_id: String::new(),
                git_owner: String::new(),
            }],
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn validate_duplicate_names() {
        let cfg = Config {
            apps: vec![
                App {
                    name: "dup".into(),
                    client_id: "xxx".into(),
                    git_owner: String::new(),
                },
                App {
                    name: "dup".into(),
                    client_id: "yyy".into(),
                    git_owner: String::new(),
                },
            ],
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn validate_duplicate_git_owners() {
        let cfg = Config {
            apps: vec![
                App {
                    name: "app1".into(),
                    client_id: "xxx".into(),
                    git_owner: "same-owner".into(),
                },
                App {
                    name: "app2".into(),
                    client_id: "yyy".into(),
                    git_owner: "same-owner".into(),
                },
            ],
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn validate_unique_git_owners_with_empty() {
        let cfg = Config {
            apps: vec![
                App {
                    name: "app1".into(),
                    client_id: "xxx".into(),
                    git_owner: "owner1".into(),
                },
                App {
                    name: "app2".into(),
                    client_id: "yyy".into(),
                    git_owner: "owner2".into(),
                },
                App {
                    name: "app3".into(),
                    client_id: "zzz".into(),
                    git_owner: String::new(), // empty is allowed
                },
            ],
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn validate_invalid_app_among_valid_ones() {
        let cfg = Config {
            apps: vec![
                App {
                    name: "valid-app".into(),
                    client_id: "xxx".into(),
                    git_owner: String::new(),
                },
                App {
                    name: String::new(), // invalid
                    client_id: "yyy".into(),
                    git_owner: String::new(),
                },
            ],
        };
        assert!(cfg.validate().is_err());
    }

    // ---------------------------------------------------------------
    // read (YAML parsing)
    // ---------------------------------------------------------------

    #[test]
    fn read_valid_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ghtkn.yaml");
        std::fs::write(
            &path,
            "apps:\n  - name: test-app\n    client_id: Iv1.abc123\n",
        )
        .unwrap();

        let cfg = read(&path).unwrap().unwrap();
        assert_eq!(cfg.apps.len(), 1);
        assert_eq!(cfg.apps[0].name, "test-app");
        assert_eq!(cfg.apps[0].client_id, "Iv1.abc123");
        assert!(cfg.apps[0].git_owner.is_empty());
    }

    #[test]
    fn read_multiple_apps() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ghtkn.yaml");
        std::fs::write(
            &path,
            "apps:\n  - name: app1\n    client_id: xxx\n  - name: app2\n    client_id: yyy\n",
        )
        .unwrap();

        let cfg = read(&path).unwrap().unwrap();
        assert_eq!(cfg.apps.len(), 2);
    }

    #[test]
    fn read_with_git_owner() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ghtkn.yaml");
        std::fs::write(
            &path,
            "apps:\n  - name: my-app\n    client_id: Iv1.abc123\n    git_owner: myorg\n",
        )
        .unwrap();

        let cfg = read(&path).unwrap().unwrap();
        assert_eq!(cfg.apps[0].git_owner, "myorg");
    }

    #[test]
    fn read_empty_path() {
        let result = read("").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn read_file_not_found() {
        let result = read("/nonexistent/path/ghtkn.yaml");
        assert!(result.is_err());
    }

    #[test]
    fn read_invalid_yaml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ghtkn.yaml");
        std::fs::write(&path, "invalid yaml: [").unwrap();

        let result = read(&path);
        assert!(result.is_err());
    }

    // ---------------------------------------------------------------
    // get_path
    // ---------------------------------------------------------------

    fn make_env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        move |key: &str| map.get(key).cloned()
    }

    #[test]
    fn get_path_linux_xdg() {
        let env = make_env(&[("XDG_CONFIG_HOME", "/home/user/.config")]);
        let p = get_path(env, "linux").unwrap();
        assert_eq!(p, PathBuf::from("/home/user/.config/ghtkn/ghtkn.yaml"));
    }

    #[test]
    fn get_path_darwin_xdg() {
        let env = make_env(&[("XDG_CONFIG_HOME", "/custom/config/dir")]);
        let p = get_path(env, "darwin").unwrap();
        assert_eq!(p, PathBuf::from("/custom/config/dir/ghtkn/ghtkn.yaml"));
    }

    #[test]
    fn get_path_linux_home_fallback() {
        let env = make_env(&[("HOME", "/home/user")]);
        let p = get_path(env, "linux").unwrap();
        assert_eq!(p, PathBuf::from("/home/user/.config/ghtkn/ghtkn.yaml"));
    }

    #[test]
    fn get_path_linux_xdg_empty_falls_back_to_home() {
        let env = make_env(&[("XDG_CONFIG_HOME", ""), ("HOME", "/home/user")]);
        let p = get_path(env, "linux").unwrap();
        assert_eq!(p, PathBuf::from("/home/user/.config/ghtkn/ghtkn.yaml"));
    }

    #[test]
    fn get_path_linux_no_vars() {
        let env = make_env(&[]);
        let result = get_path(env, "linux");
        assert!(result.is_err());
    }

    #[test]
    fn get_path_linux_both_empty() {
        let env = make_env(&[("XDG_CONFIG_HOME", ""), ("HOME", "")]);
        let result = get_path(env, "linux");
        assert!(result.is_err());
    }

    #[test]
    fn get_path_windows_appdata() {
        let env = make_env(&[("APPDATA", "C:\\Users\\testuser\\AppData\\Roaming")]);
        let p = get_path(env, "windows").unwrap();
        assert_eq!(
            p,
            PathBuf::from("C:\\Users\\testuser\\AppData\\Roaming")
                .join("ghtkn")
                .join("ghtkn.yaml")
        );
    }

    #[test]
    fn get_path_windows_no_appdata() {
        let env = make_env(&[]);
        let result = get_path(env, "windows");
        assert!(result.is_err());
    }

    #[test]
    fn get_path_windows_empty_appdata() {
        let env = make_env(&[("APPDATA", "")]);
        let result = get_path(env, "windows");
        assert!(result.is_err());
    }

    #[test]
    fn get_path_relative_xdg() {
        let env = make_env(&[("XDG_CONFIG_HOME", "relative/config")]);
        let p = get_path(env, "linux").unwrap();
        assert_eq!(p, PathBuf::from("relative/config/ghtkn/ghtkn.yaml"));
    }

    #[test]
    fn get_path_path_with_spaces() {
        let env = make_env(&[("XDG_CONFIG_HOME", "/path with spaces/config")]);
        let p = get_path(env, "darwin").unwrap();
        assert_eq!(
            p,
            PathBuf::from("/path with spaces/config/ghtkn/ghtkn.yaml")
        );
    }

    // ---------------------------------------------------------------
    // select_app
    // ---------------------------------------------------------------

    fn test_config() -> Config {
        Config {
            apps: vec![
                App {
                    name: "app1".into(),
                    client_id: "xxx".into(),
                    git_owner: "owner1".into(),
                },
                App {
                    name: "app2".into(),
                    client_id: "yyy".into(),
                    git_owner: "owner2".into(),
                },
                App {
                    name: "app3".into(),
                    client_id: "zzz".into(),
                    git_owner: String::new(),
                },
            ],
        }
    }

    #[test]
    fn select_app_empty_config() {
        let cfg = Config { apps: vec![] };
        assert!(select_app(&cfg, "any", "").is_none());
    }

    #[test]
    fn select_app_by_owner() {
        let cfg = test_config();
        let app = select_app(&cfg, "", "owner2").unwrap();
        assert_eq!(app.name, "app2");
    }

    #[test]
    fn select_app_by_name() {
        let cfg = test_config();
        let app = select_app(&cfg, "app3", "").unwrap();
        assert_eq!(app.name, "app3");
    }

    #[test]
    fn select_app_owner_priority_over_name() {
        let cfg = test_config();
        // owner matches app1, key matches app2 -- owner wins
        let app = select_app(&cfg, "app2", "owner1").unwrap();
        assert_eq!(app.name, "app1");
    }

    #[test]
    fn select_app_name_not_found() {
        let cfg = test_config();
        assert!(select_app(&cfg, "nonexistent", "").is_none());
    }

    #[test]
    fn select_app_default_first() {
        let cfg = test_config();
        let app = select_app(&cfg, "", "").unwrap();
        assert_eq!(app.name, "app1");
    }

    #[test]
    fn select_app_owner_not_found_falls_through_to_default() {
        // Matches Go SDK: owner miss falls through, key is empty => first app
        let cfg = test_config();
        let app = select_app(&cfg, "", "nonexistent-owner").unwrap();
        assert_eq!(app.name, "app1");
    }

    #[test]
    fn select_app_owner_not_found_falls_through_to_key() {
        // Matches Go SDK: owner miss falls through, key matches => that app
        let cfg = test_config();
        let app = select_app(&cfg, "app3", "nonexistent-owner").unwrap();
        assert_eq!(app.name, "app3");
    }

    #[test]
    fn select_app_owner_not_found_key_not_found() {
        let cfg = test_config();
        assert!(select_app(&cfg, "nonexistent", "nonexistent-owner").is_none());
    }
}
