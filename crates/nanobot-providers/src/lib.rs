//! RustBot LLM Providers
//!
//! This crate provides implementations for various LLM providers
//! including OpenAI-compatible APIs, Anthropic, and more.
//!
//! ## Features
//!
//! - Unified trait interface for all providers
//! - Provider registry for auto-detection
//! - Support for streaming responses
//! - Built-in retry logic for transient errors

mod base;
mod registry;
mod openai_compat;
mod error;

pub use base::*;
pub use registry::*;
pub use openai_compat::*;
pub use error::*;
