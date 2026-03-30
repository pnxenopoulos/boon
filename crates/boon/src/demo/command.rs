//! Demo command types and utilities.

// Re-export proto enums for convenient access
pub use boon_proto::proto::EDemoCommands;
pub use boon_proto::proto::SvcMessages;

/// Outer demo command type constants.
///
/// Rust doesn't allow `Enum as i32` in match patterns, so we re-export
/// the protobuf enum values as plain `i32`/`u32` constants.
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
    /// Bitmask ORed into the raw command to indicate Snappy compression.
    pub const IS_COMPRESSED: u32 = EDemoCommands::DemIsCompressed as u32;
}

/// Inner packet service message type constants.
pub mod svc {
    use super::SvcMessages;
    pub const SERVER_INFO: u32 = SvcMessages::SvcServerInfo as u32;
    pub const CREATE_STRING_TABLE: u32 = SvcMessages::SvcCreateStringTable as u32;
    pub const UPDATE_STRING_TABLE: u32 = SvcMessages::SvcUpdateStringTable as u32;
    pub const PACKET_ENTITIES: u32 = SvcMessages::SvcPacketEntities as u32;
    pub const USER_MESSAGE: u32 = SvcMessages::SvcUserMessage as u32;
}

/// Game event message type constants (from `EBaseGameEvents`).
pub mod ge {
    /// `GE_Source1LegacyGameEventList` — defines available event types.
    pub const SOURCE1_LEGACY_GAME_EVENT_LIST: u32 = 205;
    /// `GE_Source1LegacyGameEvent` — an individual game event instance.
    pub const SOURCE1_LEGACY_GAME_EVENT: u32 = 207;
}

/// Return a human-readable name for a user message type.
pub fn user_message_name(msg_type: i32) -> String {
    boon_proto::proto::CitadelUserMessageIds::try_from(msg_type)
        .map(|e| e.as_str_name().to_string())
        .unwrap_or_else(|_| format!("UserMessage_{}", msg_type))
}

/// Header for a demo command in the stream.
///
/// Each command in the `.dem` file is prefixed with this header:
/// `(varint cmd | IS_COMPRESSED, varint tick, varint body_size)`.
#[derive(Debug, Clone)]
pub struct CmdHeader {
    /// Command type (one of the `dem::*` constants).
    pub cmd: i32,
    /// Game tick this command applies to.
    pub tick: i32,
    /// Whether the body is Snappy-compressed.
    pub compressed: bool,
    /// Size of the body in bytes (before decompression).
    pub body_size: u32,
}

/// Return a human-readable name for a demo command.
pub fn command_name(cmd: i32) -> &'static str {
    EDemoCommands::try_from(cmd)
        .map(|e| e.as_str_name())
        .unwrap_or("DEM_Unknown")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_name_stop() {
        assert_eq!(command_name(dem::STOP), "DEM_Stop");
    }

    #[test]
    fn command_name_file_header() {
        assert_eq!(command_name(dem::FILE_HEADER), "DEM_FileHeader");
    }

    #[test]
    fn command_name_unknown() {
        assert_eq!(command_name(9999), "DEM_Unknown");
    }

    #[test]
    fn user_message_name_known() {
        // 300 is k_ECitadelUserMsg_Damage
        let name = user_message_name(300);
        assert!(!name.starts_with("UserMessage_"), "got: {name}");
    }

    #[test]
    fn user_message_name_unknown() {
        let name = user_message_name(99999);
        assert_eq!(name, "UserMessage_99999");
    }

    #[test]
    fn dem_constants_match_expected() {
        assert_eq!(dem::STOP, 0);
        assert_eq!(dem::FILE_HEADER, 1);
        assert_eq!(dem::PACKET, 7);
        assert_eq!(dem::SIGNON_PACKET, 8);
    }

    #[test]
    fn ge_constants() {
        assert_eq!(ge::SOURCE1_LEGACY_GAME_EVENT_LIST, 205);
        assert_eq!(ge::SOURCE1_LEGACY_GAME_EVENT, 207);
    }
}
