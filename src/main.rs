mod config;
mod error;
use config::{Config, Settings};
use error::Error;
mod tmux;

use std::{
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

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
    use crate::config::SearchPath;

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
