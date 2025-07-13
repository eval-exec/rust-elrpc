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
        
        // EPC protocol sends arguments as a single list
        if args.len() != 1 {
            return Err(rust_elrpc::EpcError::application("Expected argument list"));
        }
        
        let arg_list = &args[0];
        let actual_args = if let EpcValue::List(ref inner_args) = arg_list {
            inner_args
        } else {
            return Err(rust_elrpc::EpcError::application("Arguments must be provided as a list"));
        };

        if actual_args.len() != 2 {
            return Err(rust_elrpc::EpcError::application("add requires exactly 2 arguments"));
        }
        
        let a = actual_args[0].as_int().ok_or_else(|| {
            rust_elrpc::EpcError::application("First argument must be an integer")
        })?;
        
        let b = actual_args[1].as_int().ok_or_else(|| {
            rust_elrpc::EpcError::application("Second argument must be an integer")
        })?;
        
        let result = a + b;
        eprintln!("➕ {} + {} = {}", a, b, result);
        Ok(EpcValue::Int(result))
    });
    
    server.register_method("greet".to_string(), |args| {
        eprintln!("👋 Greet called with args: {:?}", args);
        
        let name = if args.len() >= 1 {
            if let EpcValue::List(ref arg_list) = args[0] {
                if let Some(EpcValue::String(name)) = arg_list.get(0) {
                    name.clone()
                } else {
                    "World".to_string()
                }
            } else if let Some(EpcValue::String(name)) = args.get(0) {
                name.clone()
            } else {
                "World".to_string()
            }
        } else {
            "World".to_string()
        };
        
        Ok(EpcValue::String(format!("Hello, {}! 🦀", name)))
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