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
//! let parser = Parser::from_file(Path::new("match.dem"))?;
//! let header = parser.file_header()?;
//! println!("Map: {:?}", header.map_name);
//! # Ok::<(), boon::Error>(())
//! ```
//!
//! # Reading game events
//!
//! ```no_run
//! use std::path::Path;
//! use boon::Parser;
//!
//! let parser = Parser::from_file(Path::new("match.dem"))?;
//! let events = parser.events(None)?;
//! for event in &events {
//!     println!("[tick {}] {} (msg_type {})", event.tick, event.name, event.msg_type);
//! }
//! # Ok::<(), boon::Error>(())
//! ```
//!
//! # Iterating entities per tick
//!
//! ```no_run
//! use std::path::Path;
//! use boon::Parser;
//!
//! let parser = Parser::from_file(Path::new("match.dem"))?;
//! parser.run_to_end(|ctx| {
//!     for (idx, entity) in ctx.entities().iter() {
//!         if entity.class_name.as_ref() == "CCitadelPlayerPawn" {
//!             // Access entity fields by resolved key
//!         }
//!     }
//! })?;
//! # Ok::<(), boon::Error>(())
//! ```
//!
//! # Name lookups
//!
//! Hero, ability, and modifier names come from boon-data. The first lookup
//! downloads latest if no verified local version exists. Reuse the maps:
//!
//! ```no_run
//! let names = boon::CatalogNames::load(None)?;
//! assert_eq!(names.hero_name(1), "Infernus");
//! # Ok::<(), boon::data::DataError>(())
//! ```
//!
//! ```
//! // Resolve numeric IDs to human-readable names
//! assert_eq!(boon::team_name(2), "Hidden King");
//! assert_eq!(boon::team_name(3), "Archmother");
//! assert_eq!(boon::hitgroup_name(1), "head");
//! assert_eq!(boon::lifestate_name(0), "alive");
//! ```

pub mod catalog_names;
pub mod data;
pub mod demo;
pub mod entity;
pub mod error;
pub mod game_modes;
pub mod heroes;
pub mod hitgroups;
pub mod io;
pub mod lifestates;
pub mod modifier_state;
pub mod patron_phases;
pub mod position;
pub mod rift;
pub mod stat_modifiers;
pub mod teams;

// Re-export commonly used types at the crate root for convenience
pub use catalog_names::CatalogNames;
pub use demo::{
    CmdHeader, Context, GameEvent, MessageInfo, Parser, command_name, decode_event_payload,
};
pub use entity::{
    ClassEntry, ClassInfo, ENTITY_HANDLE_INDEX_MASK, Entity, EntityChange, EntityChangeKind,
    EntityContainer, EntityId, FieldValue, INVALID_ENTITY_HANDLE, Serializer, SerializerContainer,
    SerializerField, StringTable, StringTableContainer, StringTableEntry, protobuf_handle_index,
};
pub use error::{Error, Result};
pub use game_modes::{all_game_modes, game_mode_name};
pub use heroes::hero_id_for_player_slot;
pub use hitgroups::{all_hitgroups, hitgroup_name};
pub use lifestates::{all_lifestates, lifestate_name};
pub use modifier_state::{
    EffectiveModifierState, ModifierChange, ModifierChangeKind, ModifierState,
    modifier_is_effective_at,
};
pub use patron_phases::{all_patron_phases, patron_phase_name};
pub use position::{CELL_BITS, CELL_SIZE, WORLD_HALF, cell_to_world};
pub use stat_modifiers::{
    DecodedStatModifierValue, StatModifierKind, StatModifierTotals, aggregate_stat_modifier_values,
    decode_stat_modifier_value_type,
};
pub use teams::{all_teams, team_name};
