use rust_elrpc::{EpcServer, EpcClient, EpcValue};

#[tokio::main]
async fn main() {
    rust_elrpc::init_file_logging().expect("Failed to init logging");
    
    println!("Starting debug test...");
    
    // Start server
    let server = EpcServer::new().await.unwrap();
    let addr = server.local_addr().unwrap();
    println!("Server bound to: {}", addr);
    
    server.register_method("echo".to_string(), |args| {
        println!("Echo called with: {:?}", args);
        Ok(EpcValue::List(args.to_vec()))
    });
    
    // Spawn server task
    tokio::spawn(async move {
        println!("Server starting serve_forever...");
        let _ = server.serve_forever().await;
    });
    
    // Give server time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
    
    println!("Connecting client to {}...", addr);
    
    // Connect client
    match EpcClient::connect("127.0.0.1", addr.port()).await {
        Ok(client) => {
            println!("Client connected successfully");
            
            println!("Calling echo method...");
            match client.call_method(
                "echo".to_string(),
                vec![EpcValue::String("test".to_string())]
            ).await {
                Ok(result) => println!("Got result: {:?}", result),
                Err(e) => println!("Call failed: {}", e),
            }
        }
        Err(e) => println!("Connection failed: {}", e),
    }
    
    println!("Test completed");
}