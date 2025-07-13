use elrpc::{Server, Result};
use tokio::signal;
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
    // Create server
    let mut server = Server::new();
    let addr = server.bind("127.0.0.1:12345").await?;
    
    println!("Echo server starting on port {}", addr.port());
    
    // Register echo method
    server.register_method(
        "echo",
        |args: String| Ok(args),
        Some("args"),
        Some("Echo back the arguments"),
    ).await?;
    
    // Register add method
    server.register_method(
        "add",
        |(a, b): (i64, i64)| Ok(a + b),
        Some("a b"),
        Some("Add two numbers"),
    ).await?;
    
    // Print port for Emacs compatibility
    server.print_port()?;
    
    // Start serving - this will run in the background
    println!("Server is running on port {}. Press Ctrl+C to stop...", addr.port());
    server.serve().await?;
    
    // Wait for Ctrl+C to stop the server
    signal::ctrl_c().await?;
    println!("Shutting down server...");
    server.shutdown().await?;
    
    Ok(())
}
