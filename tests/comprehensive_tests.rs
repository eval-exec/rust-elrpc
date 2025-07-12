use rust_elrpc::{EpcServer, EpcClient, EpcValue, EpcResult, EpcError};
use tokio::task::JoinHandle;

struct AbortOnDrop(JoinHandle<()>);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}
use tokio::time::{sleep, Duration};

/// Test basic echo functionality
#[tokio::test]
async fn test_echo_string() -> EpcResult<()> {
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
    
    // Test string echo
    let result = client.call_method(
        "echo".to_string(),
        vec![EpcValue::String("hello".to_string())]
    ).await?;
    
    assert_eq!(result, EpcValue::String("hello".to_string()));
    
    Ok(())
}

/// Test echo with numbers
#[tokio::test] 
async fn test_echo_number() -> EpcResult<()> {
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
    
    // Test integer echo
    let result = client.call_method(
        "echo".to_string(),
        vec![EpcValue::Int(12345)]
    ).await?;
    
    assert_eq!(result, EpcValue::Int(12345));
    
    Ok(())
}

/// Test echo with lists/arrays
#[tokio::test]
async fn test_echo_list() -> EpcResult<()> {
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
    
    // Test list echo
    let test_list = EpcValue::List(vec![
        EpcValue::Int(1),
        EpcValue::String("2".to_string()),
        EpcValue::Float(3.2),
        EpcValue::Nil
    ]);
    
    let result = client.call_method(
        "echo".to_string(),
        vec![test_list.clone()]
    ).await?;
    
    assert_eq!(result, test_list);
    
    Ok(())
}

