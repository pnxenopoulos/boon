use std::path::Path;

use memmap2::Mmap;
use pbdems2::{ParserState, PreparedPlayback};
use prost::Message;

use crate::error::{Error, Result};
use crate::io::{BitReader, ByteReader};

use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};

use super::adapter::CitadelAdapter;
use super::command::{self, CmdHeader, dem, ge, svc};

mod playback;

use boon_proto::proto::{
    CDemoFileHeader, CDemoFileInfo, CDemoFullPacket, CDemoPacket, CMsgSource1LegacyGameEvent,
    CMsgSource1LegacyGameEventList, CitadelUserMessageIds, CsvcMsgUserMessage, EBaseUserMessages,
    ECitadelGameEvents,
};

/// Magic bytes at the start of every Source 2 demo file.
const MAGIC: &[u8; 8] = b"PBDEMS2\0";
/// File header: 8 bytes magic + 4 bytes fileinfo_offset + 4 bytes spawngroups_offset.
const HEADER_SIZE: usize = 16;

/// Scratch buffer size for decompressed command bodies.
const BUF_SIZE: usize = 2 * 1024 * 1024;
/// Initial capacity for selected inner-message payloads. The buffer grows for
/// unusually large messages without eagerly zeroing 2 MiB on every scan.
const PACKET_BUF_CAPACITY: usize = 64 * 1024;

fn is_direct_event_type(message_type: u32) -> bool {
    let message_type = message_type as i32;
    CitadelUserMessageIds::try_from(message_type).is_ok()
        || ECitadelGameEvents::try_from(message_type).is_ok()
        || EBaseUserMessages::try_from(message_type).is_ok()
}

/// Information about a demo message in the command stream.
#[derive(Debug, Clone, serde::Serialize)]
pub struct MessageInfo {
    /// Zero-based ordinal position in the command stream.
    pub index: usize,
    /// Command type (one of the `dem::*` constants).
    pub cmd: i32,
    /// Human-readable command name.
    pub cmd_name: String,
    /// Game tick this command applies to.
    pub tick: i32,
    /// Whether the body is Snappy-compressed.
    pub compressed: bool,
    /// Body size in bytes (before decompression).
    pub body_size: u32,
    /// Absolute byte offset from the start of the file.
    pub offset: usize,
}

/// Full parser context after initialization.
///
/// Holds all decoded game state: serializers, class definitions, string
/// tables, and live entities. Returned by [`Parser::parse_init`],
/// [`Parser::parse_to_tick`], and updated incrementally during
/// [`Parser::run_to_end`].
pub type Context = ParserState;

/// A game event extracted from the demo.
#[derive(Debug, Clone, serde::Serialize)]
pub struct GameEvent {
    /// Game tick at which this event occurred.
    pub tick: i32,
    /// Human-readable event name, such as `"player_death"`.
    pub name: String,
    /// Numeric message type from the packet stream.
    pub msg_type: u32,
    /// Key-value pairs for Source 1 legacy game events; empty for user messages.
    pub keys: Vec<(String, String)>,
    /// Raw protobuf bytes of the event. Use [`crate::decode_event_payload`] to decode.
    #[serde(skip)]
    pub payload: Vec<u8>,
}

struct EventDescriptor {
    name: String,
    field_names: Vec<String>,
}

fn format_event_key(key: &boon_proto::proto::c_msg_source1_legacy_game_event::KeyT) -> String {
    if let Some(ref s) = key.val_string {
        return s.clone();
    }
    if let Some(f) = key.val_float {
        return f.to_string();
    }
    if let Some(l) = key.val_long {
        return l.to_string();
    }
    if let Some(s) = key.val_short {
        return s.to_string();
    }
    if let Some(b) = key.val_byte {
        return b.to_string();
    }
    if let Some(b) = key.val_bool {
        return b.to_string();
    }
    if let Some(u) = key.val_uint64 {
        return u.to_string();
    }
    String::new()
}

/// Internal storage for demo data — either memory-mapped or an owned byte buffer.
enum Storage {
    Mmap(Mmap),
    Bytes(Vec<u8>),
}

impl AsRef<[u8]> for Storage {
    fn as_ref(&self) -> &[u8] {
        match self {
            Storage::Mmap(m) => m,
            Storage::Bytes(b) => b,
        }
    }
}

/// The main parser. Owns the demo file data (memory-mapped or in-memory).
pub struct Parser {
    storage: Storage,
    prepared_cache: OnceLock<PreparedPlayback<CitadelAdapter>>,
    prepared_lock: Mutex<()>,
}

