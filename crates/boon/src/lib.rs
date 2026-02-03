//! Boon - A Deadlock demo file parser
//!
//! This crate provides functionality for parsing Deadlock demo files (.dem),
//! extracting game state, entity information, and metadata.
//!
//! # Example
//!
//! ```no_run
//! use boon::Parser;
//!
//! let parser = Parser::from_file("demo.dem").unwrap();
//! let header = parser.file_header().unwrap();
//! println!("Map: {:?}", header.map_name);
//! ```

pub mod demo;
pub mod entity;
pub mod error;
pub mod io;

// Re-export commonly used types at the crate root for convenience
pub use demo::{command_name, CmdHeader, Context, DemoHeader, MessageInfo, Parser};
pub use entity::{
    ClassEntry, ClassInfo, Entity, EntityContainer, FieldValue, Serializer, SerializerContainer,
    SerializerField, StringTable, StringTableContainer, StringTableEntry,
};
pub use error::{Error, Result};
