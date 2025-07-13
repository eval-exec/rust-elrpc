use elrpc::{Result, Server};
use lexpr::Value;
use tokio::signal;
use tracing_subscriber;

fn subtraction(args: (i64, i64)) -> Result<i64> {
    let (big, small) = args;
    Ok(big - small)
}

fn add(args: Value) -> Result<Value> {
    args.as_slice()
        .ok_or_else(|| {
            elrpc::ERPCError::InvalidArgument(format!(
                "Expected a list of numbers, found: {}",
                args
            ))
        })?
        .iter()
        .map(|value| {
            value.as_i64().ok_or_else(|| {
                elrpc::ERPCError::InvalidArgument(format!("Expected integer, found: {}", value))
            })
        })
        .sum::<Result<i64>>()
        .map(Value::from)
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Create server
    let mut server = Server::new();
    let addr = server.bind("127.0.0.1:12345").await?;

    println!("Echo server starting on port {}", addr.port());

    // Register echo method (using Value directly)
    server
        .register_value_method(
            "echo",
            |args: Value| Ok(args),
            Some("args"),
            Some("Echo back the arguments"),
        )
        .await?;

    // Register add method (using Value directly)
    server
        .register_value_method("add", add, Some("numbers..."), Some("Add list of numbers"))
        .await?;

    // Register subtraction method (typed version)
    server
        .register_method(
            "subtraction",
            subtraction,
            Some("big small"),
            Some("Subtract small from big"),
        )
        .await?;

    // Print port for Emacs compatibility
    server.print_port()?;

    // Start serving - this will run in the background
    println!(
        "Server is running on port {}. Press Ctrl+C to stop...",
        addr.port()
    );
    server.serve().await?;

    // Wait for Ctrl+C to stop the server
    signal::ctrl_c().await?;
    println!("Shutting down server...");
    server.shutdown().await?;

    Ok(())
}
