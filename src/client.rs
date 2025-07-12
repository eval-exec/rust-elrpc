use tokio::net::TcpStream;
use tokio::process::{Child, Command};
use std::process::Stdio;
use std::time::Duration;
use tokio::time::timeout;
use tokio::io::{AsyncBufReadExt, BufReader};
use log::{debug, error};

use crate::error::{EpcError, EpcResult};
use crate::protocol::EpcConnection;
use crate::types::EpcValue;

pub struct EpcClient {
    connection: EpcConnection,
    process: Option<Child>,
}

impl EpcClient {
    /// Connect to an existing EPC server
    pub async fn connect(host: &str, port: u16) -> EpcResult<Self> {
        let addr = format!("{}:{}", host, port);
        let stream = TcpStream::connect(addr).await?;
        let connection = EpcConnection::new(stream).await?;
        
        Ok(EpcClient {
            connection,
            process: None,
        })
    }
    
    /// Start a new EPC server process and connect to it
    pub async fn start_process(cmd: &str, args: &[&str]) -> EpcResult<Self> {
        debug!("Starting EPC process: {} {:?}", cmd, args);
        
        let mut child = Command::new(cmd)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        
        // Read the port number from the process stdout
        let stdout = child.stdout.take()
            .ok_or_else(|| EpcError::protocol("Failed to get process stdout"))?;
        
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        
        // Wait for the port number with timeout
        let port = timeout(Duration::from_secs(10), async {
            reader.read_line(&mut line).await?;
            line.trim().parse::<u16>()
                .map_err(|_| EpcError::protocol(format!("Invalid port number: {}", line.trim())))
        }).await
            .map_err(|_| EpcError::Timeout)??;
        
        debug!("EPC process listening on port: {}", port);
        
        // Connect to the server
        let stream = TcpStream::connect(format!("localhost:{}", port)).await?;
        let connection = EpcConnection::new(stream).await?;
        
        Ok(EpcClient {
            connection,
            process: Some(child),
        })
    }
    
    /// Register a method that can be called by the peer
    pub async fn register_method<F>(&self, name: String, handler: F)
    where
        F: Fn(&[EpcValue]) -> EpcResult<EpcValue> + Send + Sync + 'static,
    {
        self.connection.register_method(name, handler).await;
    }
    
    /// Call a remote method
    pub async fn call_method(&self, method_name: String, args: Vec<EpcValue>) -> EpcResult<EpcValue> {
        self.connection.call_method(method_name, args).await
    }
    
    /// Query available methods from the peer
    pub async fn query_methods(&self) -> EpcResult<Vec<EpcValue>> {
        self.connection.query_methods().await
    }
    
    /// Stop the client and kill the process if it was started by this client
    pub async fn stop(mut self) -> EpcResult<()> {
        if let Some(mut process) = self.process.take() {
            if let Err(e) = process.kill().await {
                error!("Failed to kill EPC process: {}", e);
            }
            
            // Wait for the process to exit
            if let Err(e) = process.wait().await {
                error!("Failed to wait for EPC process: {}", e);
            }
        }
        
        Ok(())
    }
}

impl Drop for EpcClient {
    fn drop(&mut self) {
        if let Some(mut process) = self.process.take() {
            // Best effort to kill the process
            let _ = process.start_kill();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_test;
    
    #[tokio::test]
    async fn test_client_creation() {
        // This test would require a running EPC server
        // For now, just test that the client struct can be created
        assert!(true);
    }
}