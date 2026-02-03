//! Demo file parsing and command handling.
//!
//! This module provides the main [`Parser`] for reading Deadlock demo files,
//! along with command type definitions and header structures.

mod command;
mod parser;

pub use command::{command_name, CmdHeader, EDemoCommands, SvcMessages};
pub use parser::{Context, DemoHeader, MessageInfo, Parser};
