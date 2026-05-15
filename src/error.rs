use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum MinecliError {
    #[error("{0}")]
    Message(String),

    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse TOML at {path}: {source}")]
    TomlDeserialize {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    #[error("failed to write TOML at {path}: {source}")]
    TomlSerialize {
        path: PathBuf,
        #[source]
        source: toml::ser::Error,
    },

    #[error("failed to parse JSON response: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Modrinth request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("hash mismatch for {filename}: expected {expected}, got {actual}")]
    HashMismatch {
        filename: String,
        expected: String,
        actual: String,
    },
}

impl MinecliError {
    pub fn message(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }
}

pub type Result<T> = std::result::Result<T, MinecliError>;

pub trait IoResultExt<T> {
    fn at(self, path: impl Into<PathBuf>) -> Result<T>;
}

impl<T> IoResultExt<T> for std::io::Result<T> {
    fn at(self, path: impl Into<PathBuf>) -> Result<T> {
        let path = path.into();
        self.map_err(|source| MinecliError::Io { path, source })
    }
}
