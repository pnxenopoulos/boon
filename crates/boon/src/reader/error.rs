use thiserror::Error;

/// Errors that can occur while reading demo bytes.
#[derive(Debug, Error)]
pub enum ReadError {
    /// Attempted to read beyond end-of-buffer.
    #[error("unexpected EOF")]
    Eof,
    /// Varint had more continuation bytes than allowed for its width.
    #[error("varint too long")]
    VarintTooLong,
    /// Failure while expanding a compressed payload (Snappy).
    #[error("snappy decompression failed: {0}")]
    Decompress(String),
    /// IO error (used by helpers that read from the filesystem).
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