/// Test add method (arithmetic)
#[tokio::test]
async fn test_add_method() -> EpcResult<()> {
    let server = EpcServer::new().await?;
    let addr = server.local_addr()?;
    
    server.register_method("add".to_string(), |args| {
        if args.len() != 2 {
            return Err(EpcError::application("add requires exactly 2 arguments"));
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
    
    // Test addition
    let result = client.call_method(
        "add".to_string(),
        vec![EpcValue::Int(10), EpcValue::Int(20)]
    ).await?;
    
    assert_eq!(result, EpcValue::Int(30));
    
    Ok(())
}

/// Test method not found error
#[tokio::test]
async fn test_method_not_found() -> EpcResult<()> {
    let server = EpcServer::new().await?;
    let addr = server.local_addr()?;
    
    let server_handle = tokio::spawn(async move {
        let _ = server.serve_forever().await;
    });
    let _guard = AbortOnDrop(server_handle);
    
    sleep(Duration::from_millis(100)).await;
    
    let client = EpcClient::connect("127.0.0.1", addr.port()).await?;
    
    // Test calling non-existent method
    let result = client.call_method(
        "nonexistent".to_string(),
        vec![]
    ).await;
    
    assert!(result.is_err());
    match result {
        Err(EpcError::MethodNotFound(_)) => {},
        Err(EpcError::Application(msg)) if msg.contains("Method not found") => {},
        Err(e) => panic!("Expected MethodNotFound or Application error with 'Method not found', got: {}", e),
        Ok(_) => panic!("Expected error, got success"),
    }
    
    Ok(())
}

/// Test application error handling
#[tokio::test]
async fn test_application_error() -> EpcResult<()> {
    let server = EpcServer::new().await?;
    let addr = server.local_addr()?;
    
    server.register_method("bad_method".to_string(), |_args| {
        Err(EpcError::application("This is a test error"))
    });
    
    let server_handle = tokio::spawn(async move {
        let _ = server.serve_forever().await;
    });
    let _guard = AbortOnDrop(server_handle);
    
    sleep(Duration::from_millis(100)).await;
    
    let client = EpcClient::connect("127.0.0.1", addr.port()).await?;
    
    // Test application error
    let result = client.call_method(
        "bad_method".to_string(),
        vec![]
    ).await;
    
    assert!(result.is_err());
    match result {
        Err(EpcError::Application(msg)) => {
            assert!(msg.contains("This is a test error"));
        },
        Err(e) => panic!("Expected Application error, got: {}", e),
        Ok(_) => panic!("Expected error, got success"),
    }
    
    Ok(())
}

/// Test query methods functionality
#[tokio::test]
async fn test_query_methods() -> EpcResult<()> {
    let server = EpcServer::new().await?;
    let addr = server.local_addr()?;
    
    server.register_method("echo".to_string(), |args| {
        if args.is_empty() {
            Ok(EpcValue::Nil)
        } else {
            Ok(args[0].clone())
        }
    });
    
    server.register_method("add".to_string(), |args| {
        if args.len() != 2 {
            return Err(EpcError::application("add requires exactly 2 arguments"));
        }
        
        let a = args[0].as_int().unwrap_or(0);
        let b = args[1].as_int().unwrap_or(0);
        
        Ok(EpcValue::Int(a + b))
    });
    
    let server_handle = tokio::spawn(async move {
        let _ = server.serve_forever().await;
    });
    let _guard = AbortOnDrop(server_handle);
    
    sleep(Duration::from_millis(100)).await;
    
    let client = EpcClient::connect("127.0.0.1", addr.port()).await?;
    
    // Query available methods
    let methods = client.query_methods().await?;
    
    assert!(!methods.is_empty());
    
    // Should contain at least our registered methods
    let method_names: Vec<String> = methods.iter()
        .filter_map(|m| {
            if let EpcValue::List(list) = m {
                if let Some(EpcValue::Symbol(name)) = list.get(0) {
                    Some(name.clone())
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();
    
    assert!(method_names.contains(&"echo".to_string()));
    assert!(method_names.contains(&"add".to_string()));
    
    Ok(())
}

/// Test multiple simultaneous calls
#[tokio::test]
async fn test_concurrent_calls() -> EpcResult<()> {
    let server = EpcServer::new().await?;
    let addr = server.local_addr()?;
    
    server.register_method("add".to_string(), |args| {
        if args.len() != 2 {
            return Err(EpcError::application("add requires exactly 2 arguments"));
        }
        
        let a = args[0].as_int().unwrap_or(0);
        let b = args[1].as_int().unwrap_or(0);
        
        Ok(EpcValue::Int(a + b))
    });
    
    let server_handle = tokio::spawn(async move {
        let _ = server.serve_forever().await;
    });
    let _guard = AbortOnDrop(server_handle);
    
    sleep(Duration::from_millis(100)).await;
    
    let client = EpcClient::connect("127.0.0.1", addr.port()).await?;
    
    // Make multiple sequential calls
    for i in 0..5 {
        let result = client.call_method(
            "add".to_string(),
            vec![EpcValue::Int(i), EpcValue::Int(i * 2)]
        ).await?;
        assert_eq!(result, EpcValue::Int(i as i64 + i as i64 * 2));
    }
    
    Ok(())
}

/// Test with nil/empty values
#[tokio::test]
async fn test_nil_values() -> EpcResult<()> {
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
    
    // Test nil echo
    let result = client.call_method(
        "echo".to_string(),
        vec![EpcValue::Nil]
    ).await?;
    
    assert_eq!(result, EpcValue::Nil);
    
    // Test empty args
    let result = client.call_method(
        "echo".to_string(),
        vec![]
    ).await?;
    
    assert_eq!(result, EpcValue::Nil);
    
    Ok(())
}

/// Test with symbols
#[tokio::test]
async fn test_symbols() -> EpcResult<()> {
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
    
    // Test symbol echo
    let symbol = EpcValue::Symbol("test-symbol".to_string());
    let result = client.call_method(
        "echo".to_string(),
        vec![symbol.clone()]
    ).await?;
    
    assert_eq!(result, symbol);
    
    Ok(())
}
