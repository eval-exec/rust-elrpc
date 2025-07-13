# Rust-ELRPC Specification Document

## Overview

Rust-ELRPC is a comprehensive implementation of the Emacs RPC (EPC) protocol in Rust, providing seamless inter-process communication between Emacs and Rust applications using S-expression serialization over TCP.

## Core Architecture

### Peer-to-Peer Architecture
- **Server**: Process that opens a TCP port and waits for connections
- **Client**: Process that connects to a server
- **Bidirectional**: Both peers can define methods and call remote methods

### Protocol Foundation
- Based on SWANK protocol with EPC extensions
- Uses S-expression format for all data serialization
- TCP transport with length-prefixed messages
- UTF-8 encoding for all text content

## Protocol Specification

### Message Format
```
[6-byte length][S-expression payload]
```

### Message Types
1. **call** - Invoke remote method
2. **return** - Successful method return
3. **return-error** - Application-level error
4. **epc-error** - Protocol-level error
5. **methods** - Query available methods

### Message Structure
```lisp
;; Method call
(call uid method-name [arg1 arg2 ...])

;; Successful return
(return uid return-value)

;; Application error
(return-error uid [error-class error-message backtrace])

;; Protocol error
(epc-error uid error-message)

;; Method query
(methods uid)
```

## Data Type Mapping

### S-expression to Rust Types (per emacs-epc spec)
| S-expression | Rust Type |
|--------------|-----------|
| `nil` | `lexpr::Value::Null` |
| Symbol | `lexpr::Value::Symbol` |
| Number | `lexpr::Value::Number` |
| String | `lexpr::Value::String` |
| List | `lexpr::Value::Vector` |
| Complex object (alist) | `lexpr::Value::Vector` of cons cells |

### Complex Objects and Association Lists with lexpr-rs
```rust
use lexpr::Value;
use lexpr_macros::sexp;

// Creating alists and complex objects as supported by emacs-epc
let person = sexp!(((name . "Alice") (age . 30) (active . #t)));

// Accessing values via indexing (works with cons cells)
let name = person["name"].as_str().unwrap(); // "Alice"
let age = person["age"].as_u64().unwrap();   // 30

// Building complex objects programmatically
let mut attributes = Vec::new();
attributes.push(Value::cons("name", "Alice"));
attributes.push(Value::cons("age", 30));
let person_value = Value::list(attributes);

// Note: emacs-epc uses standard S-expression lists and cons cells
// No special "dotted pair" type - just regular cons cell representation
```

### Serialization Framework with lexpr-rs
- Uses `serde-lexpr` for serialization/deserialization
- Reuses existing `lexpr` crate for S-expression parsing/printing
- Supports custom type conversions via Serde derive macros
- Integrates with existing `lexpr::Value` type system and standard cons cells for complex objects

## Core API Design

### Server API
```rust
use elrpc::{Server, MethodRegistry};

// Create server
let server = Server::bind("127.0.0.1:0").await?;
let port = server.port();

// Register methods
server.register_method("echo", |args: Vec<serde_lexpr::Value>| {
    Ok(args)
});

server.register_method_with_meta("add", |a: i64, b: i64| {
    Ok(a + b)
}, "a b", "Add two numbers");

// Start serving
server.serve().await?;
```

### Client API
```rust
use elrpc::{Client, Process};

// Connect to server
let client = Client::connect("127.0.0.1:12345").await?;

// Synchronous calls
let result: i64 = client.call_sync("add", (1, 2)).await?;

// Asynchronous calls
let future = client.call_async("echo", vec!["hello", "world"]);
let result = future.await?;

// Process management
let mut process = Process::spawn("/usr/bin/python3", &["server.py"]).await?;
let result: String = process.call_sync("greet", "Alice").await?;
process.stop().await?;
```

### Method Registry
```rust
pub trait MethodRegistry {
    fn register_method<F, Args, Ret>(&self, name: &str, func: F)
    where
        F: Fn(Args) -> Result<Ret, Box<dyn Error + Send + Sync>> + Send + Sync + 'static,
        Args: DeserializeOwned,
        Ret: Serialize;
    
    fn register_method_with_meta<F, Args, Ret>(
        &self,
        name: &str,
        func: F,
        arg_spec: &str,
        docstring: &str
    );
    
    fn query_methods(&self) -> Vec<MethodInfo>;
}
```

