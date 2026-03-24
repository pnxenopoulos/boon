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
pub mod game_modes;
pub mod heroes;
pub mod io;
pub mod modifiers;
pub mod teams;

// Re-export commonly used types at the crate root for convenience
pub use abilities::{ability_name, all_abilities};
pub use demo::{
    CmdHeader, Context, GameEvent, MessageInfo, Parser, command_name, decode_event_payload,
};
pub use entity::{
    ClassEntry, ClassInfo, Entity, EntityContainer, FieldValue, Serializer, SerializerContainer,
    SerializerField, StringTable, StringTableContainer, StringTableEntry,
};
pub use error::{Error, Result};
pub use game_modes::{all_game_modes, game_mode_name};
pub use heroes::{all_heroes, hero_name};
pub use modifiers::{all_modifiers, modifier_name};
pub use teams::{all_teams, team_name};
