use rust_elrpc::{EpcServer, EpcClient, EpcValue, EpcResult};
use tokio::task::JoinHandle;

struct AbortOnDrop(JoinHandle<()>);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}
use tokio::time::{sleep, Duration};

/// Test rapid sequential calls (based on Node.js bench tests)
#[tokio::test]
async fn test_rapid_calls() -> EpcResult<()> {
    let server = EpcServer::new().await?;
    let addr = server.local_addr()?;
    
    server.register_method("add".to_string(), |args| {
        if args.len() != 2 {
            return Ok(EpcValue::Int(0));
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
    
    // Make many rapid calls
    for i in 0..50 {
        let result = client.call_method(
            "add".to_string(),
            vec![EpcValue::Int(i), EpcValue::Int(i + 1)]
        ).await?;
        
        assert_eq!(result, EpcValue::Int(i + i + 1));
    }
    
    Ok(())
}

/// Test with very large data structures
#[tokio::test]
async fn test_large_data_structures() -> EpcResult<()> {
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
    
    // Create large list
    let large_list = EpcValue::List(
        (0..1000)
            .map(|i| EpcValue::Int(i))
            .collect()
    );
    
    let result = client.call_method(
        "echo".to_string(),
        vec![large_list.clone()]
    ).await?;
    
    assert_eq!(result, large_list);
    
    Ok(())
}

/// Test deeply nested structures
#[tokio::test]
async fn test_deep_nesting() -> EpcResult<()> {
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
    
    // Create deeply nested structure
    let mut nested = EpcValue::Int(42);
    for i in 0..100 {
        nested = EpcValue::List(vec![
            EpcValue::String(format!("level_{}", i)),
            nested
        ]);
    }
    
    let result = client.call_method(
        "echo".to_string(),
        vec![nested.clone()]
    ).await?;
    
    assert_eq!(result, nested);
    
    Ok(())
}

/// Test multiple clients connecting to same server
#[tokio::test]
async fn test_multiple_clients() -> EpcResult<()> {
    let server = EpcServer::new().await?;
    let addr = server.local_addr()?;
    
    server.register_method("get_client_id".to_string(), |args| {
        // Simple method that returns a unique response per call
        static mut COUNTER: i64 = 0;
        unsafe {
            COUNTER += 1;
            Ok(EpcValue::Int(COUNTER))
        }
    });
    
    let server_handle = tokio::spawn(async move {
        let _ = server.serve_forever().await;
    });
    let _guard = AbortOnDrop(server_handle);
    
    sleep(Duration::from_millis(100)).await;
    
    // Create multiple clients
    let client1 = EpcClient::connect("127.0.0.1", addr.port()).await?;
    let client2 = EpcClient::connect("127.0.0.1", addr.port()).await?;
    let client3 = EpcClient::connect("127.0.0.1", addr.port()).await?;
    
    // Each client should get a different response
    let result1 = client1.call_method(
        "get_client_id".to_string(),
        vec![]
    ).await?;
    
    let result2 = client2.call_method(
        "get_client_id".to_string(),
        vec![]
    ).await?;
    
    let result3 = client3.call_method(
        "get_client_id".to_string(),
        vec![]
    ).await?;
    
    // Results should be different (indicating separate connections)
    assert_ne!(result1, result2);
    assert_ne!(result2, result3);
    assert_ne!(result1, result3);
    
    Ok(())
}

/// Test server with many methods
#[tokio::test]
async fn test_many_methods() -> EpcResult<()> {
    let server = EpcServer::new().await?;
    let addr = server.local_addr()?;
    
    // Register many methods
    for i in 0..100 {
        let method_name = format!("method_{}", i);
        let expected_result = i * 2;
        
        server.register_method(method_name, move |_args| {
            Ok(EpcValue::Int(expected_result))
        });
    }
    
    let server_handle = tokio::spawn(async move {
        let _ = server.serve_forever().await;
    });
    let _guard = AbortOnDrop(server_handle);
    
    sleep(Duration::from_millis(100)).await;
    
    let client = EpcClient::connect("127.0.0.1", addr.port()).await?;
    
    // Test calling various methods
    for i in [0, 25, 50, 75, 99] {
        let method_name = format!("method_{}", i);
        let result = client.call_method(
            method_name,
            vec![]
        ).await?;
        
        assert_eq!(result, EpcValue::Int(i * 2));
    }
    
    // Test query methods returns all our methods
    let methods = client.query_methods().await?;
    assert!(methods.len() >= 100);
    
    Ok(())
}

/// Test edge case with empty strings and whitespace
#[tokio::test]
async fn test_edge_case_strings() -> EpcResult<()> {
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
    
    // Test edge case strings
    let edge_cases = vec![
        "",                           // Empty string
        " ",                          // Single space
        "   ",                        // Multiple spaces
        "\n",                         // Newline
        "\t",                         // Tab
        "\r\n",                       // Windows line ending
        "\"",                         // Quote
        "\\",                         // Backslash
        "()",                         // Parentheses
        "[]",                         // Brackets
        "{}",                         // Braces
        "null",                       // Keyword-like strings
        "nil",
        "true",
        "false",
    ];
    
    for test_str in edge_cases {
        let result = client.call_method(
            "echo".to_string(),
            vec![EpcValue::String(test_str.to_string())]
        ).await?;
        
        assert_eq!(result, EpcValue::String(test_str.to_string()));
    }
    
    Ok(())
}