use std::sync::Arc;

use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use bytes::BytesMut;
use tracing::debug;
use lexpr::Value;
use serde::{Serialize, Deserialize};

use crate::error::ERPCError;
use crate::protocol::{Framer, Message};
use crate::registry::{MethodInfo, MethodRegistry};

/// EPC Client
pub struct Client {
    stream: Arc<Mutex<TcpStream>>,
    registry: Arc<MethodRegistry>,
    next_uid: Arc<Mutex<u64>>,
}

impl Client {
    /// Connect to a server
    pub async fn connect(addr: impl Into<String>) -> std::result::Result<Self, ERPCError> {
        let addr = addr.into();
        let stream = TcpStream::connect(&addr).await
            .map_err(|e| ERPCError::Io(e))?;
        
        debug!("Connected to EPC server at {}", addr);
        
        Ok(Client {
            stream: Arc::new(Mutex::new(stream)),
            registry: Arc::new(MethodRegistry::new()),
            next_uid: Arc::new(Mutex::new(1)),
        })
    }

    /// Get the method registry for registering client-side methods
    pub fn registry(&self
    ) -> &Arc<MethodRegistry> {
        &self.registry
    }

    /// Generate next UID
    fn next_uid(&self
    ) -> u64 {
        let mut uid = self.next_uid.blocking_lock();
        let result = *uid;
        *uid += 1;
        result
    }

    /// Send a message and wait for response
    async fn send_message(
        &self,
        message: Message,
    ) -> std::result::Result<Message, ERPCError> {
        let message_str = message.to_sexp()?;
        let framed = Framer::frame(message_str.as_bytes());
        
        {
            let mut stream = self.stream.lock().await;
            stream.write_all(&framed).await
                .map_err(|e| ERPCError::Io(e))?;
        }
        
        let mut buffer = BytesMut::with_capacity(1024);
        
        loop {
            {
                let mut stream = self.stream.lock().await;
                let bytes_read = stream.read_buf(&mut buffer).await
                    .map_err(|e| ERPCError::Io(e))?;
                
                if bytes_read == 0 {
                    return Err(ERPCError::ConnectionClosed);
                }
            }
            
            if let Some(message_bytes) = Framer::extract_message(&mut buffer) {
                let message_str = std::str::from_utf8(&message_bytes)
                    .map_err(|e| ERPCError::InvalidMessageFormat(e.to_string()))?;
                
                return Message::from_sexp(message_str);
            }
        }
    }

    /// Call a method synchronously
    pub async fn call_sync<Args, Ret>(
        &self,
        method: &str,
        args: Args,
    ) -> std::result::Result<Ret, ERPCError>
    where
        Args: Serialize,
        Ret: for<'de> Deserialize<'de>,
    {
        let args_value = serde_lexpr::to_value(&args)
            .map_err(|e| ERPCError::SerializationError(e.to_string()))?;
        
        let uid = self.next_uid();
        let message = Message::new_call(uid, method, args_value);
        
        let response = self.send_message(message).await?;
        
        match response {
            Message::Return { result, .. } => {
                serde_lexpr::from_value(&result)
                    .map_err(|e| ERPCError::SerializationError(e.to_string()))
            }
            Message::ReturnError { error, .. } => {
                Err(ERPCError::ApplicationError {
                    class: "RuntimeError".to_string(),
                    message: error,
                    backtrace: vec![],
                })
            }
            Message::EPCError { error, .. } => {
                Err(ERPCError::ProtocolError(error))
            }
            _ => {
                Err(ERPCError::InvalidMessageFormat(
                    "Unexpected response type".to_string(),
                ))
            }
        }
    }

    /// Call a method asynchronously (returns a future)
    pub async fn call_async<Args, Ret>(
        &self,
        method: &str,
        args: Args,
    ) -> std::result::Result<Ret, ERPCError>
    where
        Args: Serialize,
        Ret: for<'de> Deserialize<'de>,
    {
        self.call_sync(method, args).await
    }

