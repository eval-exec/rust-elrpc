//! Rust implementation of EPC (Emacs RPC) protocol
//!
//! EPC is an RPC stack for Emacs Lisp that enables asynchronous communication
//! between Emacs and other processes using S-expression serialization.

pub mod error;
pub mod message;
pub mod protocol;
pub mod client;
pub mod server;
pub mod server_handler;
pub mod types;

pub use error::{EpcError, EpcResult};
pub use message::{Message, MessageType};
pub use protocol::EpcConnection;
pub use client::EpcClient;
pub use server::EpcServer;
pub use types::EpcValue;

/// Initialize logging to write to /tmp/epc.log
pub fn init_file_logging() -> Result<(), Box<dyn std::error::Error>> {
    use simplelog::*;
    use std::fs::File;
    
    CombinedLogger::init(vec![
        WriteLogger::new(
            LevelFilter::Debug,
            Config::default(),
            File::create("/tmp/epc.log")?,
        ),
    ])?;
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::{Message, MessageType};
    use crate::types::EpcValue;

    #[test]
    fn test_basic() {
        // Basic test placeholder
        assert_eq!(2 + 2, 4);
    }

    #[test]
    fn test_parse_string_with_spaces() {
        let sexpr = r#"(call 179 greet ("Emacs User"))"#;
        let message = Message::from_sexpr(sexpr);
        assert!(message.is_ok(), "Failed to parse: {:?}", message.err());
        let message = message.unwrap();
        assert_eq!(message.msg_type, MessageType::Call);
        assert_eq!(message.session_id, "179");
        
        let method_name = message.get_method_name().unwrap();
        assert_eq!(method_name, "greet");
        
        let args = message.get_args().unwrap();
        assert_eq!(args.len(), 1);
        
        match &args[0] {
            EpcValue::String(s) => assert_eq!(s, "Emacs User"),
            _ => panic!("Expected a string argument"),
        }
    }
}
