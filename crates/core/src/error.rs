use thiserror::Error;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("configuration error: {0}")]
    Config(String),
    #[error("runtime error: {0}")]
    Runtime(String),
    #[error("tool error: {0}")]
    Tool(String),
}
