use rust_elrpc::{EpcServer, EpcClient, EpcValue, EpcResult, EpcError};
use tokio::task::JoinHandle;

struct AbortOnDrop(JoinHandle<()>);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}
use tokio::time::{sleep, Duration};

/// Test large message handling (based on Python EPC tests)
#[tokio::test]
async fn test_large_message() -> EpcResult<()> {
    let server = EpcServer::new().await?;
    let addr = server.local_addr()?;
    
    server.register_method("echo".to_string(), |args| {
        if args.is_empty() {
            Ok(EpcValue::Nil)
        } else {
            Ok(args[0].clone())
        }
    });
    
    let server_handle = tokio::spawn(async move {
        let _ = server.serve_forever().await;
    });
    let _guard = AbortOnDrop(server_handle);
    
    sleep(Duration::from_millis(100)).await;
    
    let client = EpcClient::connect("127.0.0.1", addr.port()).await?;
    
    // Create a large string (similar to Python test)
    let large_string = "a".repeat(10000);
    
    let result = client.call_method(
        "echo".to_string(),
        vec![EpcValue::String(large_string.clone())]
    ).await?;
    
    assert_eq!(result, EpcValue::String(large_string));
    
    Ok(())
}

/// Test nested list structures
#[tokio::test]
async fn test_nested_structures() -> EpcResult<()> {
    let server = EpcServer::new().await?;
    let addr = server.local_addr()?;
    
    server.register_method("echo".to_string(), |args| {
        if args.is_empty() {
            Ok(EpcValue::Nil)
        } else {
            Ok(args[0].clone())
        }
    });
    
    let server_handle = tokio::spawn(async move {
        let _ = server.serve_forever().await;
    });
    let _guard = AbortOnDrop(server_handle);
    
    sleep(Duration::from_millis(100)).await;
    
    let client = EpcClient::connect("127.0.0.1", addr.port()).await?;
    
    // Create nested structure
    let nested = EpcValue::List(vec![
        EpcValue::String("outer".to_string()),
        EpcValue::List(vec![
            EpcValue::String("inner1".to_string()),
            EpcValue::List(vec![
                EpcValue::String("deep".to_string()),
                EpcValue::Int(42)
            ])
        ]),
        EpcValue::Int(100)
    ]);
    
    let result = client.call_method(
        "echo".to_string(),
        vec![nested.clone()]
    ).await?;
    
    assert_eq!(result, nested);
    
    Ok(())
}

/// Test special characters and unicode
#[tokio::test]
async fn test_unicode_strings() -> EpcResult<()> {
    let server = EpcServer::new().await?;
    let addr = server.local_addr()?;
    
    server.register_method("echo".to_string(), |args| {
        if args.is_empty() {
            Ok(EpcValue::Nil)
        } else {
            Ok(args[0].clone())
        }
    });
    
    let server_handle = tokio::spawn(async move {
        let _ = server.serve_forever().await;
    });
    let _guard = AbortOnDrop(server_handle);
    
    sleep(Duration::from_millis(100)).await;
    
    let client = EpcClient::connect("127.0.0.1", addr.port()).await?;
    
    // Test various unicode strings
    let test_strings = vec![
        "Hello 世界",
        "🦀 Rust EPC",
        "Special chars: \"'()[]{}",
        "Newlines\nand\ttabs",
        "Empty: ",
    ];
    
    for test_str in test_strings {
        let result = client.call_method(
            "echo".to_string(),
            vec![EpcValue::String(test_str.to_string())]
        ).await?;
        
        assert_eq!(result, EpcValue::String(test_str.to_string()));
    }
    
    Ok(())
}

/// Test argument validation
#[tokio::test]
async fn test_argument_validation() -> EpcResult<()> {
    let server = EpcServer::new().await?;
    let addr = server.local_addr()?;
    
    server.register_method("strict_add".to_string(), |args| {
        if args.len() != 2 {
            return Err(EpcError::application(&format!(
                "strict_add requires exactly 2 arguments, got {}", 
                args.len()
            )));
        }
        
        let a = args[0].as_int().ok_or_else(|| {
            EpcError::application("First argument must be an integer")
        })?;
        
        let b = args[1].as_int().ok_or_else(|| {
            EpcError::application("Second argument must be an integer")
        })?;
        
        Ok(EpcValue::Int(a + b))
    });
    
    let server_handle = tokio::spawn(async move {
        let _ = server.serve_forever().await;
    });
    let _guard = AbortOnDrop(server_handle);
    
    sleep(Duration::from_millis(100)).await;
    
    let client = EpcClient::connect("127.0.0.1", addr.port()).await?;
    
    // Test wrong number of arguments
    let result = client.call_method(
        "strict_add".to_string(),
        vec![EpcValue::Int(1)]
    ).await;
    
    assert!(result.is_err());
    
    // Test wrong argument types
    let result = client.call_method(
        "strict_add".to_string(),
        vec![EpcValue::String("not a number".to_string()), EpcValue::Int(2)]
    ).await;
    
    assert!(result.is_err());
    
    // Test correct usage
    let result = client.call_method(
        "strict_add".to_string(),
        vec![EpcValue::Int(5), EpcValue::Int(7)]
    ).await?;
    
    assert_eq!(result, EpcValue::Int(12));
    
    Ok(())
}

