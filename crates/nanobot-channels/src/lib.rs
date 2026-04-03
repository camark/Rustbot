//! RustBot Channels
//!
//! This crate provides channel connectors for external messaging platforms
//! including Telegram, Discord, Feishu, and more.
//!
//! ## Features
//!
//! - Unified trait interface for all channels
//! - Channel registry and lifecycle management
//! - Automatic MessageBus integration
//! - Authentication storage and management

pub mod auth;
mod base;
mod manager;
mod registry;

#[cfg(feature = "telegram")]
mod telegram;

#[cfg(feature = "discord")]
mod discord;

#[cfg(feature = "feishu")]
pub mod feishu;

pub use auth::*;
pub use base::*;
pub use manager::*;
pub use registry::*;

#[cfg(feature = "telegram")]
pub use telegram::TelegramConnector;

#[cfg(feature = "discord")]
pub use discord::DiscordConnector;

#[cfg(feature = "feishu")]
pub use feishu::FeishuConnector;