impl Parser {
    /// Open a demo file and memory-map it for zero-copy parsing.
    pub fn from_file(path: &Path) -> Result<Self> {
        let file = std::fs::File::open(path)?;
        // SAFETY: The file is opened read-only and the mapping lives as
        // long as the Parser.  Undefined behavior can occur if an external
        // process truncates or modifies the file while mapped; callers must
        // ensure the file is not concurrently mutated.
        let mmap = unsafe { Mmap::map(&file)? };
        Ok(Self {
            storage: Storage::Mmap(mmap),
            prepared_cache: OnceLock::new(),
            prepared_lock: Mutex::new(()),
        })
    }

    /// Create a parser from an in-memory byte buffer.
    ///
    /// This is useful for testing, WASM targets (where mmap is unavailable),
    /// or when the demo data has already been loaded into memory.
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self {
            storage: Storage::Bytes(bytes),
            prepared_cache: OnceLock::new(),
            prepared_lock: Mutex::new(()),
        }
    }

    /// Returns the raw demo data.
    fn data(&self) -> &[u8] {
        self.storage.as_ref()
    }

    /// Verify magic bytes.
    /// Verify that the file has valid demo magic bytes.
    pub fn verify(&self) -> Result<()> {
        if self.data().len() < HEADER_SIZE {
            return Err(Error::Parse {
                context: "file too small for demo header".into(),
            });
        }

        let mut magic = [0u8; 8];
        magic.copy_from_slice(&self.data()[0..8]);
        if &magic != MAGIC {
            return Err(Error::InvalidMagic { got: magic });
        }

        Ok(())
    }

    fn read_cmd_header(reader: &mut ByteReader) -> Result<CmdHeader> {
        let raw_cmd = reader.read_uvarint32()?;
        let compress_flag = dem::IS_COMPRESSED;
        let compressed = (raw_cmd & compress_flag) != 0;
        let cmd = (raw_cmd & !compress_flag) as i32;
        let tick_raw = reader.read_uvarint32()?;
        let tick = tick_raw as i32;
        let body_size = reader.read_uvarint32()?;
        Ok(CmdHeader {
            cmd,
            tick,
            compressed,
            body_size,
        })
    }

    /// Read and decompress a command body into the provided buffer.
    /// The buffer is resized as needed and can be reused across calls.
    fn read_cmd_body(reader: &mut ByteReader, header: &CmdHeader, buf: &mut Vec<u8>) -> Result<()> {
        let raw = reader.read_bytes(header.body_size as usize)?;
        if header.compressed {
            let decompressed_len =
                snap::raw::decompress_len(raw).map_err(|e| Error::Decompress(e.to_string()))?;
            buf.clear();
            buf.resize(decompressed_len, 0);
            snap::raw::Decoder::new()
                .decompress(raw, buf)
                .map_err(|e| Error::Decompress(e.to_string()))?;
        } else {
            buf.clear();
            buf.extend_from_slice(raw);
        }
        Ok(())
    }

    /// Iterate all commands and return metadata about each.
    /// Continues past DEM_Stop to capture DEM_FileInfo.
    pub fn messages(&self) -> Result<Vec<MessageInfo>> {
        self.verify()?;
        let data = &self.data()[HEADER_SIZE..];
        let mut reader = ByteReader::new(data);
        let mut messages = Vec::new();
        let mut index = 0;

        while reader.remaining() > 0 {
            let offset = reader.position() + HEADER_SIZE;
            let header = match Self::read_cmd_header(&mut reader) {
                Ok(h) => h,
                Err(_) => break,
            };

            messages.push(MessageInfo {
                index,
                cmd: header.cmd,
                cmd_name: command::command_name(header.cmd).to_string(),
                tick: header.tick,
                compressed: header.compressed,
                body_size: header.body_size,
                offset,
            });

            // DEM_Stop has no body, and DEM_FileInfo follows it
            if header.cmd == dem::STOP {
                index += 1;
                continue;
            }

            // DEM_FileInfo comes after DEM_Stop; once we've read it, we're done
            if header.cmd == dem::FILE_INFO {
                reader.skip(header.body_size as usize).ok();
                break;
            }

            if reader.skip(header.body_size as usize).is_err() {
                break;
            }

            index += 1;
        }

        Ok(messages)
    }

    /// Find and decode the CDemoFileHeader message.
    pub fn file_header(&self) -> Result<CDemoFileHeader> {
        self.verify()?;
        let data = &self.data()[HEADER_SIZE..];
        let mut reader = ByteReader::new(data);
        let mut body_buf = Vec::with_capacity(BUF_SIZE);

        while reader.remaining() > 0 {
            let header = Self::read_cmd_header(&mut reader)?;

            if header.cmd == dem::FILE_HEADER {
                Self::read_cmd_body(&mut reader, &header, &mut body_buf)?;
                return CDemoFileHeader::decode(&body_buf[..]).map_err(Error::from);
            }

            if header.cmd == dem::STOP {
                break;
            }

            reader.skip(header.body_size as usize)?;
        }

        Err(Error::Parse {
            context: "DEM_FileHeader not found".into(),
        })
    }

    /// Decode CDemoFileInfo using the offset stored in the file header.
    pub fn file_info(&self) -> Result<CDemoFileInfo> {
        self.verify()?;

        // Bytes 8..12 of the file header contain the absolute offset to DEM_FileInfo.
        let fileinfo_offset = u32::from_le_bytes([
            self.data()[8],
            self.data()[9],
            self.data()[10],
            self.data()[11],
        ]) as usize;

        let data = &self.data()[HEADER_SIZE..];
        let mut reader = ByteReader::new(data);
        // The offset is relative to the start of the file; adjust for the header we sliced off.
        reader.seek(fileinfo_offset.saturating_sub(HEADER_SIZE))?;

        let header = Self::read_cmd_header(&mut reader)?;
        if header.cmd != dem::FILE_INFO {
            return Err(Error::Parse {
                context: format!(
                    "expected DEM_FileInfo at offset {}, found command {}",
                    fileinfo_offset, header.cmd
                ),
            });
        }

        let mut body_buf = Vec::new();
        Self::read_cmd_body(&mut reader, &header, &mut body_buf)?;
        CDemoFileInfo::decode(&body_buf[..]).map_err(Error::from)
    }

    /// Parse game events from the demo.
    ///
    /// Extracts Source 1 legacy game events and Citadel user messages from
    /// `DEM_Packet`, `DEM_SignonPacket`, and `DEM_FullPacket` commands.
    /// If `max_tick` is set, stops parsing once the tick exceeds the limit.
    pub fn events(&self, max_tick: Option<i32>) -> Result<Vec<GameEvent>> {
        self.decode_events(max_tick, None)
    }

    /// Parse only events whose final numeric message type is selected.
    ///
    /// Direct Citadel events can be rejected before their payload is copied.
    /// Wrapped user messages require decoding their small outer envelope first.
    pub fn events_filtered(
        &self,
        max_tick: Option<i32>,
        event_types: &HashSet<u32>,
    ) -> Result<Vec<GameEvent>> {
        self.decode_events(max_tick, Some(event_types))
    }

    fn decode_events(
        &self,
        max_tick: Option<i32>,
        event_types: Option<&HashSet<u32>>,
    ) -> Result<Vec<GameEvent>> {
        self.verify()?;
        let data = &self.data()[HEADER_SIZE..];
        let mut reader = ByteReader::new(data);
        let mut body_buf = Vec::with_capacity(BUF_SIZE);
        let mut packet_buf = Vec::with_capacity(PACKET_BUF_CAPACITY);
        let mut events = Vec::new();
        let mut descriptors: HashMap<i32, EventDescriptor> = HashMap::new();

        while reader.remaining() > 0 {
            let header = match Self::read_cmd_header(&mut reader) {
                Ok(h) => h,
                Err(_) => break,
            };

            if header.cmd == dem::STOP {
                break;
            }

            if let Some(max) = max_tick
                && header.tick > max
            {
                break;
            }

            match header.cmd {
                dem::PACKET | dem::SIGNON_PACKET => {
                    Self::read_cmd_body(&mut reader, &header, &mut body_buf)?;
                    let cmd = CDemoPacket::decode(&body_buf[..])?;
                    let pkt_data = cmd.data.unwrap_or_default();
                    Self::process_packet_events(
                        &pkt_data,
                        header.tick,
                        &mut descriptors,
                        &mut events,
                        &mut packet_buf,
                        event_types,
                    )?;
                }
                dem::FULL_PACKET => {
                    Self::read_cmd_body(&mut reader, &header, &mut body_buf)?;
                    let cmd = CDemoFullPacket::decode(&body_buf[..])?;
                    if let Some(packet) = cmd.packet {
                        let pkt_data = packet.data.unwrap_or_default();
                        Self::process_packet_events(
                            &pkt_data,
                            header.tick,
                            &mut descriptors,
                            &mut events,
                            &mut packet_buf,
                            event_types,
                        )?;
                    }
                }
                _ => {
                    reader.skip(header.body_size as usize)?;
                }
            }
        }

        Ok(events)
    }

    /// Process a packet's inner messages for game events.
    fn process_packet_events(
        pkt_data: &[u8],
        tick: i32,
        descriptors: &mut HashMap<i32, EventDescriptor>,
        events: &mut Vec<GameEvent>,
        packet_buf: &mut Vec<u8>,
        event_types: Option<&HashSet<u32>>,
    ) -> Result<()> {
        let mut br = BitReader::new(pkt_data);

        while br.bits_remaining() > 8 {
            let msg_type = br.read_ubitvar()?;
            let size = br.read_uvarint32()? as usize;

            let selected = event_types.is_none_or(|types| types.contains(&msg_type));
            let relevant = msg_type == ge::SOURCE1_LEGACY_GAME_EVENT_LIST
                || (msg_type == ge::SOURCE1_LEGACY_GAME_EVENT && selected)
                // The final type of a wrapped message is inside its envelope.
                || msg_type == svc::USER_MESSAGE
                || (selected && is_direct_event_type(msg_type));
            if !relevant {
                br.skip_bits(size * 8)?;
                continue;
            }

            if size > packet_buf.len() {
                packet_buf.resize(size, 0);
            }
            br.read_bytes(&mut packet_buf[..size])?;
            let msg_data = &packet_buf[..size];

            match msg_type {
                ge::SOURCE1_LEGACY_GAME_EVENT_LIST => {
                    let msg = CMsgSource1LegacyGameEventList::decode(msg_data)?;
                    for desc in msg.descriptors {
                        let eventid = desc.eventid.unwrap_or_default();
                        let name = desc.name.unwrap_or_default();
                        let field_names = desc
                            .keys
                            .iter()
                            .map(|k| k.name.clone().unwrap_or_default())
                            .collect();
                        descriptors.insert(eventid, EventDescriptor { name, field_names });
                    }
                }
                ge::SOURCE1_LEGACY_GAME_EVENT => {
                    let msg = CMsgSource1LegacyGameEvent::decode(msg_data)?;
                    let eventid = msg.eventid.unwrap_or_default();
                    let (name, keys) = if let Some(desc) = descriptors.get(&eventid) {
                        let keys: Vec<(String, String)> = desc
                            .field_names
                            .iter()
                            .zip(msg.keys.iter())
                            .map(|(fname, key)| (fname.clone(), format_event_key(key)))
                            .collect();
                        (desc.name.clone(), keys)
                    } else {
                        let name = msg
                            .event_name
                            .unwrap_or_else(|| format!("event_{}", eventid));
                        (name, Vec::new())
                    };
                    events.push(GameEvent {
                        tick,
                        name,
                        msg_type,
                        keys,
                        payload: msg_data.to_vec(),
                    });
                }
                svc::USER_MESSAGE => {
                    let msg = CsvcMsgUserMessage::decode(msg_data)?;
                    let inner_type = msg.msg_type.unwrap_or_default();
                    if event_types.is_some_and(|types| !types.contains(&(inner_type as u32))) {
                        continue;
                    }
                    let name = command::user_message_name(inner_type);
                    let inner_payload = msg.msg_data.unwrap_or_default();
                    events.push(GameEvent {
                        tick,
                        name,
                        msg_type: inner_type as u32,
                        keys: Vec::new(),
                        payload: inner_payload,
                    });
                }
                _ => {
                    // Citadel user messages (300-366) are sent directly in
                    // the packet stream, not wrapped in CSVCMsg_UserMessage.
                    let t = msg_type as i32;
                    let name = if let Ok(e) = CitadelUserMessageIds::try_from(t) {
                        Some(e.as_str_name().to_string())
                    } else if let Ok(e) = ECitadelGameEvents::try_from(t) {
                        Some(e.as_str_name().to_string())
                    } else if let Ok(e) = EBaseUserMessages::try_from(t) {
                        Some(e.as_str_name().to_string())
                    } else {
                        None
                    };
                    if let Some(name) = name {
                        events.push(GameEvent {
                            tick,
                            name,
                            msg_type,
                            keys: Vec::new(),
                            payload: msg_data.to_vec(),
                        });
                    }
                }
            }
        }

        Ok(())
    }
}
