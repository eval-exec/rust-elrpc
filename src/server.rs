use std::net::SocketAddr;
use std::sync::Arc;

use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use bytes::BytesMut;
use tracing::{debug, error, info, warn};
use lexpr::Value;
use serde::{Serialize, Deserialize};

use crate::error::ERPCError;
use crate::protocol::{Framer, Message};
use crate::registry::MethodRegistry;

/// Server configuration
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub bind_addr: String,
    pub max_connections: usize,
    pub request_timeout: std::time::Duration,
}

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig {
            bind_addr: "127.0.0.1:0".to_string(),
            max_connections: 100,
            request_timeout: std::time::Duration::from_secs(30),
        }
    }
}

/// EPC Server
pub struct Server {
    config: ServerConfig,
    registry: Arc<MethodRegistry>,
    listener: Option<TcpListener>,
    shutdown_tx: Option<mpsc::Sender<()>>,
    handles: Vec<JoinHandle<std::result::Result<(), ERPCError>>>,
}

impl Server {
    /// Create a new server with default configuration
    pub fn new() -> Self {
        Server::with_config(ServerConfig::default())
    }

    /// Create a new server with custom configuration
    pub fn with_config(config: ServerConfig) -> Self {
        Server {
            config,
            registry: Arc::new(MethodRegistry::new()),
            listener: None,
            shutdown_tx: None,
            handles: Vec::new(),
        }
    }

    /// Get the method registry for registering methods
    pub fn registry(&self) -> &Arc<MethodRegistry> {
        &self.registry
    }

    /// Bind to a socket address
    pub async fn bind(&mut self,
        addr: impl Into<String>
    ) -> std::result::Result<SocketAddr, ERPCError> {
        let addr = addr.into();
        debug!("Binding server to address: {}", addr);
        let listener = TcpListener::bind(&addr).await
            .map_err(|e| ERPCError::Io(e))?;
        
        let socket_addr = listener.local_addr()
            .map_err(|e| ERPCError::Io(e))?;
        
        self.listener = Some(listener);
        
        info!("EPC server successfully bound to {}", socket_addr);
        debug!("Server ready to accept connections on {}", socket_addr);
        Ok(socket_addr)
    }

    /// Get the port the server is bound to
    pub fn port(&self) -> Option<u16> {
        self.listener.as_ref()
            .and_then(|l| l.local_addr().ok())
            .map(|addr| addr.port())
    }

    /// Start serving in the background
    pub async fn serve(&mut self
    ) -> std::result::Result<(), ERPCError> {
        let listener = self.listener.take()
            .ok_or_else(|| ERPCError::ProtocolError("Server not bound".to_string()))?;
        
        let registry = self.registry.clone();
        let config = self.config.clone();
        
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel(1);
        self.shutdown_tx = Some(shutdown_tx);
        
        info!("Starting server listener on {}", listener.local_addr()?);
        
        let handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    accept_result = listener.accept() => {
                        match accept_result {
                            Ok((stream, addr)) => {
                                info!("New connection accepted from {}", addr);
                                debug!("Spawning handler for connection from {}", addr);
                                let registry = registry.clone();
                                let config = config.clone();
                                
                                tokio::spawn(async move {
                                    debug!("Starting connection handler for {}", addr);
                                    if let Err(e) = handle_connection(stream, addr, registry, config).await {
                                        error!("Connection error from {}: {}", addr, e);
                                    } else {
                                        debug!("Connection handler completed for {}", addr);
                                    }
                                });
                            }
                            Err(e) => {
                                error!("Failed to accept connection: {}", e);
                                break;
                            }
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Server received shutdown signal, stopping...");
                        break;
                    }
                }
            }
            info!("Server listener stopped");
            Ok(())
        });
        
        self.handles.push(handle);
        Ok(())
    }

    /// Stop the server gracefully
    pub async fn shutdown(&mut self
    ) -> std::result::Result<(), ERPCError> {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(()).await;
        }
        
        for handle in self.handles.drain(..) {
            let _ = handle.await;
        }
        
        info!("Server shutdown complete");
        Ok(())
    }

    /// Register a method with closure (typed arguments)
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

    /// Register a method that accepts Value directly (for maximum flexibility)
    pub async fn register_value_method(
        &self,
        name: impl Into<String>,
        func: impl Fn(Value) -> std::result::Result<Value, ERPCError> + Send + Sync + 'static,
        arg_spec: Option<impl Into<String>>,
        docstring: Option<impl Into<String>>,
    ) -> std::result::Result<(), ERPCError> {
        self.registry.register_value_method(name, func, arg_spec, docstring).await
    }

    /// Print the port number to stdout (for Emacs compatibility)
    pub fn print_port(&self
    ) -> std::result::Result<(), ERPCError> {
        if let Some(port) = self.port() {
            println!("{}", port);
            Ok(())
        } else {
            Err(ERPCError::ProtocolError("Server not bound".to_string()))
        }
    }
}

