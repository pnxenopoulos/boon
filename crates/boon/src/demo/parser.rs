use std::path::Path;

use memmap2::Mmap;
use prost::Message;

use crate::entity::{
    ClassInfo, EntityContainer, FieldDecodeContext, SerializerContainer, StringTableContainer,
};
use crate::error::{Error, Result};
use crate::io::{BitReader, ByteReader};

use std::collections::HashMap;

use super::command::{self, CmdHeader, dem, ge, svc};

use boon_proto::proto::{
    CDemoClassInfo, CDemoFileHeader, CDemoFileInfo, CDemoFullPacket, CDemoPacket, CDemoSendTables,
    CMsgSource1LegacyGameEvent, CMsgSource1LegacyGameEventList, CitadelUserMessageIds,
    CsvcMsgCreateStringTable, CsvcMsgPacketEntities, CsvcMsgServerInfo, CsvcMsgUpdateStringTable,
    CsvcMsgUserMessage, EBaseUserMessages, ECitadelGameEvents,
};

const MAGIC: &[u8; 8] = b"PBDEMS2\0";
const HEADER_SIZE: usize = 16; // 8 magic + 4 fileinfo_offset + 4 spawngroups_offset

const DEFAULT_TICK_INTERVAL: f32 = 1.0 / 30.0;
const DEFAULT_FULL_PACKET_INTERVAL: i32 = 1800;

/// Information about a demo message in the command stream.
#[derive(Debug, Clone)]
pub struct MessageInfo {
    pub index: usize,
    pub cmd: i32,
    pub cmd_name: String,
    pub tick: i32,
    pub compressed: bool,
    pub body_size: u32,
    pub offset: usize,
}

/// Parsed demo file header.
#[derive(Debug, Clone)]
pub struct DemoHeader {}

/// Full parser context after initialization.
pub struct Context {
    pub serializers: SerializerContainer,
    pub class_info: ClassInfo,
    pub string_tables: StringTableContainer,
    pub entities: EntityContainer,
    pub tick_interval: f32,
    pub full_packet_interval: i32,
    pub tick: i32,
}

/// A game event extracted from the demo.
#[derive(Debug, Clone)]
pub struct GameEvent {
    pub tick: i32,
    pub name: String,
    pub msg_type: u32,
    pub keys: Vec<(String, String)>,
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

/// The main parser. Owns the memory-mapped demo file.
pub struct Parser {
    mmap: Mmap,
}

impl Parser {
    /// Open a demo file and memory-map it.
    pub fn from_file(path: &Path) -> Result<Self> {
        let file = std::fs::File::open(path)?;
        let mmap = unsafe { Mmap::map(&file)? };
        Ok(Self { mmap })
    }

