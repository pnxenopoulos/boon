//! Demo file parsing and command handling.
//!
//! This module provides the main [`Parser`] for reading Deadlock demo files,
//! along with command type definitions and header structures.

mod command;
pub mod decode;
mod parser;

pub use command::{CmdHeader, EDemoCommands, SvcMessages, command_name};
pub use decode::decode_event_payload;
pub use parser::{Context, GameEvent, MessageInfo, Parser};
