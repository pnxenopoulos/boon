//! Demo parser entry points.
//!
//! This module provides a lightweight `Parser` that verifies a file is a
//! Source-style demo by checking its magic header (`"PBDEMS2\0"`), reads the
//! 8-byte prologue, and can peek the first `(cmd, tick, size)` triple.

pub mod core;
pub mod error;
pub mod sendtables;
pub mod stringtables;
