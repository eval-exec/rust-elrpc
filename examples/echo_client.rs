use elrpc::{Client, Result};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    // Connect to server (replace with actual port)
    let client = Client::connect("127.0.0.1:12345").await?;
    
    // Test echo method
    let result: String = client.call_sync("echo", "Hello from Rust!").await?;
    println!("Echo result: {}", result);
    
    // Test add method
    let sum: i64 = client.call_sync("add", (5, 3)).await?;
    println!("5 + 3 = {}", sum);
    
    // Query available methods
    let methods = client.query_methods().await?;
    println!("Available methods:");
    for method in methods {
        println!("  {}", method);
    }
    
    // Close connection
    client.close().await?;
    
    Ok(())
}