    /// Query available methods from server
    pub async fn query_methods(&self
    ) -> std::result::Result<Vec<MethodInfo>, ERPCError> {
        let uid = self.next_uid();
        let message = Message::new_methods(uid);
        
        let response = self.send_message(message).await?;
        
        match response {
            Message::Return { result, .. } => {
                let methods = serde_lexpr::from_value(&result)
                    .map_err(|e| ERPCError::SerializationError(e.to_string()))?;
                Ok(methods)
            }
            _ => {
                Err(ERPCError::InvalidMessageFormat(
                    "Expected methods response".to_string(),
                ))
            }
        }
    }

    /// Register a method with closure (for client-side methods)
    pub async fn register_method<F, Args, Ret>(
        &self,
        name: impl Into<String>,
        func: F,
        arg_spec: Option<impl Into<String>>,
        docstring: Option<impl Into<String>>,
    ) -> std::result::Result<(), ERPCError>
    where
        F: Fn(Args) -> std::result::Result<Ret, ERPCError> + Send + Sync + 'static,
        Args: for<'de> Deserialize<'de> + Send,
        Ret: Serialize + Send,
    {
        self.registry.register_closure(name, func, arg_spec, docstring).await
    }

    /// Close the connection
    pub async fn close(&self
    ) -> std::result::Result<(), ERPCError> {
        let mut stream = self.stream.lock().await;
        stream.shutdown().await
            .map_err(|e| ERPCError::Io(e))?;
        Ok(())
    }
}

/// Process management for starting external processes
pub struct Process {
    command: String,
    args: Vec<String>,
    port: Option<u16>,
    client: Option<Client>,
}

impl Process {
    /// Create a new process configuration
    pub fn new(
        command: impl Into<String>,
        args: Vec<impl Into<String>>,
    ) -> Self {
        Process {
            command: command.into(),
            args: args.into_iter().map(Into::into).collect(),
            port: None,
            client: None,
        }
    }

    /// Start the process and connect to it
    pub async fn start(&mut self
    ) -> std::result::Result<(), ERPCError> {
        use tokio::process::Command;
        
        let mut child = Command::new(&self.command)
            .args(&self.args)
            .stdout(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| ERPCError::ProcessError(e.to_string()))?;
        
        // Read port from stdout
        if let Some(stdout) = child.stdout.take() {
            use tokio::io::AsyncBufReadExt;
            let reader = tokio::io::BufReader::new(stdout);
            let mut lines = reader.lines();
            
            if let Some(line) = lines.next_line().await
                .map_err(|e| ERPCError::ProcessError(e.to_string()))? {
                
                let port: u16 = line.trim().parse()
                    .map_err(|_| ERPCError::ProcessError("Invalid port format".to_string()))?;
                
                self.port = Some(port);
                
                // Wait a bit for the server to start
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                
                // Connect to the server
                let client = Client::connect(format!("127.0.0.1:{}", port)).await?;
                self.client = Some(client);
                
                Ok(())
            } else {
                Err(ERPCError::ProcessError("No port received from process".to_string()))
            }
        } else {
            Err(ERPCError::ProcessError("No stdout from process".to_string()))
        }
    }

    /// Get the underlying client
    pub fn client(&self
    ) -> Option<&Client> {
        self.client.as_ref()
    }

    /// Get the port number
    pub fn port(&self
    ) -> Option<u16> {
        self.port
    }

    /// Stop the process
    pub async fn stop(&mut self
    ) -> std::result::Result<(), ERPCError> {
        if let Some(client) = &self.client {
            client.close().await?;
        }
        self.client = None;
        Ok(())
    }

    /// Delegate calls to underlying client
    pub async fn call_sync<Args, Ret>(
        &self,
        method: &str,
        args: Args,
    ) -> std::result::Result<Ret, ERPCError>
    where
        Args: Serialize,
        Ret: for<'de> Deserialize<'de>,
    {
        if let Some(client) = &self.client {
            client.call_sync(method, args).await
        } else {
            Err(ERPCError::ConnectionClosed)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_connection() {
        // This test requires a running server
        // For now, we'll test the message format
        let message = Message::new_call(1, "test", Value::from("hello"));
        let sexp = message.to_sexp().unwrap();
        assert!(sexp.contains("call"));
        assert!(sexp.contains("test"));
    }

    #[tokio::test]
    async fn test_method_query_format() {
        let message = Message::new_methods(123);
        let sexp = message.to_sexp().unwrap();
        assert!(sexp.contains("methods"));
        assert!(sexp.contains("123"));
    }
}