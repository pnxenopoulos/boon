//! Boon - A Deadlock demo file parser
//!
//! This crate provides functionality for parsing Deadlock demo files (.dem),
//! extracting game state, entity information, and metadata.
//!
//! # Example
//!
//! ```no_run
//! use std::path::Path;
//! use boon::Parser;
//!
//! let parser = Parser::from_file(Path::new("demo.dem")).unwrap();
//! let header = parser.file_header().unwrap();
//! println!("Map: {:?}", header.map_name);
//! ```

pub mod abilities;
pub mod demo;
pub mod entity;
pub mod error;
pub mod io;
pub mod modifiers;

// Re-export commonly used types at the crate root for convenience
pub use abilities::ability_name;
pub use demo::{
    CmdHeader, Context, GameEvent, MessageInfo, Parser, command_name, decode_event_payload,
};
pub use entity::{
    ClassEntry, ClassInfo, Entity, EntityContainer, FieldValue, Serializer, SerializerContainer,
    SerializerField, StringTable, StringTableContainer, StringTableEntry,
};
pub use error::{Error, Result};
pub use modifiers::modifier_name;
