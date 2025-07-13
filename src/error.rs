use thiserror::Error;

#[derive(Error, Debug)]
pub enum ERPCError {
    #[error("connection closed")]
    ConnectionClosed,
    
    #[error("method not found: {0}")]
    MethodNotFound(String),
    
    #[error("serialization error: {0}")]
    SerializationError(String),
    
    #[error("protocol error: {0}")]
    ProtocolError(String),
    
    #[error("application error: {class}: {message}")]
    ApplicationError {
        class: String,
        message: String,
        backtrace: Vec<String>,
    },
    
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("parse error: {0}")]
    Parse(#[from] lexpr::parse::Error),
    
    #[error("encoding error: {0}")]
    Encoding(String),
    
    #[error("UTF-8 error: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    
    #[error("invalid message format: {0}")]
    InvalidMessageFormat(String),
    
    #[error("timeout error")]
    Timeout,
    
    #[error("process error: {0}")]
    ProcessError(String),

    #[error("invalid argument: {0}")]
    InvalidArgument(String),
}

pub type Result<T> = std::result::Result<T, ERPCError>;