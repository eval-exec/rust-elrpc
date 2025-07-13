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
        let listener = TcpListener::bind(&addr).await
            .map_err(|e| ERPCError::Io(e))?;
        
        let socket_addr = listener.local_addr()
            .map_err(|e| ERPCError::Io(e))?;
        
        self.listener = Some(listener);
        
        info!("EPC server bound to {}", socket_addr);
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
        
        let handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    accept_result = listener.accept() => {
                        match accept_result {
                            Ok((stream, addr)) => {
                                debug!("New connection from {}", addr);
                                let registry = registry.clone();
                                let config = config.clone();
                                
                                tokio::spawn(async move {
                                    if let Err(e) = handle_connection(stream, addr, registry, config).await {
                                        error!("Connection error from {}: {}", addr, e);
                                    }
                                });
                            }
                            Err(e) => {
                                error!("Accept error: {}", e);
                                break;
                            }
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Server shutting down");
                        break;
                    }
                }
            }
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

    /// Register a method with closure
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
    info!("Handling connection from {}", addr);
    
    let mut buffer = BytesMut::with_capacity(1024);
    
    loop {
        // Read more data
        let bytes_read = stream.read_buf(&mut buffer).await
            .map_err(|e| ERPCError::Io(e))?;
        
        if bytes_read == 0 {
            debug!("Client {} disconnected", addr);
            break;
        }
        
        // Process complete messages
        while let Some(message_bytes) = Framer::extract_message(&mut buffer) {
            match process_message(message_bytes, &registry).await {
                Ok(response) => {
                    let framed = Framer::frame(response.as_bytes());
                    stream.write_all(&framed).await
                        .map_err(|e| ERPCError::Io(e))?;
                }
                Err(e) => {
                    warn!("Error processing message from {}: {}", addr, e);
                    let error_msg = Message::new_epc_error(0, e.to_string())
                        .to_sexp()
                        .unwrap_or_else(|_| "(epc-error 0 \"Unknown error\")".to_string());
                    let framed = Framer::frame(error_msg.as_bytes());
                    let _ = stream.write_all(&framed).await;
                    break;
                }
            }
        }
    }
    
    Ok(())
}

/// Process a single message
async fn process_message(
    message_bytes: bytes::Bytes,
    registry: &Arc<MethodRegistry>,
) -> std::result::Result<String, ERPCError> {
    let message_str = std::str::from_utf8(&message_bytes)
        .map_err(|e| ERPCError::InvalidMessageFormat(e.to_string()))?;
    
    let message = Message::from_sexp(message_str)?;
    
    match message {
        Message::Call { uid, method, args } => {
            match registry.call_method(&method, args).await {
                Ok(result) => {
                    let response = Message::new_return(uid, result);
                    response.to_sexp()
                }
                Err(e) => {
                    let response = Message::new_return_error(uid, e.to_string());
                    response.to_sexp()
                }
            }
        }
        Message::Methods { uid } => {
            let methods = registry.query_methods().await?;
            let method_list = Value::list(
                methods.into_iter()
                    .map(|info| {
                        let mut items = vec![
                            Value::symbol(info.name),
                        ];
                        
                        if let Some(args) = info.arg_spec {
                            items.push(Value::string(args));
                        } else {
                            items.push(Value::Null);
                        }
                        
                        if let Some(doc) = info.docstring {
                            items.push(Value::string(doc));
                        } else {
                            items.push(Value::Null);
                        }
                        
                        Value::list(items)
                    })
                    .collect::<Vec<Value>>()
            );
            
            Message::new_return(uid, method_list).to_sexp()
        }
        _ => {
            Err(ERPCError::InvalidMessageFormat(
                "Unexpected message type".to_string(),
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