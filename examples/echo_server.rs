use rust_elrpc::{EpcServer, EpcValue, EpcResult};

#[tokio::main]
async fn main() -> EpcResult<()> {
    env_logger::init();
    
    let server = EpcServer::new().await?;
    
    // Register echo method
    server.register_method("echo".to_string(), |args| {
        // Return the arguments as-is
        Ok(EpcValue::List(args.to_vec()))
    });
    
    // Register add method
    server.register_method("add".to_string(), |args| {
        if args.len() != 2 {
            return Err(rust_elrpc::EpcError::application("add requires exactly 2 arguments"));
        }
        
        let a = args[0].as_int().ok_or_else(|| {
            rust_elrpc::EpcError::application("First argument must be an integer")
        })?;
        
        let b = args[1].as_int().ok_or_else(|| {
            rust_elrpc::EpcError::application("Second argument must be an integer")
        })?;
        
        Ok(EpcValue::Int(a + b))
    });
    
    // Print port for EPC protocol
    server.print_port()?;
    
    // Start serving
    server.serve_forever().await
}