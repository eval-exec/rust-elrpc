use rust_elrpc::{EpcServer, EpcValue, EpcResult};

#[tokio::main]
async fn main() -> EpcResult<()> {
    // Initialize logging
    env_logger::init();
    
    println!("🦀 Rust EPC Implementation Demo");
    println!("================================");
    
    // Create a new EPC server
    let server = EpcServer::new().await?;
    let addr = server.local_addr()?;
    
    println!("✅ Server created successfully");
    println!("📍 Listening on: {}", addr);
    
    // Register some example methods
    server.register_method("echo".to_string(), |args| {
        println!("📧 Echo called with args: {:?}", args);
        Ok(EpcValue::List(args.to_vec()))
    });
    
    server.register_method("add".to_string(), |args| {
        println!("➕ Add called with args: {:?}", args);
        if args.len() != 2 {
            return Err(rust_elrpc::EpcError::application("add requires exactly 2 arguments"));
        }
        
        let a = args[0].as_int().ok_or_else(|| {
            rust_elrpc::EpcError::application("First argument must be an integer")
        })?;
        
        let b = args[1].as_int().ok_or_else(|| {
            rust_elrpc::EpcError::application("Second argument must be an integer")
        })?;
        
        let result = a + b;
        println!("➕ {} + {} = {}", a, b, result);
        Ok(EpcValue::Int(result))
    });
    
    server.register_method("greet".to_string(), |args| {
        println!("👋 Greet called with args: {:?}", args);
        if let Some(EpcValue::String(name)) = args.get(0) {
            Ok(EpcValue::String(format!("Hello, {}! 🦀", name)))
        } else {
            Ok(EpcValue::String("Hello, World! 🦀".to_string()))
        }
    });
    
    println!("🎯 Registered methods: echo, add, greet");
    
    // Print port for EPC protocol compliance
    server.print_port()?;
    
    println!("🚀 Server is ready! You can now:");
    println!("   1. Connect from Emacs using (epc:start-epc \"cargo\" '(\"run\" \"--example\" \"simple_demo\"))");
    println!("   2. Call methods like (epc:call-sync epc 'echo '(\"test\"))");
    println!("   3. Try (epc:call-sync epc 'add '(10 20))");
    println!("   4. Or (epc:call-sync epc 'greet '(\"Emacs User\"))");
    println!("📟 Ctrl+C to stop the server");
    
    // Start serving (this will run forever)
    server.serve_forever().await
}