/// Test float/double precision
#[tokio::test]
async fn test_float_precision() -> EpcResult<()> {
    let server = EpcServer::new().await?;
    let addr = server.local_addr()?;
    
    server.register_method("echo".to_string(), |args| {
        if args.is_empty() {
            Ok(EpcValue::Nil)
        } else {
            Ok(args[0].clone())
        }
    });
    
    let server_handle = tokio::spawn(async move {
        let _ = server.serve_forever().await;
    });
    let _guard = AbortOnDrop(server_handle);
    
    sleep(Duration::from_millis(100)).await;
    
    let client = EpcClient::connect("127.0.0.1", addr.port()).await?;
    
    // Test float precision
    let test_float = 3.141592653589793;
    
    let result = client.call_method(
        "echo".to_string(),
        vec![EpcValue::Float(test_float)]
    ).await?;
    
    if let EpcValue::Float(returned_float) = result {
        // Allow for some floating point precision loss
        assert!((returned_float - test_float).abs() < 1e-10);
    } else {
        panic!("Expected float, got: {:?}", result);
    }
    
    Ok(())
}

/// Test boolean handling (mapped to nil/t in EPC)
#[tokio::test]
async fn test_boolean_handling() -> EpcResult<()> {
    let server = EpcServer::new().await?;
    let addr = server.local_addr()?;
    
    server.register_method("echo".to_string(), |args| {
        if args.is_empty() {
            Ok(EpcValue::Nil)
        } else {
            Ok(args[0].clone())
        }
    });
    
    server.register_method("is_true".to_string(), |args| {
        if args.is_empty() {
            Ok(EpcValue::Nil)
        } else {
            match &args[0] {
                EpcValue::Nil => Ok(EpcValue::Nil),
                _ => Ok(EpcValue::Symbol("t".to_string())),
            }
        }
    });
    
    let server_handle = tokio::spawn(async move {
        let _ = server.serve_forever().await;
    });
    let _guard = AbortOnDrop(server_handle);
    
    sleep(Duration::from_millis(100)).await;
    
    let client = EpcClient::connect("127.0.0.1", addr.port()).await?;
    
    // Test true (represented as symbol 't')
    let result = client.call_method(
        "is_true".to_string(),
        vec![EpcValue::Int(1)]
    ).await?;
    
    assert_eq!(result, EpcValue::Symbol("t".to_string()));
    
    // Test false (represented as nil)
    let result = client.call_method(
        "is_true".to_string(),
        vec![EpcValue::Nil]
    ).await?;
    
    assert_eq!(result, EpcValue::Nil);
    
    Ok(())
}

/// Test method registration edge cases
#[tokio::test]
async fn test_method_registration() -> EpcResult<()> {
    let server = EpcServer::new().await?;
    let addr = server.local_addr()?;
    
    // Register method with dashes in name (common in Lisp)
    server.register_method("kebab-case-method".to_string(), |args| {
        Ok(EpcValue::String("kebab".to_string()))
    });
    
    // Register method with underscores
    server.register_method("snake_case_method".to_string(), |args| {
        Ok(EpcValue::String("snake".to_string()))
    });
    
    // Register method with numbers
    server.register_method("method123".to_string(), |args| {
        Ok(EpcValue::String("numbers".to_string()))
    });
    
    let server_handle = tokio::spawn(async move {
        let _ = server.serve_forever().await;
    });
    let _guard = AbortOnDrop(server_handle);
    
    sleep(Duration::from_millis(100)).await;
    
    let client = EpcClient::connect("127.0.0.1", addr.port()).await?;
    
    // Test calling methods with different naming conventions
    let result = client.call_method(
        "kebab-case-method".to_string(),
        vec![]
    ).await?;
    assert_eq!(result, EpcValue::String("kebab".to_string()));
    
    let result = client.call_method(
        "snake_case_method".to_string(),
        vec![]
    ).await?;
    assert_eq!(result, EpcValue::String("snake".to_string()));
    
    let result = client.call_method(
        "method123".to_string(),
        vec![]
    ).await?;
    assert_eq!(result, EpcValue::String("numbers".to_string()));
    
    Ok(())
}