## Error Handling

### Error Types
```rust
pub enum ERPCError {
    ConnectionClosed,
    MethodNotFound(String),
    SerializationError(String),
    ProtocolError(String),
    ApplicationError {
        class: String,
        message: String,
        backtrace: Vec<String>,
    },
}
```

### Error Propagation
- Application errors are serialized and returned to caller
- Protocol errors trigger connection termination
- All errors implement `std::error::Error`

## Async Model

### Tokio-based Architecture
- Uses Tokio runtime for async I/O
- Provides both async and sync APIs
- Backpressure handling for message queues
- Connection pooling for clients

### Threading Model
- Single-threaded event loop per connection
- Worker thread pool for CPU-intensive tasks
- Configurable thread limits

## Serialization Layer

### Integration with lexpr-rs
```rust
use lexpr::Value;
use serde_lexpr::{from_value, to_value};

// Using lexpr::Value for dynamic typing
let value = lexpr::from_str("(add 1 2)")?;

// Using serde-lexpr for static typing
#[derive(Serialize, Deserialize)]
struct AddRequest {
    a: i64,
    b: i64,
}

let request = AddRequest { a: 1, b: 2 };
let sexp = serde_lexpr::to_value(&request)?;
```

### Custom Type Support
```rust
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
struct Person {
    name: String,
    age: u32,
}

// Automatic conversion via serde-lexpr
impl Person {
    fn to_sexp(&self) -> Result<lexpr::Value, ERPCError> {
        Ok(serde_lexpr::to_value(self)?)
    }
    
    fn from_sexp(value: &lexpr::Value) -> Result<Self, ERPCError> {
        Ok(serde_lexpr::from_value(value)?)
    }
}
```

## Connection Management

### Server Lifecycle
```rust
impl Server {
    pub async fn bind(addr: &str) -> Result<Self, ERPCError>;
    pub fn port(&self) -> u16;
    pub async fn serve(self) -> Result<(), ERPCError>;
    pub async fn shutdown(self) -> Result<(), ERPCError>;
}
```

### Client Lifecycle
```rust
impl Client {
    pub async fn connect(addr: &str) -> Result<Self, ERPCError>;
    pub async fn call_sync<Args, Ret>(&self, method: &str, args: Args) -> Result<Ret, ERPCError>;
    pub fn call_async<Args, Ret>(&self, method: &str, args: Args) -> impl Future<Output = Result<Ret, ERPCError>>;
    pub async fn query_methods(&self) -> Result<Vec<MethodInfo>, ERPCError>;
    pub async fn close(self) -> Result<(), ERPCError>;
}
```

### Process Management
```rust
impl Process {
    pub async fn spawn(program: &str, args: &[&str]) -> Result<Self, ERPCError>;
    pub async fn call_sync<Args, Ret>(&self, method: &str, args: Args) -> Result<Ret, ERPCError>;
    pub fn call_async<Args, Ret>(&self, method: &str, args: Args) -> impl Future<Output = Result<Ret, ERPCError>>;
    pub async fn stop(self) -> Result<(), ERPCError>;
    pub fn pid(&self) -> Option<u32>;
}
```

## Configuration Options

### Server Configuration
```rust
pub struct ServerConfig {
    pub bind_addr: String,
    pub max_connections: usize,
    pub worker_threads: usize,
    pub timeout: Duration,
    pub logger: Option<Logger>,
}
```

### Client Configuration
```rust
pub struct ClientConfig {
    pub connect_timeout: Duration,
    pub request_timeout: Duration,
    pub max_retries: u32,
    pub retry_delay: Duration,
}
```

## Testing Framework

### Unit Tests
- Serialization/deserialization round-trip tests
- Protocol message format validation
- Error handling coverage

### Integration Tests
- Cross-language compatibility tests
- Performance benchmarks
- Stress testing with concurrent connections

