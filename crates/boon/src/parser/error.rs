use thiserror::Error;

/// Errors emitted by the demo parser.
#[derive(Debug, Error)]
pub enum ParserError {
    /// Underlying I/O error when loading a file.
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// Errors from the byte reader helpers.
    #[error(transparent)]
    Read(#[from] crate::reader::ReadError),

    /// File too small to contain the required header.
    #[error("file too small: {0} bytes")]
    TooSmall(usize),

    /// Wrong magic header. (Expected `PBDEMS2\0`.)
    #[error("wrong magic: got {0:02X?}")]
    WrongMagic([u8; 8]),
}
