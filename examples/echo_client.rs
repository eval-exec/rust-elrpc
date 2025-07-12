use rust_elrpc::{EpcClient, EpcValue, EpcResult};

#[tokio::main]
async fn main() -> EpcResult<()> {
    rust_elrpc::init_file_logging().expect("Failed to initialize logging");
    
    // Connect to a server process (you need to start echo_server first)
    let client = EpcClient::connect("localhost", 12345).await?;
    
    // Call echo method
    let result = client.call_method(
        "echo".to_string(),
        vec![EpcValue::String("Hello, EPC!".to_string())]
    ).await?;
    
    println!("Echo result: {:?}", result);
    
    // Call add method
    let sum = client.call_method(
        "add".to_string(),
        vec![EpcValue::Int(10), EpcValue::Int(20)]
    ).await?;
    
    println!("Add result: {:?}", sum);
    
    // Query available methods
    let methods = client.query_methods().await?;
    println!("Available methods: {:?}", methods);
    
    Ok(())
}