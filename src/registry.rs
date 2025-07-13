use std::collections::HashMap;
use std::sync::Arc;
use std::fmt;

use lexpr::Value;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::error::ERPCError;

/// Method metadata for introspection
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MethodInfo {
    pub name: String,
    pub arg_spec: Option<String>,
    pub docstring: Option<String>,
}

impl MethodInfo {
    pub fn new(
        name: impl Into<String>,
        arg_spec: Option<impl Into<String>>,
        docstring: Option<impl Into<String>>,
    ) -> Self {
        MethodInfo {
            name: name.into(),
            arg_spec: arg_spec.map(Into::into),
            docstring: docstring.map(Into::into),
        }
    }
}

impl fmt::Display for MethodInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)?;
        if let Some(args) = &self.arg_spec {
            write!(f, " {}", args)?;
        }
        if let Some(doc) = &self.docstring {
            write!(f, " - {}", doc)?;
        }
        Ok(())
    }
}

/// Trait for methods that can be registered
#[async_trait::async_trait]
pub trait MethodHandler: Send + Sync {
    async fn call(&self,
        args: Value,
    ) -> std::result::Result<Value, ERPCError>;
    
    fn info(&self) -> MethodInfo;
}

/// Type-erased method handler using closures
pub struct ClosureHandler {
    func: Box<dyn Fn(Value) -> std::result::Result<Value, ERPCError> + Send + Sync>,
    info: MethodInfo,
}

impl ClosureHandler {
    pub fn new<F>(
        func: F,
        name: impl Into<String>,
        arg_spec: Option<impl Into<String>>,
        docstring: Option<impl Into<String>>,
    ) -> Self
    where
        F: Fn(Value) -> std::result::Result<Value, crate::error::ERPCError> + Send + Sync + 'static,
    {
        ClosureHandler {
            func: Box::new(func),
            info: MethodInfo::new(name, arg_spec, docstring),
        }
    }
}

#[async_trait::async_trait]
impl MethodHandler for ClosureHandler {
    async fn call(&self,
        args: Value,
    ) -> std::result::Result<Value, ERPCError> {
        (self.func)(args)
    }
    
    fn info(&self) -> MethodInfo {
        self.info.clone()
    }
}

/// Thread-safe method registry
#[derive(Default)]
pub struct MethodRegistry {
    methods: RwLock<HashMap<String, Arc<dyn MethodHandler>>>,
}

impl MethodRegistry {
    pub fn new() -> Self {
        MethodRegistry {
            methods: RwLock::new(HashMap::new()),
        }
    }

    /// Register a method with closure
    pub async fn register_closure<F, Args, Ret>(
        &self,
        name: impl Into<String>,
        func: F,
        arg_spec: Option<impl Into<String>>,
        docstring: Option<impl Into<String>>,
    ) -> std::result::Result<(), crate::error::ERPCError>
    where
        F: Fn(Args) -> std::result::Result<Ret, ERPCError> + Send + Sync + 'static,
        Args: for<'de> Deserialize<'de> + Send,
        Ret: Serialize + Send,
    {
        let name = name.into();
        let handler = Arc::new(ClosureHandler::new(
            move |args_val: Value| {
                let args: Args = serde_lexpr::from_value(&args_val)
                    .map_err(|e| ERPCError::SerializationError(e.to_string()))?;
                
                let result = func(args)?;
                
                serde_lexpr::to_value(&result)
                    .map_err(|e| ERPCError::SerializationError(e.to_string()))
            },
            name.clone(),
            arg_spec,
            docstring,
        ));
        
        self.methods.write().await.insert(name, handler);
        Ok(())
    }

    /// Register a method with handler
    pub async fn register_handler(
        &self,
        name: impl Into<String>,
        handler: Arc<dyn MethodHandler>,
    ) {
        let name = name.into();
        self.methods.write().await.insert(name, handler);
    }

    /// Call a registered method
    pub async fn call_method(
        &self,
        name: &str,
        args: Value,
    ) -> std::result::Result<Value, crate::error::ERPCError> {
        let methods = self.methods.read().await;
        let handler = methods.get(name)
            .ok_or_else(|| ERPCError::MethodNotFound(name.to_string()))?
            .clone();
        
        handler.call(args).await
    }

    /// Check if a method exists
    pub async fn has_method(&self,
        name: &str
    ) -> bool {
        self.methods.read().await.contains_key(name)
    }

    /// Get method information for introspection
    pub async fn query_methods(&self) -> std::result::Result<Vec<MethodInfo>, crate::error::ERPCError> {
        let methods = self.methods.read().await;
        Ok(methods.values()
            .map(|handler| handler.info())
            .collect())
    }

    /// Remove a method
    pub async fn unregister(&self, name: &str) -> std::result::Result<(), crate::error::ERPCError> {
        self.methods.write().await.remove(name)
            .ok_or_else(|| ERPCError::MethodNotFound(name.to_string()))?;
        Ok(())
    }

    /// Get list of method names
    pub async fn method_names(&self) -> Vec<String> {
        let methods = self.methods.read().await;
        methods.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_method_registration() {
        let registry = MethodRegistry::new();
        
        registry.register_closure(
            "echo",
            |args: String| Ok(args),
            Some("args"),
            Some("Echo back the arguments"),
        ).await.unwrap();
        
        let result = registry.call_method("echo", Value::from("hello")).await.unwrap();
        assert_eq!(result, Value::from("hello"));
        
        let methods = registry.query_methods().await.unwrap();
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "echo");
    }

    #[tokio::test]
    async fn test_typed_method_registration() {
        let registry = MethodRegistry::new();
        
        registry.register_closure(
            "add",
            |(a, b): (i64, i64)| Ok(a + b),
            Some("a b"),
            Some("Add two numbers"),
        ).await.unwrap();
        
        let result = registry.call_method("add", Value::list(vec![Value::from(5), Value::from(3)]))
            .await.unwrap();
        
        assert_eq!(result, Value::from(8));
    }

    #[tokio::test]
    async fn test_method_not_found() {
        let registry = MethodRegistry::new();
        
        let result = registry.call_method("nonexistent", Value::Null).await;
        assert!(matches!(result, Err(ERPCError::MethodNotFound(_))));
    }
}