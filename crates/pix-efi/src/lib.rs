//! Efí (formerly Gerencianet) Pix provider implementation.
//!
//! Provides OAuth2 + mTLS authentication and implements the `PixProvider` trait
//! for creating and managing Pix charges through the Efí API.

pub mod auth;
pub mod client;
pub mod config;
pub mod error;

pub use client::EfiClient;
pub use config::{EfiConfig, EfiEnvironment};
pub use error::EfiError;