### Example Test Suite
```rust
#[tokio::test]
async fn test_echo_server() {
    let server = TestServer::new().await;
    let client = Client::connect(&server.addr()).await.unwrap();
    
    let result: String = client.call_sync("echo", "hello").await.unwrap();
    assert_eq!(result, "hello");
}

#[tokio::test]
async fn test_async_calls() {
    let server = TestServer::new().await;
    let client = Client::connect(&server.addr()).await.unwrap();
    
    let futures = (0..100).map(|i| {
        client.call_async("add", (i, i * 2))
    });
    
    let results = futures::future::join_all(futures).await;
    assert_eq!(results.len(), 100);
}
```

## Performance Characteristics

### Expected Performance
- **Throughput**: 10,000+ calls/second (local)
- **Latency**: <1ms round-trip (local)
- **Memory**: <1MB per connection
- **CPU**: Minimal overhead due to zero-copy serialization

### Optimization Features
- Connection pooling
- Request batching
- Lazy serialization
- Async buffer management

## Security Considerations

### Transport Security
- TLS support for encrypted connections
- Authentication hooks
- Rate limiting
- Input validation

### Sandboxing
- Restricted method execution
- Resource limits
- Process isolation

## Migration Guide

### From Python EPC
```python
# Python
server = EPCServer(('localhost', 0))
@server.register_function
def add(a, b):
    return a + b
```

```rust
// Rust
let server = Server::bind("localhost:0").await?;
server.register_method("add", |(a, b): (i64, i64)| Ok(a + b));
```

### From Ruby Elrpc
```ruby
# Ruby
server = Elrpc.start_server
server.def_method("echo") { |arg| arg }
```

```rust
// Rust
let server = Server::bind("localhost:0").await?;
server.register_method("echo", |arg: String| Ok(arg));
```

## Project Structure

```
rust-elrpc/
├── src/
│   ├── lib.rs              # Main library entry
│   ├── server.rs           # Server implementation
│   ├── client.rs           # Client implementation
│   ├── protocol.rs         # Protocol definitions
│   ├── serialization.rs    # S-expression handling
│   ├── error.rs            # Error types
│   └── utils.rs            # Utilities
├── examples/
│   ├── echo_server.rs      # Basic echo server
│   ├── echo_client.rs      # Basic echo client
│   ├── process_server.rs   # Process management
│   └── gtk_integration.rs  # GTK integration
├── tests/
│   ├── integration_tests.rs
│   ├── cross_language.rs
│   └── benchmarks.rs
├── benches/
│   └── performance.rs
└── docs/
    ├── tutorial.md
    ├── api_reference.md
    └── migration_guide.md
```

## Development Milestones

### Phase 1: Core Protocol (Week 1-2)
- [ ] Basic message framing
- [ ] S-expression serialization
- [ ] TCP transport layer
- [ ] Protocol message types

### Phase 2: Basic Server/Client (Week 3-4)
- [ ] Server implementation
- [ ] Client implementation
- [ ] Method registry
- [ ] Sync/async call support

### Phase 3: Advanced Features (Week 5-6)
- [ ] Process management
- [ ] Error handling
- [ ] Method introspection
- [ ] Connection pooling

### Phase 4: Testing & Documentation (Week 7-8)
- [ ] Comprehensive test suite
- [ ] Performance benchmarks
- [ ] Cross-language tests
- [ ] Documentation completion

### Phase 5: Integration & Polish (Week 9-10)
- [ ] Example applications
- [ ] Migration guides
- [ ] Performance optimization
- [ ] Security review

## Compatibility Matrix

| Feature | Emacs | Python | Ruby | Node.js | Rust |
|---------|-------|--------|------|---------|------|
| Basic calls | ✅ | ✅ | ✅ | ✅ | ✅ |
| Async calls | ✅ | ✅ | ✅ | ✅ | ✅ |
| Method registry | ✅ | ✅ | ✅ | ✅ | ✅ |
| Process spawning | ✅ | ✅ | ✅ | ✅ | ✅ |
| Error handling | ✅ | ✅ | ✅ | ✅ | ✅ |
| Cross-language | ✅ | ✅ | ✅ | ✅ | ✅ |

## License

MIT License - consistent with Ruby and Node.js implementations