// Parser module with convenience re-exports
pub mod parser;
pub use parser::core::Parser;
pub use parser::error::ParserError;

// Reader module with convenience re-exports
pub mod reader;
pub use reader::bytes::Reader;
pub use reader::error::ReadError;
