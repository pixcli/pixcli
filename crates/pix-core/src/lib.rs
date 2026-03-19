//! Core types, validation, and utilities for Brazilian Pix payments.
//!
//! This crate provides:
//! - CRC16-CCITT checksum calculation
//! - Pix key type definitions and validation (CPF, CNPJ, Email, Phone, EVP)
//! - Common error types

pub mod crc16;
pub mod error;
pub mod pix_key;

pub use crc16::crc16_ccitt;
pub use error::PixError;
pub use pix_key::{PixKey, PixKeyType};