/// Handle a single client connection
async fn handle_connection(
    mut stream: TcpStream,
    addr: std::net::SocketAddr,
    registry: Arc<MethodRegistry>,
    _config: ServerConfig,
) -> std::result::Result<(), ERPCError> {
    info!("Starting to handle connection from {}", addr);
    debug!("Connection details: local_addr={}, peer_addr={}", 
           stream.local_addr().unwrap_or_else(|_| "unknown".parse().unwrap()),
           addr);
    
    let mut buffer = BytesMut::with_capacity(1024);
    let mut message_count = 0;
    
    loop {
        debug!("Waiting for data from client {}", addr);
        // Read more data
        let bytes_read = stream.read_buf(&mut buffer).await
            .map_err(|e| ERPCError::Io(e))?;
        
        debug!("Received {} bytes from client {}", bytes_read, addr);
        
        if bytes_read == 0 {
            info!("Client {} disconnected gracefully", addr);
            break;
        }
        
        debug!("Total buffer size: {} bytes for client {}", buffer.len(), addr);
        
        // Process complete messages
        while let Some(message_bytes) = Framer::extract_message(&mut buffer) {
            message_count += 1;
            debug!("Processing message #{} from client {} ({} bytes)", message_count, addr, message_bytes.len());
            
            match process_message(message_bytes, &registry).await {
                Ok(response) => {
                    debug!("Generated response for client {}: {} bytes", addr, response.len());
                    let framed = Framer::frame(response.as_bytes());
                    debug!("Sending framed response to client {}: {} bytes total", addr, framed.len());
                    stream.write_all(&framed).await
                        .map_err(|e| ERPCError::Io(e))?;
                    debug!("Successfully sent response to client {}", addr);
                }
                Err(e) => {
                    error!("Error processing message #{} from {}: {}", message_count, addr, e);
                    let error_msg = Message::new_epc_error(0, e.to_string())
                        .to_sexp()
                        .unwrap_or_else(|_| "(epc-error 0 \"Unknown error\")".to_string());
                    debug!("Sending error response to client {}: {}", addr, error_msg);
                    let framed = Framer::frame(error_msg.as_bytes());
                    let _ = stream.write_all(&framed).await;
                    break;
                }
            }
        }
        
        debug!("Processed all complete messages for client {}, remaining buffer: {} bytes", addr, buffer.len());
    }
    
    info!("Connection handler completed for client {}, processed {} messages", addr, message_count);
    Ok(())
}

