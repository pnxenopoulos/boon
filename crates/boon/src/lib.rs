//! Boon - A Deadlock demo file parser
//!
//! This crate provides functionality for parsing Deadlock demo files (.dem),
//! extracting game state, entity information, and metadata.
//!
//! # Quick start
//!
//! ```no_run
//! use std::path::Path;
//! use boon::Parser;
//!
//! let parser = Parser::from_file(Path::new("match.dem")).unwrap();
//! let header = parser.file_header().unwrap();
//! println!("Map: {:?}", header.map_name);
//! ```
//!
//! # Reading game events
//!
//! ```no_run
//! use std::path::Path;
//! use boon::Parser;
//!
//! let parser = Parser::from_file(Path::new("match.dem")).unwrap();
//! let events = parser.events(None).unwrap();
//! for event in &events {
//!     println!("[tick {}] {} (msg_type {})", event.tick, event.name, event.msg_type);
//! }
//! ```
//!
//! # Iterating entities per tick
//!
//! ```no_run
//! use std::path::Path;
//! use boon::Parser;
//!
//! let parser = Parser::from_file(Path::new("match.dem")).unwrap();
//! parser.run_to_end(|ctx| {
//!     for (&idx, entity) in ctx.entities.iter() {
//!         if entity.class_name == "CCitadelPlayerPawn" {
//!             // Access entity fields by resolved key
//!         }
//!     }
//! }).unwrap();
//! ```
//!
//! # Name lookups
//!
//! ```
//! // Resolve numeric IDs to human-readable names
//! assert_eq!(boon::hero_name(1), "Infernus");
//! assert_eq!(boon::team_name(2), "Hidden King");
//! assert_eq!(boon::team_name(3), "Archmother");
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
