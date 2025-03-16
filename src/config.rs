use crate::error::Error;
use rayon::prelude::*;
use serde::Deserialize;
use std::{
    env::VarError,
    fs::{DirEntry, File},
    os::unix::ffi::OsStrExt,
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
    Complex {
        path: String,
        depth: Option<u8>,
        show_hidden: Option<bool>,
    },
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
            Self::Complex {
                path,
                depth,
                show_hidden,
            } => Ok(Self::Complex {
                path: expand(path)?,
                depth: *depth,
                show_hidden: *show_hidden,
            }),
        }
    }

    pub fn show_hidden(&self) -> bool {
        match self {
            Self::Simple(_) => false,
            Self::Complex { show_hidden, .. } => show_hidden.unwrap_or(false),
        }
    }
}

fn is_hidden_path<P: AsRef<Path>>(path: P) -> bool {
    path.as_ref()
        .file_name()
        .map(|n| n.as_bytes()[0] == b'.')
        .unwrap_or(false)
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
        show_hidden: bool,
        path: &Path,
        depth: u8,
        max_depth: u8,
    ) -> Vec<PathBuf> {
        if max_depth == 0 {
            return vec![path.to_path_buf()];
        }

        fn is_dir(de: &DirEntry) -> bool {
            de.file_type().map(|ft| ft.is_dir()).unwrap_or(false)
        }

        let dir_iter = path
            .read_dir()
            .unwrap()
            .map_while(Result::ok)
            .par_bridge()
            .filter(is_dir)
            .filter(|x| {
                if show_hidden {
                    true
                } else {
                    !is_hidden_path(x.path())
                }
            })
            .flat_map(|e| {
                let path = e.path();
                if depth < max_depth {
                    let iter = std::iter::once(path.clone()).chain(Self::find_dir_recursive(
                        show_hidden,
                        &path,
                        depth + 1,
                        max_depth,
                    ));

                    Vec::from_iter(iter)
                } else {
                    vec![path]
                }
            });

        dir_iter.collect()
    }

    pub fn find_dirs(&self) -> Result<Vec<PathBuf>, Error> {
        let paths = self.paths.par_iter().map(|path| path.expand()).map(|path| {
            let p = path.unwrap();
            let path = Path::new(p.path());
            if !path.exists() {
                panic!("Path does not exist: {}", path.display());
            }
            let depth = p.depth(self.settings.default_depth);
            Self::find_dir_recursive(p.show_hidden(), path, 1, depth)
        });

        Ok(paths.flatten().collect())
    }
}

#[cfg(test)]
mod tests {
    use crate::config::is_hidden_path;

    #[test]
    fn hidden_path_test() {
        assert!(is_hidden_path(".hidden"));
        assert!(!is_hidden_path("not_hidden"));
        assert!(is_hidden_path("a/b/.c"));
        assert!(!is_hidden_path("a/b/c"));
    }
}
