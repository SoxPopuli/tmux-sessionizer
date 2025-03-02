mod error;
use error::Error;

use std::{
    fs::File,
    io::Write,
    path::{Path, PathBuf},
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
                std::env::VarError::NotUnicode(e) => panic!("{e:?}"),
                std::env::VarError::NotPresent => Error::MissingHome,
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
            .filter(|d| {
                if let Ok(ty) = d.file_type() {
                    ty.is_dir()
                } else {
                    false
                }
            })
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

fn run_finder(paths: &[PathBuf]) {
    // let paths = paths.iter()
    //     .map(|p| p.to_string_lossy().to_string())
    //     .collect::<Vec<_>>();
    // let index = dialoguer::FuzzySelect::new()
    //     .items(&paths)
    //     .interact()
    //     .unwrap();

    // println!("{}", paths[index]);
    use skim::prelude::*;
    use std::collections::VecDeque;

    let options = SkimOptionsBuilder::default().build().unwrap();

    let mut buf: VecDeque<u8> = VecDeque::default();
    for p in paths {
        let s = p.display().to_string();
        let bytes = s.as_bytes();
        buf.extend(bytes);
        buf.write_all(b"\n").unwrap();
    }

    let reader = SkimItemReader::default();
    let reader = reader.of_bufread(buf);

    let items = Skim::run_with(&options, Some(reader))
        .map(|o| o.selected_items)
        .unwrap();

    for i in items {
        println!("{}", i.output())
    }
}

fn main() {
    let config = Config::try_open().unwrap();
    let paths = config.find_dirs().unwrap();

    run_finder(&paths);
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
                settings: Settings { default_depth: 8 },
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
