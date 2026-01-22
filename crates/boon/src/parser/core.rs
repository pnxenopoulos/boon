use std::{cell::RefCell, path::Path};

use boon_proto::generated as pb;
use prost::Message;
use snap::raw::Decoder;

use crate::parser::error::ParserError;
use crate::parser::sendtables::{parse_sendtables, SerializerRegistry as SendTableRegistry};
use crate::parser::stringtables::{BaselineRegistry, StringTableRegistry};
use crate::reader::{ReadError, Reader};

const MAGIC: [u8; 8] = *b"PBDEMS2\0";
const PROLOGUE_BYTES: usize = 16; // magic (8) + prologue (8)

#[derive(Debug, Clone)]
pub struct Parser {
    bytes: Vec<u8>,
    // Reuse a Snappy decoder; interior mutability avoids &mut self borrow conflicts
    pub(crate) snappy: RefCell<Decoder>, // keep crate-visible for helpers if needed
}

#[derive(Debug, Clone, Default)]
pub struct DemoMetadata {
    pub header: Option<pb::CDemoFileHeader>,
    pub info: Option<pb::CDemoFileInfo>,
}

impl Parser {
    /* ---------- construction ---------- */

    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, ParserError> {
        let p = Self {
            bytes,
            snappy: RefCell::new(Decoder::new()),
        };
        p.verify()?;
        Ok(p)
    }

    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, ParserError> {
        let bytes = std::fs::read(path)?;
        Self::from_bytes(bytes)
    }

    /* ---------- basic checks / reader ---------- */

    /// Basic format sanity.
    pub fn verify(&self) -> Result<(), ParserError> {
        if self.bytes.len() < PROLOGUE_BYTES {
            return Err(ParserError::TooSmall(self.bytes.len()));
        }
        if self.bytes[..8] != MAGIC {
            let mut got = [0u8; 8];
            got.copy_from_slice(&self.bytes[..8]);
            return Err(ParserError::WrongMagic(got));
        }
        Ok(())
    }

