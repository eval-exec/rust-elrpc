use rust_elrpc::{EpcServer, EpcClient, EpcValue, EpcResult};
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn test_basic_echo() -> EpcResult<()> {
    // Start server
    let server = EpcServer::new().await?;
    let addr = server.local_addr()?;
    
    server.register_method("echo".to_string(), |args| {
        Ok(EpcValue::List(args.to_vec()))
    });
    
    // Spawn server task
    tokio::spawn(async move {
        let _ = server.serve_forever().await;
    });
    
    // Give server time to start
    sleep(Duration::from_millis(100)).await;
    
    // Connect client
    let client = EpcClient::connect("localhost", addr.port()).await?;
    
    // Test echo
    let result = client.call_method(
        "echo".to_string(),
        vec![EpcValue::String("test".to_string())]
    ).await?;
    
    match result {
        EpcValue::List(list) => {
            assert_eq!(list.len(), 1);
            assert_eq!(list[0], EpcValue::String("test".to_string()));
        }
        _ => panic!("Expected list result"),
    }
    
    Ok(())
}

#[tokio::test]
async fn test_method_not_found() -> EpcResult<()> {
    let server = EpcServer::new().await?;
    let addr = server.local_addr()?;
    
    tokio::spawn(async move {
        let _ = server.serve_forever().await;
    });
    
    sleep(Duration::from_millis(100)).await;
    
    let client = EpcClient::connect("localhost", addr.port()).await?;
    
    let result = client.call_method(
        "nonexistent".to_string(),
        vec![]
    ).await;
    
    assert!(result.is_err());
    
    Ok(())
}

#[tokio::test]
async fn test_query_methods() -> EpcResult<()> {
    let server = EpcServer::new().await?;
    let addr = server.local_addr()?;
    
    server.register_method("test_method".to_string(), |_| {
        Ok(EpcValue::Nil)
    });
    
    tokio::spawn(async move {
        let _ = server.serve_forever().await;
    });
    
    sleep(Duration::from_millis(100)).await;
    
    let client = EpcClient::connect("localhost", addr.port()).await?;
    
    let methods = client.query_methods().await?;
    assert!(!methods.is_empty());
    
    Ok(())
}