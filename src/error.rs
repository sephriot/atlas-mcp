use thiserror::Error;

#[derive(Debug, Error)]
pub enum AtlasError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Context error: {0}")]
    Context(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Validation error: {0}")]
    Validation(String),
}
