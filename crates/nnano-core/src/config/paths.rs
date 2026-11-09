//! Where the configuration files live, and how their layers are assembled.
//!
//! Precedence, highest first: `CLI > user TOML > system TOML > XDG nanorc >
//! ~/.nanorc > defaults`. A missing file is not a warning; an unreadable one is.
//! `$XDG_CONFIG_HOME` unset falls back to `~/.config`, and no `HOME` simply means
//! no user files.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::{Config, Layer, from_nanorc, from_toml};

/// Files the process would read, resolved from the environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    /// `$XDG_CONFIG_HOME/nnano/config.toml` or `~/.config/nnano/config.toml`.
    pub user: Option<PathBuf>,
    /// `/etc/nnano/config.toml`, read-only.
    pub system: PathBuf,
    /// Lowest precedence first: `~/.nanorc`, then the XDG `nano/nanorc`.
    pub nanorc: Vec<PathBuf>,
}

impl Paths {
    /// Resolution against the real environment.
    pub fn detect() -> Self {
        Self::from_env(|name| std::env::var(name).ok())
    }
    /// Resolution against an injected environment, so the rules are testable
    /// without mutating the process (`edition 2024` makes `set_var` unsafe).
    pub fn from_env(get: impl Fn(&str) -> Option<String>) -> Self {
        let home = get("HOME").filter(|value| !value.is_empty());
        let config_home = get("XDG_CONFIG_HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                home.as_ref()
                    .map(|home| PathBuf::from(home).join(".config"))
            });

        let user = config_home
            .as_ref()
            .map(|dir| dir.join("nnano").join("config.toml"));

        let mut nanorc = Vec::new();
        if let Some(home) = &home {
            nanorc.push(PathBuf::from(home).join(".nanorc"));
        }
        if let Some(dir) = &config_home {
            nanorc.push(dir.join("nano").join("nanorc"));
        }

        Self {
            user,
            system: PathBuf::from("/etc/nnano/config.toml"),
            nanorc,
        }
    }

    /// Every layer, highest precedence first. `cli` is the layer the binary
    /// parsed, so callers pass it here instead of merging by hand.
    pub fn layers(&self, cli: Layer) -> Vec<Layer> {
        let mut layers = vec![cli];
        if let Some(path) = &self.user {
            layers.push(read_toml(path));
        }
        layers.push(read_toml(&self.system));
        // `nanorc` is lowest-first; the vec is highest-first.
        layers.extend(self.nanorc.iter().rev().map(|path| read_nanorc(path)));
        layers
    }

    /// Read, parse and merge everything into the effective configuration.
    pub fn load(&self, cli: Layer) -> Config {
        Config::resolve(self.layers(cli))
    }
}

/// `$XDG_STATE_HOME/nnano`, falling back to `~/.local/state/nnano`. `None` when
/// neither variable is set: then there is nowhere to write a recovery file.
pub fn state_dir(get: impl Fn(&str) -> Option<String>) -> Option<PathBuf> {
    let home = get("HOME").filter(|value| !value.is_empty());
    let state_home = get("XDG_STATE_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            home.as_ref()
                .map(|home| PathBuf::from(home).join(".local").join("state"))
        });
    state_home.map(|dir| dir.join("nnano"))
}

/// [`state_dir`] against the real environment.
pub fn detect_state_dir() -> Option<PathBuf> {
    state_dir(|name| std::env::var(name).ok())
}

fn read_toml(path: &Path) -> Layer {
    match read(path) {
        Ok(text) => from_toml(&origin(path), &text),
        Err(err) => unreadable(path, err),
    }
}

fn read_nanorc(path: &Path) -> Layer {
    match read(path) {
        Ok(text) => from_nanorc(&origin(path), &text),
        Err(err) => unreadable(path, err),
    }
}

/// Missing files are expected: most installs have none of these.
fn read(path: &Path) -> io::Result<String> {
    fs::read_to_string(path)
}

fn unreadable(path: &Path, err: io::Error) -> Layer {
    let mut layer = Layer::new(origin(path));
    if err.kind() != io::ErrorKind::NotFound {
        layer.warn(format!("cannot read: {err}"));
    }
    layer
}

