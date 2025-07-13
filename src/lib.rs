//! Rust implementation of Emacs RPC (EPC) protocol
//!
//! This crate provides a complete implementation of the EPC protocol
//! for communication between Emacs and Rust applications.

pub mod error;
pub mod protocol;

pub use error::{ERPCError, Result};
pub use protocol::{Framer, Message};