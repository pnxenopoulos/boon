//! Demo command types and utilities.

// Re-export proto enums for convenient access
pub use boon_proto::proto::EDemoCommands;
pub use boon_proto::proto::SvcMessages;

// Constants for use in match patterns (Rust doesn't allow `Enum as i32` in patterns)
pub mod dem {
    use super::EDemoCommands;
    pub const STOP: i32 = EDemoCommands::DemStop as i32;
    pub const FILE_HEADER: i32 = EDemoCommands::DemFileHeader as i32;
    pub const FILE_INFO: i32 = EDemoCommands::DemFileInfo as i32;
    pub const SYNC_TICK: i32 = EDemoCommands::DemSyncTick as i32;
    pub const SEND_TABLES: i32 = EDemoCommands::DemSendTables as i32;
    pub const CLASS_INFO: i32 = EDemoCommands::DemClassInfo as i32;
    pub const PACKET: i32 = EDemoCommands::DemPacket as i32;
    pub const SIGNON_PACKET: i32 = EDemoCommands::DemSignonPacket as i32;
    pub const FULL_PACKET: i32 = EDemoCommands::DemFullPacket as i32;
    pub const IS_COMPRESSED: u32 = EDemoCommands::DemIsCompressed as u32;
}

pub mod svc {
    use super::SvcMessages;
    pub const SERVER_INFO: u32 = SvcMessages::SvcServerInfo as u32;
    pub const CREATE_STRING_TABLE: u32 = SvcMessages::SvcCreateStringTable as u32;
    pub const UPDATE_STRING_TABLE: u32 = SvcMessages::SvcUpdateStringTable as u32;
    pub const PACKET_ENTITIES: u32 = SvcMessages::SvcPacketEntities as u32;
}

/// Header for a demo command in the stream.
#[derive(Debug, Clone)]
pub struct CmdHeader {
    pub cmd: i32,
    pub tick: i32,
    pub compressed: bool,
    pub body_size: u32,
}

/// Return a human-readable name for a demo command.
pub fn command_name(cmd: i32) -> &'static str {
    EDemoCommands::try_from(cmd)
        .map(|e| e.as_str_name())
        .unwrap_or("DEM_Unknown")
}