fn origin(path: &Path) -> String {
    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::Paths;
    use crate::config::{Layer, Opt, Value};
    fn unique_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nnano-config-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let pairs: Vec<(String, String)> = pairs
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect();
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.clone())
        }
    }

    #[test]
    fn xdg_config_home_wins_over_home() {
        let paths = Paths::from_env(env(&[
            ("HOME", "/home/ada"),
            ("XDG_CONFIG_HOME", "/home/ada/.xdg"),
        ]));
        assert_eq!(
            paths.user,
            Some(PathBuf::from("/home/ada/.xdg/nnano/config.toml"))
        );
        assert_eq!(
            paths.nanorc,
            vec![
                PathBuf::from("/home/ada/.nanorc"),
                PathBuf::from("/home/ada/.xdg/nano/nanorc"),
            ]
        );
    }

    #[test]
    fn home_alone_falls_back_to_dot_config() {
        let paths = Paths::from_env(env(&[("HOME", "/home/ada")]));
        assert_eq!(
            paths.user,
            Some(PathBuf::from("/home/ada/.config/nnano/config.toml"))
        );
    }

    #[test]
    fn an_empty_xdg_value_is_ignored() {
        let paths = Paths::from_env(env(&[("HOME", "/home/ada"), ("XDG_CONFIG_HOME", "")]));
        assert_eq!(
            paths.user,
            Some(PathBuf::from("/home/ada/.config/nnano/config.toml"))
        );
    }

    #[test]
    fn no_home_means_no_user_files() {
        let paths = Paths::from_env(env(&[]));
        assert_eq!(paths.user, None);
        assert!(paths.nanorc.is_empty());
    }

    #[test]
    fn missing_files_are_not_warnings() {
        let dir = unique_dir("missing");
        let mut paths = Paths::from_env(env(&[("HOME", dir.to_str().unwrap())]));
        paths.system = dir.join("etc/nnano/config.toml");
        let config = paths.load(Layer::new("command line"));
        assert!(config.warnings().is_empty(), "{:?}", config.warnings());
        assert_eq!(config.int(Opt::TabSize), 8);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_whole_precedence_chain_is_applied() {
        let dir = unique_dir("chain");
        let home = dir.join("home");
        let xdg = dir.join(".config");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(xdg.join("nnano")).unwrap();
        std::fs::create_dir_all(xdg.join("nano")).unwrap();

        // Lowest: ~/.nanorc
        std::fs::write(home.join(".nanorc"), "set tabsize 3\n").unwrap();
        // XDG nanorc beats the legacy one.
        std::fs::write(xdg.join("nano/nanorc"), "set tabsize 5\n").unwrap();
        // System beats both nanorc files.
        let system = dir.join("etc/nnano/config.toml");
        std::fs::create_dir_all(system.parent().unwrap()).unwrap();
        std::fs::write(&system, "tab_size = 6\n").unwrap();
        // User beats system.
        std::fs::write(xdg.join("nnano/config.toml"), "tab_size = 7\n").unwrap();

        let mut paths = Paths::from_env(env(&[
            ("HOME", home.to_str().unwrap()),
            ("XDG_CONFIG_HOME", xdg.to_str().unwrap()),
        ]));
        paths.system = system;

        let cli = Layer {
            origin: "command line".to_string(),
            values: vec![(Opt::TabSize, Value::Int(2))],
            warnings: Vec::new(),
        };
        assert_eq!(paths.load(Layer::new("command line")).int(Opt::TabSize), 7);
        assert_eq!(paths.load(cli).int(Opt::TabSize), 2);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unreadable_file_warns_with_its_path() {
        let dir = unique_dir("unreadable");
        let mut paths = Paths::from_env(env(&[("HOME", dir.to_str().unwrap())]));
        // A directory where the config file is expected: readable path, bad read.
        std::fs::create_dir_all(dir.join(".config/nnano/config.toml")).unwrap();
        paths.system = dir.join("etc/nnano/config.toml");
        let config = paths.load(Layer::new("command line"));
        assert_eq!(config.warnings().len(), 1, "{:?}", config.warnings());
        assert!(config.warnings()[0].contains("cannot read"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_state_dir_follows_xdg_or_the_home_fallback() {
        assert_eq!(
            super::state_dir(env(&[
                ("HOME", "/home/ada"),
                ("XDG_STATE_HOME", "/home/ada/.state")
            ])),
            Some(PathBuf::from("/home/ada/.state/nnano"))
        );
        assert_eq!(
            super::state_dir(env(&[("HOME", "/home/ada"), ("XDG_STATE_HOME", "")])),
            Some(PathBuf::from("/home/ada/.local/state/nnano"))
        );
        assert_eq!(super::state_dir(env(&[])), None, "nowhere to write");
    }
}
