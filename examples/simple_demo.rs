use rust_elrpc::{EpcServer, EpcValue, EpcResult};

#[tokio::main]
async fn main() -> EpcResult<()> {
    // Initialize logging
    rust_elrpc::init_file_logging().expect("Failed to initialize logging");
    
    // Create a new EPC server
    let server = EpcServer::new().await?;
    
    // Print port for EPC protocol compliance - MUST be first line
    server.print_port()?;
    
    let addr = server.local_addr()?;
    
    eprintln!("🦀 Rust EPC Implementation Demo");
    eprintln!("================================");
    eprintln!("✅ Server created successfully");
    eprintln!("📍 Listening on: {}", addr);
    
    // Register some example methods
    server.register_method("echo".to_string(), |args| {
        eprintln!("📧 Echo called with args: {:?}", args);
        Ok(EpcValue::List(args.to_vec()))
    });
    
    server.register_method("add".to_string(), |args| {
        eprintln!("➕ Add called with args: {:?}", args);
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
        eprintln!("➕ {} + {} = {}", a, b, result);
        Ok(EpcValue::Int(result))
    });
    
    server.register_method("greet".to_string(), |args| {
        eprintln!("👋 Greet called with args: {:?}", args);
        if let Some(EpcValue::String(name)) = args.get(0) {
            Ok(EpcValue::String(format!("Hello, {}! 🦀", name)))
        } else {
            Ok(EpcValue::String("Hello, World! 🦀".to_string()))
        }
    });
    
    server.register_method("error_test".to_string(), |_args| {
        eprintln!("💥 Error test called - will return an error");
        Err(rust_elrpc::EpcError::application("This is a test error message"))
    });
    
    eprintln!("🎯 Registered methods: echo, add, greet, error_test");
    eprintln!("🚀 Server is ready! You can now:");
    eprintln!("   1. Connect from Emacs using (epc:start-epc \"cargo\" '(\"run\" \"--example\" \"simple_demo\"))");
    eprintln!("   2. Call methods like (epc:call-sync epc 'echo '(\"test\"))");
    eprintln!("   3. Try (epc:call-sync epc 'add '(10 20))");
    eprintln!("   4. Or (epc:call-sync epc 'greet '(\"Emacs User\"))");
    eprintln!("   5. Test errors with (epc:call-sync epc 'error_test '())");
    eprintln!("📟 Ctrl+C to stop the server");
    
    // Start serving (this will run forever)
    server.serve_forever().await
}