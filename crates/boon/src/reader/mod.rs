//! Byte-level reader utilities for demo parsing.
//!
//! This module stays format-agnostic and exposes a small, forward-only
//! byte reader plus errors and helpers (varints, LE reads, snappy).

pub mod bytes;
pub mod error;

pub use bytes::Reader;
pub use error::ReadError;
