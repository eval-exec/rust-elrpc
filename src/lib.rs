//! Rust implementation of Emacs RPC (EPC) protocol
//!
//! This crate provides a complete implementation of the EPC protocol
//! for communication between Emacs and Rust applications.

pub mod error;
pub mod protocol;
pub mod registry;
pub mod server;
pub mod client;

pub use error::{ERPCError, Result};
pub use protocol::{Framer, Message};
pub use registry::{MethodInfo, MethodRegistry};
pub use server::{Server, ServerConfig};
pub use client::{Client, Process};