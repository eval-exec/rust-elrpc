//! Rust implementation of EPC (Emacs RPC) protocol
//!
//! EPC is an RPC stack for Emacs Lisp that enables asynchronous communication
//! between Emacs and other processes using S-expression serialization.

pub mod error;
pub mod message;
pub mod protocol;
pub mod client;
pub mod server;
pub mod types;

pub use error::{EpcError, EpcResult};
pub use message::{Message, MessageType};
pub use protocol::EpcConnection;
pub use client::EpcClient;
pub use server::EpcServer;
pub use types::EpcValue;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic() {
        // Basic test placeholder
        assert_eq!(2 + 2, 4);
    }
}
