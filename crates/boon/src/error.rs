use std::fmt;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid demo file: magic bytes mismatch (expected PBDEMS2\\0, got {got:?})")]
    InvalidMagic { got: [u8; 8] },
    #[error("unexpected end of data: needed {needed} bits, have {available}")]
    Overflow { needed: usize, available: usize },
    #[error("protobuf decode error: {0}")]
    Decode(#[from] prost::DecodeError),
    #[error("decompression error: {0}")]
    Decompress(String),
    #[error("unknown command type: {0}")]
    UnknownCommand(u32),
    #[error("parse error: {context}")]
    Parse { context: String },
}

impl From<snap::Error> for Error {
    fn from(e: snap::Error) -> Self {
        Error::Decompress(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// Lightweight error type for field value conversions.
#[derive(Debug)]
pub struct FieldValueConversionError;

impl fmt::Display for FieldValueConversionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "incompatible types or out of range integer conversion attempted"
        )
    }
}

impl std::error::Error for FieldValueConversionError {}
