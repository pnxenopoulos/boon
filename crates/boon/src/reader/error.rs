use thiserror::Error;

/// Errors that can occur while reading demo bytes.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum ReadError {
    /// Attempted to read past the available bits/bytes.
    #[error("unexpected EOF")]
    Eof,

    /// More than 5 continuation bytes while decoding a 32-bit LEB128.
    #[error("varint overflow")]
    Overflow,
}
