use std::fmt::Display;

#[allow(clippy::enum_variant_names)]
#[derive(Debug)]
pub enum Error {
    FileError(String),
    EnvError(String),
    MissingHome,
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FileError(e) => write!(f, "Config file error '{e}'"),
            Self::EnvError(e) => write!(f, "EnvError: {e}"),
            Self::MissingHome => write!(f, "Missing 'HOME' env var"),
        }
    }
}
impl Error {
    pub fn file_error(e: impl Into<String>) -> Self {
        Self::FileError(e.into())
    }
}
