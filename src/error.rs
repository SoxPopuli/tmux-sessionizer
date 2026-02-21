use std::fmt::Display;

#[derive(Debug)]
pub enum CacheError {
    Write(&'static str, std::io::Error),
    Read(&'static str, std::io::Error),
}
impl Display for CacheError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Write(field, err) => write!(f, "Failed to write cache field: {field} - {err}"),
            Self::Read(field, err) => write!(f, "Failed to read cache field: {field} - {err}"),
        }
    }
}

#[allow(clippy::enum_variant_names)]
#[derive(Debug)]
pub enum Error {
    FileError(String),
    EnvError(String),
    MissingHome,
    Cache(CacheError),
}

impl std::error::Error for Error {}
impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FileError(e) => write!(f, "Config file error '{e}'"),
            Self::EnvError(e) => write!(f, "EnvError: {e}"),
            Self::MissingHome => write!(f, "Missing 'HOME' env var"),
            Self::Cache(e) => write!(f, "Cache Error: {e}"),
        }
    }
}
impl Error {
    pub fn file_error(e: impl Into<String>) -> Self {
        Self::FileError(e.into())
    }
}
