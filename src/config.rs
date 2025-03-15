use crate::error::Error;
use serde::Deserialize;
use std::{
    env::VarError,
    fs::File,
    path::{Path, PathBuf},
};

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct Settings {
    pub default_depth: u8,
    pub picker: Option<String>,
}

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
pub struct Config {
    pub settings: Settings,
    pub paths: Vec<SearchPath>,
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

    pub fn find_dir_recursive(
        path: &Path,
        depth: u8,
        max_depth: u8,
    ) -> Box<dyn Iterator<Item = PathBuf>> {
        if max_depth == 0 {
            let path_iter = std::iter::once(path.into());
            return Box::new(path_iter);
        }

        let dir_iter = path
            .read_dir()
            .unwrap()
            .flatten()
            .filter(|d| d.file_type().map(|ft| ft.is_dir()).unwrap_or(false))
            .flat_map(move |e| {
                let path = e.path();
                if depth < max_depth {
                    let mut paths = vec![path.clone()];
                    let child_iter = Self::find_dir_recursive(&path, depth + 1, max_depth);
                    paths.extend(child_iter);
                    paths
                } else {
                    vec![path]
                }
            });

        Box::new(dir_iter)
    }

    pub fn find_dirs(&self) -> Result<Vec<PathBuf>, Error> {
        let paths = self
            .paths
            .iter()
            .map(|path| path.expand())
            .collect::<Result<Vec<_>, _>>()?;

        let mut all_paths = vec![];

        for p in paths {
            let path = Path::new(p.path());
            if !path.exists() {
                continue;
            }
            let depth = p.depth(self.settings.default_depth);
            let p = Self::find_dir_recursive(path, 1, depth);
            all_paths.extend(p);
        }

        Ok(all_paths)
    }
}
