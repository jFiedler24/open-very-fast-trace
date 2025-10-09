use thiserror::Error;

/// Error types for the Open Very Fast Trace library
/// [impl->dsn~error-types~1]
#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),
    
    #[error("Parse error: {message} at {location}")]
    Parse {
        message: String,
        location: String,
    },
    
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("Template error: {0}")]
    Template(#[from] askama::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("TOML parsing error: {0}")]
    Toml(#[from] toml::de::Error),
    
    #[error("TOML serialization error: {0}")]
    TomlSer(#[from] toml::ser::Error),
    
    #[error("Invalid specification item ID: {0}")]
    InvalidId(String),
    
    #[error("Duplicate specification item: {0}")]
    Duplicate(String),
    
    #[error("Requirement not found: {0}")]
    RequirementNotFound(String),
}

/// Result type alias for the library
pub type Result<T> = std::result::Result<T, Error>;
