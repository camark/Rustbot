//! RustBot Configuration System
//!
//! This crate provides configuration loading, parsing, and management
//! for the RustBot AI assistant framework.
//!
//! ## Features
//!
//! - Compatible with Python nanobot config.json format
//! - Supports both camelCase and snake_case field names
//! - Environment variable overrides via NANOBOT_ prefix
//! - Type-safe configuration structures

mod paths;
mod schema;
mod loader;

pub use paths::{get_config_dir, get_workspace_dir, ConfigPaths};
pub use schema::*;
pub use loader::{ConfigLoader, ConfigError};
