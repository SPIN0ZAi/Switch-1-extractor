use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ExtractorError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid magic: {magic}. Expected {expected}")]
    InvalidMagic { magic: String, expected: String },

    #[error("Header key (header_key) is missing or prod.keys not loaded")]
    MissingHeaderKey,

    #[error("Title key is missing for Rights ID: {0}")]
    MissingTitleKey(String),

    #[error("Invalid RomFS header: {0}")]
    InvalidRomFsHeader(String),

    #[error("Format error: {0}")]
    Format(String),

    #[error("Crypto error: {0}")]
    Crypto(String),

    #[error("Decompression error: {0}")]
    Decompression(String),

    #[error("Operation cancelled by user")]
    Cancelled,

    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    #[error("Unsupported content or encryption type: {0}")]
    Unsupported(String),
}

pub type Result<T> = std::result::Result<T, ExtractorError>;