    /// Verify magic bytes.
    pub fn verify(&self) -> Result<DemoHeader> {
        if self.mmap.len() < HEADER_SIZE {
            return Err(Error::Parse {
                context: "file too small for demo header".into(),
            });
        }

        let mut magic = [0u8; 8];
        magic.copy_from_slice(&self.mmap[0..8]);
        if &magic != MAGIC {
            return Err(Error::InvalidMagic { got: magic });
        }

        Ok(DemoHeader {})
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
        let data = &self.mmap[HEADER_SIZE..];
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
        let data = &self.mmap[HEADER_SIZE..];
        let mut reader = ByteReader::new(data);
        let mut body_buf = Vec::with_capacity(2 * 1024 * 1024);

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
        let fileinfo_offset =
            u32::from_le_bytes([self.mmap[8], self.mmap[9], self.mmap[10], self.mmap[11]]) as usize;

        let data = &self.mmap[HEADER_SIZE..];
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
        self.verify()?;
        let data = &self.mmap[HEADER_SIZE..];
        let mut reader = ByteReader::new(data);
        let mut body_buf = Vec::with_capacity(2 * 1024 * 1024);
        let mut packet_buf = vec![0u8; 2 * 1024 * 1024];
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
    ) -> Result<()> {
        let mut br = BitReader::new(pkt_data);

        while br.bits_remaining() > 8 {
            let msg_type = br.read_ubitvar()?;
            let size = br.read_uvarint32()? as usize;

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

    /// Parse send tables from DEM_SendTables command.
    pub fn parse_send_tables(&self) -> Result<SerializerContainer> {
        self.verify()?;
        let data = &self.mmap[HEADER_SIZE..];
        let mut reader = ByteReader::new(data);
        let mut body_buf = Vec::with_capacity(2 * 1024 * 1024);

        while reader.remaining() > 0 {
            let header = Self::read_cmd_header(&mut reader)?;

            if header.cmd == dem::SEND_TABLES {
                Self::read_cmd_body(&mut reader, &header, &mut body_buf)?;
                let cmd = CDemoSendTables::decode(&body_buf[..])?;
                return SerializerContainer::parse(cmd);
            }

            if header.cmd == dem::STOP || header.cmd == dem::SYNC_TICK {
                break;
            }

            reader.skip(header.body_size as usize)?;
        }

        Err(Error::Parse {
            context: "DEM_SendTables not found".into(),
        })
    }

    /// Parse class info from DEM_ClassInfo command.
    pub fn parse_class_info(&self) -> Result<ClassInfo> {
        self.verify()?;
        let data = &self.mmap[HEADER_SIZE..];
        let mut reader = ByteReader::new(data);
        let mut body_buf = Vec::with_capacity(2 * 1024 * 1024);

        while reader.remaining() > 0 {
            let header = Self::read_cmd_header(&mut reader)?;

            if header.cmd == dem::CLASS_INFO {
                Self::read_cmd_body(&mut reader, &header, &mut body_buf)?;
                let cmd = CDemoClassInfo::decode(&body_buf[..])?;
                return Ok(ClassInfo::parse(cmd));
            }

            if header.cmd == dem::STOP || header.cmd == dem::SYNC_TICK {
                break;
            }

            reader.skip(header.body_size as usize)?;
        }

        Err(Error::Parse {
            context: "DEM_ClassInfo not found".into(),
        })
    }

    /// Parse all initialization data up to DEM_SyncTick and return a Context.
    pub fn parse_init(&self) -> Result<Context> {
        self.verify()?;
        let data = &self.mmap[HEADER_SIZE..];
        let mut reader = ByteReader::new(data);

        let mut packet_buf = vec![0u8; 2 * 1024 * 1024];
        let mut body_buf = Vec::with_capacity(2 * 1024 * 1024);

        let mut serializers: Option<SerializerContainer> = None;
        let mut class_info: Option<ClassInfo> = None;
        let mut string_tables = StringTableContainer::new();
        let mut tick_interval: f32 = 0.0;
        let mut full_packet_interval: i32 = DEFAULT_FULL_PACKET_INTERVAL;

        while reader.remaining() > 0 {
            let header = Self::read_cmd_header(&mut reader)?;

            if header.cmd == dem::SYNC_TICK {
                reader.skip(header.body_size as usize)?;
                break;
            }

            if header.cmd == dem::STOP {
                break;
            }

            Self::read_cmd_body(&mut reader, &header, &mut body_buf)?;

            match header.cmd {
                dem::SEND_TABLES => {
                    let cmd = CDemoSendTables::decode(&body_buf[..])?;
                    serializers = Some(SerializerContainer::parse(cmd)?);
                }
                dem::CLASS_INFO => {
                    let cmd = CDemoClassInfo::decode(&body_buf[..])?;
                    class_info = Some(ClassInfo::parse(cmd));
                }
                dem::PACKET | dem::SIGNON_PACKET => {
                    let cmd = CDemoPacket::decode(&body_buf[..])?;
                    let pkt_data = cmd.data.unwrap_or_default();
                    Self::process_packet_for_init(
                        &pkt_data,
                        &mut string_tables,
                        &mut tick_interval,
                        &mut full_packet_interval,
                        &mut packet_buf,
                    )?;
                }
                _ => {}
            }
        }

        let serializers = serializers.ok_or_else(|| Error::Parse {
            context: "DEM_SendTables not found during init".into(),
        })?;
        let class_info = class_info.ok_or_else(|| Error::Parse {
            context: "DEM_ClassInfo not found during init".into(),
        })?;

        // Update instance baselines
        string_tables.update_instance_baselines(&class_info);

        Ok(Context {
            serializers,
            class_info,
            string_tables,
            entities: EntityContainer::new(),
            tick_interval,
            full_packet_interval,
            tick: -1,
        })
    }

    /// Process a packet's inner messages during initialization (string tables, server info).
    fn process_packet_for_init(
        pkt_data: &[u8],
        string_tables: &mut StringTableContainer,
        tick_interval: &mut f32,
        full_packet_interval: &mut i32,
        packet_buf: &mut Vec<u8>,
    ) -> Result<()> {
        let mut br = BitReader::new(pkt_data);

        while br.bits_remaining() > 8 {
            let msg_type = br.read_ubitvar()?;
            let size = br.read_uvarint32()? as usize;

            // Read the message body
            if size > packet_buf.len() {
                packet_buf.resize(size, 0);
            }
            br.read_bytes(&mut packet_buf[..size])?;
            let msg_data = &packet_buf[..size];

            match msg_type {
                svc::CREATE_STRING_TABLE => {
                    let msg = CsvcMsgCreateStringTable::decode(msg_data)?;
                    string_tables.handle_create(msg)?;
                }
                svc::UPDATE_STRING_TABLE => {
                    let msg = CsvcMsgUpdateStringTable::decode(msg_data)?;
                    string_tables.handle_update(msg)?;
                }
                svc::SERVER_INFO => {
                    let msg = CsvcMsgServerInfo::decode(msg_data)?;
                    if let Some(ti) = msg.tick_interval {
                        *tick_interval = ti;
                        let ratio = DEFAULT_TICK_INTERVAL / ti;
                        *full_packet_interval = DEFAULT_FULL_PACKET_INTERVAL * ratio as i32;
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Parse the demo to a specific tick, returning the full game state.
    pub fn parse_to_tick(&self, target_tick: i32) -> Result<Context> {
        let mut ctx = self.parse_init()?;
        let data = &self.mmap[HEADER_SIZE..];
        let mut reader = ByteReader::new(data);

        let mut packet_buf = vec![0u8; 2 * 1024 * 1024];
        let mut body_buf = Vec::with_capacity(2 * 1024 * 1024);
        let mut fp_buf = Vec::with_capacity(256);
        let mut field_decode_ctx = FieldDecodeContext::new(ctx.tick_interval);

        // Skip past init (up to and including SyncTick)
        let mut past_sync = false;
        while reader.remaining() > 0 {
            let header = Self::read_cmd_header(&mut reader)?;
            if header.cmd == dem::SYNC_TICK {
                reader.skip(header.body_size as usize)?;
                past_sync = true;
                break;
            }
            if header.cmd == dem::STOP {
                return Ok(ctx);
            }
            reader.skip(header.body_size as usize)?;
        }

        if !past_sync {
            return Ok(ctx);
        }

        // Track whether we've handled the last full packet before target
        let mut did_handle_last_full_packet = false;

        while reader.remaining() > 0 {
            let header = Self::read_cmd_header(&mut reader)?;

            if header.tick > target_tick && header.cmd != dem::STOP {
                break;
            }

            ctx.tick = header.tick;

            if header.cmd == dem::STOP {
                break;
            }

            let is_full_packet = header.cmd == dem::FULL_PACKET;
            let distance = target_tick - header.tick;
            let has_full_packet_ahead = distance > ctx.full_packet_interval + 100;

            if is_full_packet {
                Self::read_cmd_body(&mut reader, &header, &mut body_buf)?;
                let cmd = CDemoFullPacket::decode(&body_buf[..])?;

                // Handle string tables from full packet
                if let Some(st) = cmd.string_table {
                    ctx.string_tables.do_full_update(st);
                    ctx.string_tables.update_instance_baselines(&ctx.class_info);
                }

                // Handle packet from full packet (skip if more full packets ahead)
                if !has_full_packet_ahead {
                    if let Some(packet) = cmd.packet {
                        let pkt_data = packet.data.unwrap_or_default();
                        Self::process_packet_entities(
                            &pkt_data,
                            &mut ctx,
                            &mut field_decode_ctx,
                            &mut packet_buf,
                            &mut fp_buf,
                        )?;
                    }
                    did_handle_last_full_packet = true;
                }

                continue;
            }

            if !did_handle_last_full_packet {
                reader.skip(header.body_size as usize)?;
                continue;
            }

            Self::read_cmd_body(&mut reader, &header, &mut body_buf)?;

            match header.cmd {
                dem::PACKET | dem::SIGNON_PACKET => {
                    let cmd = CDemoPacket::decode(&body_buf[..])?;
                    let pkt_data = cmd.data.unwrap_or_default();
                    Self::process_packet_entities(
                        &pkt_data,
                        &mut ctx,
                        &mut field_decode_ctx,
                        &mut packet_buf,
                        &mut fp_buf,
                    )?;
                }
                _ => {}
            }
        }

        Ok(ctx)
    }

    /// Parse the entire demo, calling a callback at each tick with the current context.
    /// This is more efficient than calling parse_to_tick repeatedly.
    pub fn run_to_end<F>(&self, mut on_tick: F) -> Result<()>
    where
        F: FnMut(&Context),
    {
        let mut ctx = self.parse_init()?;
        let data = &self.mmap[HEADER_SIZE..];
        let mut reader = ByteReader::new(data);

        let mut packet_buf = vec![0u8; 2 * 1024 * 1024];
        let mut body_buf = Vec::with_capacity(2 * 1024 * 1024);
        let mut fp_buf = Vec::with_capacity(256);
        let mut field_decode_ctx = FieldDecodeContext::new(ctx.tick_interval);

        // Skip past init (up to and including SyncTick)
        while reader.remaining() > 0 {
            let header = Self::read_cmd_header(&mut reader)?;
            if header.cmd == dem::SYNC_TICK {
                reader.skip(header.body_size as usize)?;
                break;
            }
            if header.cmd == dem::STOP {
                return Ok(());
            }
            reader.skip(header.body_size as usize)?;
        }

        let mut last_tick: i32 = -1;

        while reader.remaining() > 0 {
            let header = Self::read_cmd_header(&mut reader)?;

            // Call callback when tick changes
            if header.tick != last_tick && last_tick >= 0 {
                on_tick(&ctx);
            }
            last_tick = header.tick;
            ctx.tick = header.tick;

            if header.cmd == dem::STOP {
                // Final callback
                if last_tick >= 0 {
                    on_tick(&ctx);
                }
                break;
            }

            Self::read_cmd_body(&mut reader, &header, &mut body_buf)?;

            match header.cmd {
                dem::FULL_PACKET => {
                    let cmd = CDemoFullPacket::decode(&body_buf[..])?;

                    if let Some(st) = cmd.string_table {
                        ctx.string_tables.do_full_update(st);
                        ctx.string_tables.update_instance_baselines(&ctx.class_info);
                    }

                    if let Some(packet) = cmd.packet {
                        let pkt_data = packet.data.unwrap_or_default();
                        Self::process_packet_entities(
                            &pkt_data,
                            &mut ctx,
                            &mut field_decode_ctx,
                            &mut packet_buf,
                            &mut fp_buf,
                        )?;
                    }
                }
                dem::PACKET | dem::SIGNON_PACKET => {
                    let cmd = CDemoPacket::decode(&body_buf[..])?;
                    let pkt_data = cmd.data.unwrap_or_default();
                    Self::process_packet_entities(
                        &pkt_data,
                        &mut ctx,
                        &mut field_decode_ctx,
                        &mut packet_buf,
                        &mut fp_buf,
                    )?;
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Parse the entire demo with entity class filtering.
    /// Only entities with classes in the filter are fully tracked.
    /// This is much faster when you only need specific entity types.
    pub fn run_to_end_filtered<F>(
        &self,
        class_filter: &std::collections::HashSet<&str>,
        mut on_tick: F,
    ) -> Result<()>
    where
        F: FnMut(&Context),
    {
        let mut ctx = self.parse_init()?;
        let data = &self.mmap[HEADER_SIZE..];
        let mut reader = ByteReader::new(data);

        let mut packet_buf = vec![0u8; 2 * 1024 * 1024];
        let mut body_buf = Vec::with_capacity(2 * 1024 * 1024);
        let mut fp_buf = Vec::with_capacity(256);
        let mut field_decode_ctx = FieldDecodeContext::new(ctx.tick_interval);

        // Skip past init (up to and including SyncTick)
        while reader.remaining() > 0 {
            let header = Self::read_cmd_header(&mut reader)?;
            if header.cmd == dem::SYNC_TICK {
                reader.skip(header.body_size as usize)?;
                break;
            }
            if header.cmd == dem::STOP {
                return Ok(());
            }
            reader.skip(header.body_size as usize)?;
        }

        let mut last_tick: i32 = -1;

        while reader.remaining() > 0 {
            let header = Self::read_cmd_header(&mut reader)?;

            // Call callback when tick changes
            if header.tick != last_tick && last_tick >= 0 {
                on_tick(&ctx);
            }
            last_tick = header.tick;
            ctx.tick = header.tick;

            if header.cmd == dem::STOP {
                // Final callback
                if last_tick >= 0 {
                    on_tick(&ctx);
                }
                break;
            }

            Self::read_cmd_body(&mut reader, &header, &mut body_buf)?;

            match header.cmd {
                dem::FULL_PACKET => {
                    let cmd = CDemoFullPacket::decode(&body_buf[..])?;

                    if let Some(st) = cmd.string_table {
                        ctx.string_tables.do_full_update(st);
                        ctx.string_tables.update_instance_baselines(&ctx.class_info);
                    }

                    if let Some(packet) = cmd.packet {
                        let pkt_data = packet.data.unwrap_or_default();
                        Self::process_packet_entities_filtered(
                            &pkt_data,
                            &mut ctx,
                            &mut field_decode_ctx,
                            &mut packet_buf,
                            class_filter,
                            &mut fp_buf,
                        )?;
                    }
                }
                dem::PACKET | dem::SIGNON_PACKET => {
                    let cmd = CDemoPacket::decode(&body_buf[..])?;
                    let pkt_data = cmd.data.unwrap_or_default();
                    Self::process_packet_entities_filtered(
                        &pkt_data,
                        &mut ctx,
                        &mut field_decode_ctx,
                        &mut packet_buf,
                        class_filter,
                        &mut fp_buf,
                    )?;
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Parse the entire demo with entity class filtering AND event collection.
    /// Combines `run_to_end_filtered` with `process_packet_events` in a single pass.
    /// The callback receives both the entity context and accumulated events for the tick.
    pub fn run_to_end_with_events_filtered<F>(
        &self,
        class_filter: &std::collections::HashSet<&str>,
        mut on_tick: F,
    ) -> Result<()>
    where
        F: FnMut(&Context, &[GameEvent]),
    {
        let mut ctx = self.parse_init()?;
        let data = &self.mmap[HEADER_SIZE..];
        let mut reader = ByteReader::new(data);

        let mut packet_buf = vec![0u8; 2 * 1024 * 1024];
        let mut event_packet_buf = vec![0u8; 2 * 1024 * 1024];
        let mut body_buf = Vec::with_capacity(2 * 1024 * 1024);
        let mut fp_buf = Vec::with_capacity(256);
        let mut field_decode_ctx = FieldDecodeContext::new(ctx.tick_interval);

        let mut descriptors: HashMap<i32, EventDescriptor> = HashMap::new();
        let mut tick_events: Vec<GameEvent> = Vec::new();

        // Skip past init (up to and including SyncTick)
        while reader.remaining() > 0 {
            let header = Self::read_cmd_header(&mut reader)?;
            if header.cmd == dem::SYNC_TICK {
                reader.skip(header.body_size as usize)?;
                break;
            }
            if header.cmd == dem::STOP {
                return Ok(());
            }
            reader.skip(header.body_size as usize)?;
        }

        let mut last_tick: i32 = -1;

        while reader.remaining() > 0 {
            let header = Self::read_cmd_header(&mut reader)?;

            // Call callback when tick changes
            if header.tick != last_tick && last_tick >= 0 {
                on_tick(&ctx, &tick_events);
                tick_events.clear();
            }
            last_tick = header.tick;
            ctx.tick = header.tick;

            if header.cmd == dem::STOP {
                // Final callback
                if last_tick >= 0 {
                    on_tick(&ctx, &tick_events);
                }
                break;
            }

            Self::read_cmd_body(&mut reader, &header, &mut body_buf)?;

            match header.cmd {
                dem::FULL_PACKET => {
                    let cmd = CDemoFullPacket::decode(&body_buf[..])?;

                    if let Some(st) = cmd.string_table {
                        ctx.string_tables.do_full_update(st);
                        ctx.string_tables.update_instance_baselines(&ctx.class_info);
                    }

                    if let Some(packet) = cmd.packet {
                        let pkt_data = packet.data.unwrap_or_default();
                        Self::process_packet_entities_filtered(
                            &pkt_data,
                            &mut ctx,
                            &mut field_decode_ctx,
                            &mut packet_buf,
                            class_filter,
                            &mut fp_buf,
                        )?;
                        Self::process_packet_events(
                            &pkt_data,
                            header.tick,
                            &mut descriptors,
                            &mut tick_events,
                            &mut event_packet_buf,
                        )?;
                    }
                }
                dem::PACKET | dem::SIGNON_PACKET => {
                    let cmd = CDemoPacket::decode(&body_buf[..])?;
                    let pkt_data = cmd.data.unwrap_or_default();
                    Self::process_packet_entities_filtered(
                        &pkt_data,
                        &mut ctx,
                        &mut field_decode_ctx,
                        &mut packet_buf,
                        class_filter,
                        &mut fp_buf,
                    )?;
                    Self::process_packet_events(
                        &pkt_data,
                        header.tick,
                        &mut descriptors,
                        &mut tick_events,
                        &mut event_packet_buf,
                    )?;
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Process a packet's inner messages for entity updates.
    fn process_packet_entities(
        pkt_data: &[u8],
        ctx: &mut Context,
        field_decode_ctx: &mut FieldDecodeContext,
        packet_buf: &mut Vec<u8>,
        fp_buf: &mut Vec<crate::entity::field_path::FieldPath>,
    ) -> Result<()> {
        let mut br = BitReader::new(pkt_data);

        while br.bits_remaining() > 8 {
            let msg_type = br.read_ubitvar()?;
            let size = br.read_uvarint32()? as usize;

            if size > packet_buf.len() {
                packet_buf.resize(size, 0);
            }
            br.read_bytes(&mut packet_buf[..size])?;
            let msg_data = &packet_buf[..size];

            match msg_type {
                svc::CREATE_STRING_TABLE => {
                    let msg = CsvcMsgCreateStringTable::decode(msg_data)?;
                    ctx.string_tables.handle_create(msg)?;
                    ctx.string_tables.update_instance_baselines(&ctx.class_info);
                }
                svc::UPDATE_STRING_TABLE => {
                    let msg = CsvcMsgUpdateStringTable::decode(msg_data)?;
                    ctx.string_tables.handle_update(msg)?;
                    ctx.string_tables.update_instance_baselines(&ctx.class_info);
                }
                svc::SERVER_INFO => {
                    let msg = CsvcMsgServerInfo::decode(msg_data)?;
                    if let Some(ti) = msg.tick_interval {
                        ctx.tick_interval = ti;
                        field_decode_ctx.tick_interval = ti;
                        let ratio = DEFAULT_TICK_INTERVAL / ti;
                        ctx.full_packet_interval = DEFAULT_FULL_PACKET_INTERVAL * ratio as i32;
                    }
                }
                svc::PACKET_ENTITIES => {
                    let msg = CsvcMsgPacketEntities::decode(msg_data)?;
                    ctx.entities.handle_packet_entities(
                        msg,
                        &ctx.class_info,
                        &ctx.serializers,
                        &ctx.string_tables,
                        field_decode_ctx,
                        fp_buf,
                    )?;
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Process a packet's inner messages with entity class filtering.
    fn process_packet_entities_filtered(
        pkt_data: &[u8],
        ctx: &mut Context,
        field_decode_ctx: &mut FieldDecodeContext,
        packet_buf: &mut Vec<u8>,
        class_filter: &std::collections::HashSet<&str>,
        fp_buf: &mut Vec<crate::entity::field_path::FieldPath>,
    ) -> Result<()> {
        let mut br = BitReader::new(pkt_data);

        while br.bits_remaining() > 8 {
            let msg_type = br.read_ubitvar()?;
            let size = br.read_uvarint32()? as usize;

            if size > packet_buf.len() {
                packet_buf.resize(size, 0);
            }
            br.read_bytes(&mut packet_buf[..size])?;
            let msg_data = &packet_buf[..size];

            match msg_type {
                svc::CREATE_STRING_TABLE => {
                    let msg = CsvcMsgCreateStringTable::decode(msg_data)?;
                    ctx.string_tables.handle_create(msg)?;
                    ctx.string_tables.update_instance_baselines(&ctx.class_info);
                }
                svc::UPDATE_STRING_TABLE => {
                    let msg = CsvcMsgUpdateStringTable::decode(msg_data)?;
                    ctx.string_tables.handle_update(msg)?;
                    ctx.string_tables.update_instance_baselines(&ctx.class_info);
                }
                svc::SERVER_INFO => {
                    let msg = CsvcMsgServerInfo::decode(msg_data)?;
                    if let Some(ti) = msg.tick_interval {
                        ctx.tick_interval = ti;
                        field_decode_ctx.tick_interval = ti;
                        let ratio = DEFAULT_TICK_INTERVAL / ti;
                        ctx.full_packet_interval = DEFAULT_FULL_PACKET_INTERVAL * ratio as i32;
                    }
                }
                svc::PACKET_ENTITIES => {
                    let msg = CsvcMsgPacketEntities::decode(msg_data)?;
                    ctx.entities.handle_packet_entities_filtered(
                        msg,
                        &ctx.class_info,
                        &ctx.serializers,
                        &ctx.string_tables,
                        field_decode_ctx,
                        class_filter,
                        fp_buf,
                    )?;
                }
                _ => {}
            }
        }

        Ok(())
    }
}
