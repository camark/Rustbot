//! RustBot Message Bus
//!
//! This crate provides the message bus for communication between
//! channels, agent loop, and other components.
//!
//! ## Features
//!
//! - Async message queue
//! - Inbound and outbound message separation
//! - Session-based routing

mod events;
mod queue;

pub use events::*;
pub use queue::*;
