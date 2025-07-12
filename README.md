# rust-elrpc

A Rust implementation of the EPC (Emacs RPC) protocol for asynchronous communication between Emacs and Rust processes.

## What is EPC?

EPC (Emacs RPC) is an RPC stack for Emacs Lisp that enables asynchronous communication between Emacs and other processes using S-expression serialization. This implementation provides both client and server functionality for Rust applications.

## Features

- **Asynchronous**: Built on tokio for high-performance async I/O
- **Type-safe**: Strong typing with Rust's type system
- **S-expression protocol**: Compatible with existing EPC implementations
- **Bidirectional**: Both client and server can define and call methods
- **Process management**: Can spawn and manage EPC server processes

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
rust-elrpc = "0.1.0"
```

## Quick Start

### Server Example

```rust
use rust_elrpc::{EpcServer, EpcValue, EpcResult};

#[tokio::main]
async fn main() -> EpcResult<()> {
    let server = EpcServer::new().await?;
    
    // Register an echo method
    server.register_method("echo".to_string(), |args| {
        Ok(EpcValue::List(args.to_vec()))
    });
    
    // Register an add method
    server.register_method("add".to_string(), |args| {
        if args.len() != 2 {
            return Err(rust_elrpc::EpcError::application("add requires 2 arguments"));
        }
        
        let a = args[0].as_int().unwrap();
        let b = args[1].as_int().unwrap();
        Ok(EpcValue::Int(a + b))
    });
    
    // Print port (required by EPC protocol)
    server.print_port()?;
    
    // Start serving
    server.serve_forever().await
}
```

### Client Example

```rust
use rust_elrpc::{EpcClient, EpcValue, EpcResult};

#[tokio::main]
async fn main() -> EpcResult<()> {
    // Connect to server
    let client = EpcClient::connect("localhost", 12345).await?;
    
    // Call remote method
    let result = client.call_method(
        "echo".to_string(),
        vec![EpcValue::String("Hello!".to_string())]
    ).await?;
    
    println!("Result: {:?}", result);
    
    Ok(())
}
```

### Using with Emacs

From Emacs Lisp:

```elisp
(require 'epc)

;; Start the Rust EPC server
(setq epc (epc:start-epc "cargo" '("run" "--example" "echo_server")))

;; Call the echo method
(deferred:$
  (epc:call-deferred epc 'echo '("Hello from Emacs!"))
  (deferred:nextc it 
    (lambda (x) (message "Return: %S" x))))

;; Call the add method
(message "%S" (epc:call-sync epc 'add '(10 20)))

;; Query available methods
(message "%S" (epc:call-sync epc 'query-methods '()))

;; Stop when done
(epc:stop-epc epc)
```

## API Documentation

### EpcValue

The `EpcValue` enum represents all values that can be serialized in the EPC protocol:

- `Nil` - Represents nil/null
- `Bool(bool)` - Boolean values
- `Int(i64)` - Integer values
- `Float(f64)` - Floating point values
- `String(String)` - String values
- `Symbol(String)` - Symbols (like Lisp symbols)
- `List(Vec<EpcValue>)` - Lists of values
- `Dict(HashMap<String, EpcValue>)` - Key-value mappings

### EpcServer

- `EpcServer::new()` - Create server with OS-assigned port
- `EpcServer::bind(addr)` - Create server bound to specific address
- `register_method(name, handler)` - Register a callable method
- `print_port()` - Print port to stdout (EPC requirement)
- `serve_forever()` - Start serving clients

### EpcClient

- `EpcClient::connect(host, port)` - Connect to existing server
- `EpcClient::start_process(cmd, args)` - Start and connect to new process
- `call_method(name, args)` - Call remote method
- `query_methods()` - Get list of available methods
- `register_method(name, handler)` - Register method for peer to call

## Examples

Run the examples:

```bash
# Terminal 1: Start server
cargo run --example echo_server

# Terminal 2: Run client (update port number from server output)
cargo run --example echo_client
```

## Testing

```bash
cargo test
```

## Protocol Details

This implementation follows the EPC protocol specification:

- **Message format**: 6-byte hex length + S-expression payload
- **Message types**: `call`, `return`, `return-error`, `epc-error`, `methods`
- **Encoding**: UTF-8 text with S-expression serialization
- **Transport**: TCP connections

## Compatibility

This implementation is compatible with:

- [emacs-epc](https://github.com/kiwanami/emacs-epc) (Emacs Lisp)
- [python-epc](https://github.com/tkf/python-epc) (Python)
- [node-elrpc](https://github.com/kiwanami/node-elrpc) (Node.js)

## License

This project is licensed under the GPL-3.0-or-later license, consistent with other EPC implementations.