    fn reader(&self) -> Result<Reader<'_>, ParserError> {
        self.verify()?;
        let mut r = Reader::new(&self.bytes);
        // starts byte-aligned, but keep this for robustness
        r.align_to_byte()?;
        r.skip_bytes(PROLOGUE_BYTES)?;
        Ok(r)
    }

    /* ---------- bitstream helpers ---------- */

    #[inline]
    fn is_compressed(cmd_raw: u32) -> bool {
        let flag = pb::EDemoCommands::DemIsCompressed as u32;
        (cmd_raw & flag) != 0
    }

    #[inline]
    fn strip_compressed(cmd_raw: u32) -> u32 {
        let flag = pb::EDemoCommands::DemIsCompressed as u32;
        cmd_raw & !flag
    }

    /// Read `size` bytes then optionally Snappy-decompress. Always returns owned bytes.
    fn read_payload<'a>(
        &self,
        r: &mut Reader<'a>,
        size: u32,
        compressed: bool,
    ) -> Result<Vec<u8>, ParserError> {
        let raw = r.read_bytes(size as usize)?;
        if !compressed {
            return Ok(raw);
        }
        self.snappy
            .borrow_mut()
            .decompress_vec(&raw)
            .map_err(|e| ParserError::Decompression(e.to_string()))
    }

    /// Decode a protobuf message from bytes.
    #[inline]
    fn decode<M: Message + Default>(&self, bytes: &[u8]) -> Result<M, ParserError> {
        Ok(M::decode(bytes)?)
    }

    /* ---------- top-level passes ---------- */

    pub fn parse_metadata(&self) -> Result<DemoMetadata, ParserError> {
        let mut r = self.reader()?;
        let mut meta = DemoMetadata::default();

        while let Some((cmd_raw, _tick, size)) = r.read_message_header()? {
            let compressed = Self::is_compressed(cmd_raw);
            let cmd = Self::strip_compressed(cmd_raw);
            let payload = self.read_payload(&mut r, size, compressed)?;

            if let Ok(kind) = pb::EDemoCommands::try_from(cmd as i32) {
                match kind {
                    pb::EDemoCommands::DemFileHeader if meta.header.is_none() => {
                        meta.header = Some(self.decode::<pb::CDemoFileHeader>(&payload)?);
                    }
                    pb::EDemoCommands::DemFileInfo if meta.info.is_none() => {
                        meta.info = Some(self.decode::<pb::CDemoFileInfo>(&payload)?);
                    }
                    _ => {}
                }
            }
        }
        Ok(meta)
    }

    /// Returns (cmd, tick, size, compressed)
    pub fn scan_messages(&self) -> Result<Vec<(u32, u32, u32, bool)>, ParserError> {
        let mut r = self.reader()?;
        let mut out = Vec::new();

        while let Some((cmd_raw, tick, size)) = r.read_message_header()? {
            let compressed = Self::is_compressed(cmd_raw);
            let cmd = Self::strip_compressed(cmd_raw);
            out.push((cmd, tick, size, compressed));
            r.align_to_byte()?;
            r.skip_bytes(size as usize)?;
        }
        Ok(out)
    }

    /// Extract event names present in DemPacket/DemFullPacket messages.
    /// Returns (tick, event_name)
    pub fn scan_packet_events(&self) -> Result<Vec<(u32, String)>, ParserError> {
        let mut r = self.reader()?;
        let mut out: Vec<(u32, String)> = Vec::new();

        while let Some((cmd_raw, tick, size)) = r.read_message_header()? {
            let compressed = Self::is_compressed(cmd_raw);
            let cmd = Self::strip_compressed(cmd_raw);
            let payload = self.read_payload(&mut r, size, compressed)?;

            if let Ok(kind) = pb::EDemoCommands::try_from(cmd as i32) {
                match kind {
                    pb::EDemoCommands::DemPacket => {
                        let packet: pb::CDemoPacket = self.decode(&payload)?;
                        Self::collect_packet_events(&packet, tick, &mut out)?;
                    }
                    pb::EDemoCommands::DemFullPacket => {
                        let full: pb::CDemoFullPacket = self.decode(&payload)?;
                        if let Some(packet) = full.packet {
                            Self::collect_packet_events(&packet, tick, &mut out)?;
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(out)
    }

    fn collect_packet_events(
        dem_packet: &pb::CDemoPacket,
        tick: u32,
        out: &mut Vec<(u32, String)>,
    ) -> Result<(), ParserError> {
        let data = dem_packet
            .data
            .as_deref()
            .ok_or_else(|| ParserError::Decode("CDemoPacket.data missing".into()))?;

        let mut r = Reader::new(data);

        loop {
            // Need the 6-bit UBitVar prefix to start another entry.
            if r.bits_remaining_total() < 6 {
                break;
            }

            // 1) type: UBitVar. Clean EOF => end of packet payload.
            let msg_type = match r.read_ubit_var() {
                Ok(t) => t as i32,
                Err(ReadError::Eof) => break,
                Err(e) => return Err(e.into()),
            };

            // Before size, ensure we have at least one byte available.
            if r.bits_remaining_total() < 8 {
                break;
            }

            // 2) size: varuint32 (bytes). Treat overflow as malformed tail -> stop scanning.
            let msg_size = match r.read_var_u32() {
                Ok(sz) => sz as usize,
                Err(_) => break,
            };

            // Optional sanity: if the claimed size is clearly impossible, stop gracefully.
            let remain_bytes = r.bits_remaining_total() / 8;
            if msg_size > remain_bytes.saturating_add(4) {
                break;
            }

            // 3) payload: byte-aligned by the format.
            match r.read_bytes(msg_size) {
                Ok(_buf) => {
                    let push = |name: &str, out: &mut Vec<(u32, String)>| {
                        out.push((tick, name.to_string()))
                    };

                    if let Ok(m) = pb::CitadelUserMessageIds::try_from(msg_type) {
                        push(m.as_str_name(), out);
                    } else if let Ok(m) = pb::ECitadelGameEvents::try_from(msg_type) {
                        push(m.as_str_name(), out);
                    } else if let Ok(m) = pb::SvcMessages::try_from(msg_type) {
                        push(m.as_str_name(), out);
                    } else if let Ok(m) = pb::EBaseUserMessages::try_from(msg_type) {
                        push(m.as_str_name(), out);
                    } else if let Ok(m) = pb::EBaseGameEvents::try_from(msg_type) {
                        push(m.as_str_name(), out);
                    } else if let Ok(m) = pb::NetMessages::try_from(msg_type) {
                        push(m.as_str_name(), out);
                    }
                }
                // For scanning, treat mid-payload EOF as end-of-packet, not an error.
                Err(ReadError::Eof) => break,
                Err(e) => return Err(e.into()),
            }
        }

        Ok(())
    }

    /* ---------- SendTables / ClassInfo ---------- */

    /// Extract SendTables and ClassInfo and build a `SerializerRegistry`.
    pub fn sendtables(&self) -> Result<SendTableRegistry, ParserError> {
        let mut r = self.reader()?;

        let mut st_bytes: Option<Vec<u8>> = None;
        let mut class_info: Option<pb::CDemoClassInfo> = None;

        while let Some((cmd_raw, _tick, size)) = r.read_message_header()? {
            let compressed = Self::is_compressed(cmd_raw);
            let cmd = Self::strip_compressed(cmd_raw);
            let payload = self.read_payload(&mut r, size, compressed)?;

            if let Ok(kind) = pb::EDemoCommands::try_from(cmd as i32) {
                match kind {
                    pb::EDemoCommands::DemSendTables => {
                        let msg: pb::CDemoSendTables = self.decode(&payload)?;
                        if let Some(data) = msg.data {
                            st_bytes = Some(data);
                        }
                    }
                    pb::EDemoCommands::DemClassInfo => {
                        class_info = Some(self.decode::<pb::CDemoClassInfo>(&payload)?);
                    }
                    _ => {}
                }
            }

            if st_bytes.is_some() && class_info.is_some() {
                break; // early exit once both are present
            }
        }

        let st = st_bytes.ok_or_else(|| ParserError::Decode("no CDemoSendTables found".into()))?;
        let ci = class_info.ok_or_else(|| ParserError::Decode("no CDemoClassInfo found".into()))?;

        parse_sendtables(&st, &ci)
    }

    /// Just return the first CDemoClassInfo found.
    pub fn class_info(&self) -> Result<pb::CDemoClassInfo, ParserError> {
        let mut r = self.reader()?;
        while let Some((cmd_raw, _tick, size)) = r.read_message_header()? {
            let compressed = Self::is_compressed(cmd_raw);
            let cmd = Self::strip_compressed(cmd_raw);
            let payload = self.read_payload(&mut r, size, compressed)?;
            if let Ok(kind) = pb::EDemoCommands::try_from(cmd as i32) {
                if matches!(kind, pb::EDemoCommands::DemClassInfo) {
                    return self.decode::<pb::CDemoClassInfo>(&payload);
                }
            }
        }
        Err(ParserError::Decode("no CDemoClassInfo found".into()))
    }

    /// Convenience alias: clearer call-sites (`parser.load_sendtables()?`)
    #[inline]
    pub fn load_sendtables(&self) -> Result<SendTableRegistry, ParserError> {
        self.sendtables()
    }

    /* ---------- StringTables scanning ---------- */

    /// Scan the demo and return the most recent StringTables snapshot found.
    /// Returns (tick_of_snapshot, registry).
    pub fn stringtables_latest(&self) -> Result<(u32, StringTableRegistry), ParserError> {
        self.stringtables_at_tick(None)
    }

    /// Scan forward and keep the newest CDemoStringTables snapshot seen up to (and including) `target_tick`.
    /// If `target_tick` is None, returns the last snapshot in the file.
    pub fn stringtables_at_tick(
        &self,
        target_tick: Option<u32>,
    ) -> Result<(u32, StringTableRegistry), ParserError> {
        let mut r = self.reader()?;

        let mut last_game_tick: u32 = 0;               // from DemPacket/DemFullPacket
        let mut latest_tick: Option<u32> = None;       // effective tick of last snapshot ≤ target
        let mut latest_reg: Option<StringTableRegistry> = None;

        while let Some((cmd_raw, tick_raw, size)) = r.read_message_header()? {
            let compressed = Self::is_compressed(cmd_raw);
            let cmd = Self::strip_compressed(cmd_raw);
            let payload = self.read_payload(&mut r, size, compressed)?;

            // Sentinel (-1 as u32) appears on out-of-band frames.
            let effective_tick = if tick_raw == u32::MAX { last_game_tick } else { tick_raw };

            // If caller wants a cutoff and we’re already past it, we can stop.
            if let Some(tgt) = target_tick {
                if effective_tick > tgt {
                    break;
                }
            }

            if let Ok(kind) = pb::EDemoCommands::try_from(cmd as i32) {
                match kind {
                    // Advance gameplay tick markers.
                    pb::EDemoCommands::DemPacket => {
                        let _: pb::CDemoPacket = self.decode(&payload)?;
                        last_game_tick = effective_tick;
                    }
                    pb::EDemoCommands::DemFullPacket => {
                        let full: pb::CDemoFullPacket = self.decode(&payload)?;

                        if let Some(st) = full.string_table.as_ref() {
                            let reg = StringTableRegistry::from_demo_snapshot(st)?;
                            latest_tick = Some(effective_tick);
                            latest_reg = Some(reg);
                        }

                        last_game_tick = effective_tick;
                    }

                    // Standalone string-table snapshot (often at sentinel tick).
                    pb::EDemoCommands::DemStringTables => {
                        let snap: pb::CDemoStringTables = self.decode(&payload)?;
                        let reg = StringTableRegistry::from_demo_snapshot(&snap)?;
                        latest_tick = Some(effective_tick);
                        latest_reg = Some(reg);
                    }

                    _ => {}
                }
            }
        }

        match (latest_tick, latest_reg) {
            (Some(t), Some(reg)) => Ok((t, reg)),
            _ => Err(ParserError::Decode("no string-table snapshot found".into())),
        }
    }

    /* ---------- Helpers that use Snappy internally ---------- */

    /// Build baselines from a given StringTable snapshot using the parser's Snappy decoder.
    pub fn build_instance_baselines(
        &self,
        st: &StringTableRegistry,
    ) -> Result<BaselineRegistry, ParserError> {
        st.build_instance_baselines(&self.snappy)
    }
}