/// Process a single message
async fn process_message(
    message_bytes: bytes::Bytes,
    registry: &Arc<MethodRegistry>,
) -> std::result::Result<String, ERPCError> {
    debug!("Processing message: {} bytes", message_bytes.len());
    
    let message_str = std::str::from_utf8(&message_bytes)
        .map_err(|e| ERPCError::InvalidMessageFormat(e.to_string()))?;
    
    debug!("Received message string: {}", message_str);
    
    let message = Message::from_sexp(message_str)?;
    
    debug!("Parsed message: {:?}", message);
    
    match message {
        Message::Call { uid, method, args } => {
            debug!("Processing CALL uid={}, method={}, args={:?}", uid, method, args);
            match registry.call_method(&method, args).await {
                Ok(result) => {
                    debug!("Method '{}' executed successfully, result: {:?}", method, result);
                    let response = Message::new_return(uid, result);
                    let sexp = response.to_sexp()?;
                    debug!("Returning response: {}", sexp);
                    Ok(sexp)
                }
                Err(e) => {
                    error!("Method '{}' failed: {}", method, e);
                    let response = Message::new_return_error(uid, e.to_string());
                    let sexp = response.to_sexp()?;
                    debug!("Returning error response: {}", sexp);
                    Ok(sexp)
                }
            }
        }
        Message::Methods { uid } => {
            debug!("Processing METHODS query uid={}", uid);
            let methods = registry.query_methods().await?;
            debug!("Found {} methods to return", methods.len());
            
            // Create the expected format for methods response: list of [name, arg_spec, docstring]
            let method_list = Value::list(
                methods.into_iter()
                    .map(|info| {
                        Value::list(vec![
                            Value::string(info.name),
                            info.arg_spec.map(Value::string).unwrap_or(Value::Null),
                            info.docstring.map(Value::string).unwrap_or(Value::Null),
                        ])
                    })
                    .collect::<Vec<Value>>()
            );
            
            let response = Message::new_return(uid, method_list);
            let sexp = response.to_sexp()?;
            debug!("Returning methods response: {}", sexp);
            Ok(sexp)
        }
        _ => {
            warn!("Received unexpected message type: {:?}", message);
            Err(ERPCError::InvalidMessageFormat(
                format!("Unexpected message type: {:?}", message),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_server_bind() {
        let mut server = Server::new();
        let addr = server.bind("127.0.0.1:0").await.unwrap();
        assert!(addr.port() > 0);
    }

    #[tokio::test]
    async fn test_echo_method() {
        let mut server = Server::new();
        server.bind("127.0.0.1:0").await.unwrap();
        
        server.register_method(
            "echo",
            |args: String| Ok(args),
            Some("args"),
            Some("Echo back arguments"),
        ).await.unwrap();
        
        let port = server.port().unwrap();
        server.serve().await.unwrap();
        
        // Test via TCP connection
        let mut stream = tokio::net::TcpStream::connect(format!("127.0.0.1:{}", port))
            .await
            .unwrap();
        
        let message = Message::new_call(1, "echo", Value::from("hello"));
        let message_str = message.to_sexp().unwrap();
        let framed = Framer::frame(message_str.as_bytes());
        
        stream.write_all(&framed).await.unwrap();
        
        let mut buffer = BytesMut::new();
        let bytes_read = stream.read_buf(&mut buffer).await.unwrap();
        assert!(bytes_read > 0);
        
        // Cleanup
        server.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_methods_query() {
        let mut server = Server::new();
        server.bind("127.0.0.1:0").await.unwrap();
        
        server.register_method(
            "add",
            |(a, b): (i64, i64)| Ok(a + b),
            Some("a b"),
            Some("Add two numbers"),
        ).await.unwrap();
        
        let port = server.port().unwrap();
        server.serve().await.unwrap();
        
        // Test methods query
        let mut stream = tokio::net::TcpStream::connect(format!("127.0.0.1:{}", port))
            .await
            .unwrap();
        
        let message = Message::new_methods(1);
        let message_str = message.to_sexp().unwrap();
        let framed = Framer::frame(message_str.as_bytes());
        
        stream.write_all(&framed).await.unwrap();
        
        let mut buffer = BytesMut::new();
        let bytes_read = stream.read_buf(&mut buffer).await.unwrap();
        assert!(bytes_read > 0);
        
        // Cleanup
        server.shutdown().await.unwrap();
    }
}
