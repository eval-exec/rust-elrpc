use thiserror::Error;

pub type EpcResult<T> = Result<T, EpcError>;

#[derive(Error, Debug)]
pub enum EpcError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(String),
    
    #[error("Protocol error: {0}")]
    Protocol(String),
    
    #[error("Method not found: {0}")]
    MethodNotFound(String),
    
    #[error("Application error: {0}")]
    Application(String),
    
    #[error("Connection closed")]
    ConnectionClosed,
    
    #[error("Timeout")]
    Timeout,
    
    #[error("Invalid message format")]
    InvalidMessage,
}

impl EpcError {
    pub fn serialization(msg: impl Into<String>) -> Self {
        EpcError::Serialization(msg.into())
    }
    
    pub fn protocol(msg: impl Into<String>) -> Self {
        EpcError::Protocol(msg.into())
    }
    
    pub fn application(msg: impl Into<String>) -> Self {
        EpcError::Application(msg.into())
    }
}