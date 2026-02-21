use crate::binary::{ReadBinary, WriteBinary};
use crate::error::{CacheError, Error};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::{
    fs::{DirEntry, File},
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Settings {
    pub default_depth: u8,
    pub picker: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
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

    pub fn path(&self) -> &Path {
        match self {
            Self::Simple(s) => Path::new(s.as_str()),
            Self::Complex { path, .. } => Path::new(path.as_str()),
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

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum CacheStatus {
    Hit,
    Miss,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Config {
    pub settings: Settings,
    pub paths: Vec<SearchPath>,
}
impl Config {
    const CONFIG_FILE_NAME: &str = "tms";

    /// Caches to binary file in `~/.cache/tms.bin`
    pub fn cache_binary(&self) -> Result<(), Error> {
        let cache_dir = std::env::var("HOME")
            .map_err(|_| Error::MissingHome)
            .map(PathBuf::from)
            .map(|p| p.join(".cache"))?;

        let cache_new = cache_dir.join(format!("{}.bin.tmp", Self::CONFIG_FILE_NAME));
        let cache_old = cache_dir.join(format!("{}.bin", Self::CONFIG_FILE_NAME));

        let mut cache_file = File::options()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&cache_new)
            .map_err(|e| Error::FileError(e.to_string()))?;

        Self::write_binary(self, &mut cache_file)?;

        std::fs::rename(cache_new, cache_old)
            .map_err(|e| Error::Cache(CacheError::Write("cache file", e)))?;

        Ok(())
    }

    fn load_cached_file(path: &Path) -> Result<Self, Error> {
        File::open(path)
            .map_err(|e| Error::FileError(e.to_string()))
            .and_then(|mut x| Self::read_binary(&mut x))
    }

    pub fn try_open() -> Result<(CacheStatus, Self), Error> {
        let home = std::env::var("HOME").expect("'HOME' env var not found");
        let home = PathBuf::from(home);
        let cache_file_path = home.join(".cache").join("tms.bin");
        let config_path = home.join(".config");

        let config_file_path = if let Ok(config_path) = std::env::var("TMS_CONFIG") {
            Some(PathBuf::from(config_path))
        } else {
            let possible_file_names = [
                format!("{}.yml", Self::CONFIG_FILE_NAME),
                format!("{}.yaml", Self::CONFIG_FILE_NAME),
            ];

            possible_file_names.into_iter().find_map(|name| {
                let path = config_path.join(name);
                if path.exists() { Some(path) } else { None }
            })
        };

        if let Some(config_file_path) = &config_file_path
            && config_file_path.exists()
            && cache_file_path.exists()
        {
            let cache_mtime = std::fs::metadata(&cache_file_path).and_then(|x| x.modified());
            let config_mtime = std::fs::metadata(config_file_path).and_then(|x| x.modified());

            match (config_mtime, cache_mtime) {
                (Ok(config), Ok(cache)) if cache > config => {
                    if let Ok(config) = Self::load_cached_file(&cache_file_path) {
                        return Ok((CacheStatus::Hit, config));
                    }
                }
                (Err(_), Ok(_)) => {
                    if let Ok(config) = Self::load_cached_file(&cache_file_path) {
                        return Ok((CacheStatus::Hit, config));
                    }
                }

                _ => {}
            }
        }

        fn read_file(file: File) -> Result<Config, Error> {
            serde_yml::from_reader(file).map_err(|e| Error::file_error(e.to_string()))
        }

        match config_file_path {
            Some(path) => {
                let file = File::open(path).map_err(|e| Error::FileError(e.to_string()));

                file.and_then(read_file).map(|x| (CacheStatus::Miss, x))
            }
            None => Err(Error::FileError(format!(
                "Missing config file at '~/.config/{}.yml'",
                Self::CONFIG_FILE_NAME
            ))),
        }
    }

    pub fn find_dir_recursive(
        show_hidden: bool,
        path: &Path,
        depth: u8,
        max_depth: u8,
    ) -> Vec<PathBuf> {
        if max_depth == 0 {
            return vec![];
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
        let paths = self
            .paths
            .par_iter()
            .map(|path| path.expand())
            .filter_map(|x| match x {
                Ok(p) if p.path().exists() => Some(p),
                _ => None,
            })
            .map(|p| {
                let depth = p.depth(self.settings.default_depth);
                let mut paths = Self::find_dir_recursive(p.show_hidden(), p.path(), 1, depth);

                paths.push(p.path().to_path_buf());

                paths
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
