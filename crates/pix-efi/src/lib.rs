#![deny(unsafe_code)]
//! Efí (formerly Gerencianet) Pix provider implementation.
//!
//! Provides OAuth2 + mTLS authentication and implements the `PixProvider` trait
//! for creating and managing Pix charges through the Efí API.

pub mod auth;
pub mod client;
pub mod config;
pub mod error;
pub mod validate;

pub use client::{EfiClient, WebhookInfo};
pub use config::{EfiConfig, EfiEnvironment};
pub use error::EfiError;
