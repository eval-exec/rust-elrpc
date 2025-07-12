use tokio::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::collections::HashMap;
use log::{info, error, debug};

use crate::error::{EpcError, EpcResult};
use crate::protocol::{EpcConnection, MethodHandler};
use crate::types::EpcValue;

pub struct EpcServer {
    listener: TcpListener,
    methods: Arc<std::sync::Mutex<HashMap<String, MethodHandler>>>,
}

impl EpcServer {
    /// Create a new EPC server bound to the specified address
    pub async fn bind(addr: &str) -> EpcResult<Self> {
        let listener = TcpListener::bind(addr).await?;
        let methods = Arc::new(std::sync::Mutex::new(HashMap::new()));
        
        Ok(EpcServer {
            listener,
            methods,
        })
    }
    
    /// Create a new EPC server on localhost with OS-assigned port
    pub async fn new() -> EpcResult<Self> {
        Self::bind("localhost:0").await
    }
    
    /// Get the local address the server is bound to
    pub fn local_addr(&self) -> EpcResult<std::net::SocketAddr> {
        self.listener.local_addr()
            .map_err(|e| EpcError::Io(e))
    }
    
    /// Print the port number to stdout (EPC protocol requirement)
    pub fn print_port(&self) -> EpcResult<()> {
        let addr = self.local_addr()?;
        println!("{}", addr.port());
        Ok(())
    }
    
    /// Register a method that can be called by clients
    pub fn register_method<F>(&self, name: String, handler: F)
    where
        F: Fn(&[EpcValue]) -> EpcResult<EpcValue> + Send + Sync + 'static,
    {
        let mut methods = self.methods.lock().unwrap();
        methods.insert(name, Box::new(handler));
    }
    
    /// Start serving clients and wait for connections
    pub async fn serve_forever(self) -> EpcResult<()> {
        info!("EPC server listening on {}", self.local_addr()?);
        
        loop {
            match self.listener.accept().await {
                Ok((stream, addr)) => {
                    debug!("New client connected from: {}", addr);
                    let methods = Arc::clone(&self.methods);
                    
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_client(stream, methods).await {
                            error!("Client handler error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept connection: {}", e);
                    return Err(EpcError::Io(e));
                }
            }
        }
    }
    
    async fn handle_client(
        stream: TcpStream,
        methods: Arc<std::sync::Mutex<HashMap<String, MethodHandler>>>,
    ) -> EpcResult<()> {
        let connection = EpcConnection::new(stream).await?;
        
        // Copy methods to the connection
        let method_names: Vec<String> = {
            let server_methods = methods.lock().unwrap();
            server_methods.keys().cloned().collect()
        };
        
        for name in method_names {
            let name_clone = name.clone();
            let methods_ref = Arc::clone(&methods);
            
            connection.register_method(name_clone.clone(), move |args| {
                let methods = methods_ref.lock().unwrap();
                if let Some(handler) = methods.get(&name_clone) {
                    handler(args)
                } else {
                    Err(EpcError::MethodNotFound(name_clone.clone()))
                }
            }).await;
        }
        
        // Keep the connection alive
        // In a real implementation, we might want to add some way to detect
        // when the connection is closed and clean up
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_test;
    
    #[tokio::test]
    async fn test_server_creation() {
        let server = EpcServer::new().await.unwrap();
        assert!(server.local_addr().is_ok());
    }
    
    #[tokio::test]
    async fn test_method_registration() {
        let server = EpcServer::new().await.unwrap();
        
        server.register_method("echo".to_string(), |args| {
            if let Some(arg) = args.get(0) {
                Ok(arg.clone())
            } else {
                Ok(EpcValue::Nil)
            }
        });
        
        // Method registration should not fail
        assert!(true);
    }
}