//! RustBot API Server
//!
//! This crate provides an OpenAI-compatible API server
//! for external applications to interact with RustBot.

mod auth;
mod routes;
mod server;

pub use server::*;
pub use auth::*;
pub use routes::ApiState;
