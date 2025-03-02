mod error;
use error::Error;
mod tmux;

use std::{
    env::VarError,
    fs::File,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum SearchPath {
    Simple(String),
    Complex { path: String, depth: Option<u8> },
}
impl SearchPath {
    pub fn depth(&self, default: u8) -> u8 {
        match self {
            Self::Simple(_) => default,
            Self::Complex { depth, .. } => depth.unwrap_or(default),
        }
    }

    pub fn path(&self) -> &str {
        match self {
            Self::Simple(s) => s.as_str(),
            Self::Complex { path, .. } => path.as_str(),
        }
    }

    pub fn expand(&self) -> Result<Self, Error> {
        fn expand(s: &str) -> Result<String, Error> {
            shellexpand::full(s)
                .map_err(|e| Error::EnvError(e.to_string()))
                .map(|s| s.to_string())
        }

        match self {
            Self::Simple(s) => Ok(Self::Simple(expand(s)?)),
            Self::Complex { path, depth } => Ok(Self::Complex {
                path: expand(path)?,
                depth: *depth,
            }),
        }
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct Settings {
    default_depth: u8,
    picker: Option<String>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct Config {
    settings: Settings,
    paths: Vec<SearchPath>,
}
impl Config {
    const CONFIG_FILE_NAME: &str = "tms";

    pub fn try_open() -> Result<Self, Error> {
        let config_file = std::env::var("TMS_CONFIG")
            .map_err(|e| match e {
                VarError::NotUnicode(e) => panic!("{e:?}"),
                VarError::NotPresent => Error::MissingHome,
            })
            .and_then(|path| File::open(path).map_err(|e| Error::FileError(e.to_string())));

        fn read_file(file: File) -> Result<Config, Error> {
            serde_yml::from_reader(file).map_err(|e| Error::file_error(e.to_string()))
        }

        if let Ok(config) = config_file {
            return read_file(config);
        }

        let home = std::env::var("HOME").expect("'HOME' env var not found");
        let config_path = PathBuf::from(home).join(".config");

        let possible_file_names = [
            format!("{}.yml", Self::CONFIG_FILE_NAME),
            format!("{}.yaml", Self::CONFIG_FILE_NAME),
        ];

        possible_file_names
            .into_iter()
            .find_map(|name| File::open(config_path.join(name)).ok())
            .ok_or_else(|| Error::file_error("Missing config file '~/.config/tms.yml'"))
            .and_then(read_file)
    }

    fn find_dir_recursive(path: &Path, depth: u8, max_depth: u8) -> impl Iterator<Item = PathBuf> {
        path.read_dir()
            .unwrap()
            .flatten()
            .filter(|d| d.file_type().map(|ft| ft.is_dir()).unwrap_or(false))
            .flat_map(move |e| {
                let path = e.path();
                if depth < max_depth {
                    let mut paths = vec![path.clone()];
                    paths.extend(Self::find_dir_recursive(&path, depth + 1, max_depth));
                    paths
                } else {
                    vec![path]
                }
            })
    }

    fn find_dirs(&self) -> Result<Vec<PathBuf>, Error> {
        let paths = self
            .paths
            .iter()
            .map(|path| path.expand())
            .collect::<Result<Vec<_>, _>>()?;

        let mut all_paths = vec![];

        for p in paths {
            match p {
                SearchPath::Simple(p) => {
                    let path = Path::new(&p);
                    if !path.exists() {
                        continue;
                    }

                    let p = Self::find_dir_recursive(path, 1, self.settings.default_depth);
                    all_paths.extend(p);
                }
                SearchPath::Complex { path, depth } => {
                    let path = Path::new(&path);
                    if !path.exists() {
                        continue;
                    }

                    let p = Self::find_dir_recursive(
                        path,
                        1,
                        depth.unwrap_or(self.settings.default_depth),
                    );
                    all_paths.extend(p);
                }
            }
        }

        Ok(all_paths)
    }
}

fn run_finder(Settings { picker, .. }: &Settings, paths: &[PathBuf]) -> Option<PathBuf> {
    let picker = picker.as_deref().unwrap_or("fzf-tmux -p 50%");

    let paths = paths.iter().filter_map(|p| p.to_str());

    let mut paths_input = String::new();
    for p in paths {
        paths_input.push_str(p);
        paths_input.push('\n');
    }

    let (cmd, args) = picker
        .split_once(' ')
        .map(|(cmd, args)| {
            let args = args.split(' ').collect::<Vec<_>>();
            (cmd, args)
        })
        .unwrap_or((picker, vec![]));

    let mut proc = Command::new(cmd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("Failed to spawn picker command \"{picker}\", {e}"));

    proc.stdin
        .as_mut()
        .expect("Failed to get stdin")
        .write_all(paths_input.as_bytes())
        .expect("Failed to write to stdin");

    let res = proc
        .wait_with_output()
        .expect("Failed to run picker command");

    if res.status.success() {
        let s = String::from_utf8(res.stdout).expect("Picker output is not UTF-8");
        let s = &s[..s.len() - 1]; // Strip ending new line
        let path = PathBuf::from(s);
        Some(path)
    } else {
        None
    }
}

fn get_dir_name(dir: &Path) -> String {
    let s = dir
        .file_name()
        .and_then(|s| s.to_str())
        .expect("Dir is not valid UTF-8");

    s.replace('.', "_")
}

fn main() {
    let config = Config::try_open().unwrap();
    let paths = config.find_dirs().unwrap();

    let selected_path = if let Some(path) = run_finder(&config.settings, &paths) {
        path
    } else {
        // Exit if picker is canceled
        return;
    };

    let path_str = selected_path.to_str().expect("Selected path is not UTF-8");
    let dir_name = get_dir_name(&selected_path);

    if !tmux::has_session(&dir_name) {
        tmux::new_session(&dir_name, path_str);
    }

    if std::env::var("TMUX").is_ok() {
        tmux::switch(&dir_name);
    } else {
        tmux::attach(&dir_name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_test() -> Result<(), Box<dyn std::error::Error>> {
        let yml = r#"
            settings:
                default_depth: 8
            paths:
                - first
                - path: second
                - path: third
                  depth: 2
        "#;
        let yml = serde_yml::from_str::<Config>(yml)?;

        assert_eq!(
            yml,
            Config {
                settings: Settings {
                    default_depth: 8,
                    picker: None
                },
                paths: vec![
                    SearchPath::Simple("first".into()),
                    SearchPath::Complex {
                        path: "second".into(),
                        depth: None
                    },
                    SearchPath::Complex {
                        path: "third".into(),
                        depth: Some(2)
                    }
                ]
            }
        );

        Ok(())
    }
}
