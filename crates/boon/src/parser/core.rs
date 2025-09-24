use snap::raw::Decoder;
use std::borrow::Cow;
use std::convert::TryFrom;
use std::path::Path;

use super::ParserError;
use crate::reader::Reader;
use boon_proto::generated as pb;
use prost::Message;

const MAGIC: [u8; 8] = *b"PBDEMS2\0";
const MINIMUM_SIZE: usize = 16;

#[derive(Debug, Clone)]
pub struct Parser {
    buffer: Vec<u8>, // own the bytes
}

#[derive(Debug, Clone, Default)]
pub struct DemoMetadata {
    pub header: Option<pb::CDemoFileHeader>,
    pub info: Option<pb::CDemoFileInfo>,
}

impl Parser {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, ParserError> {
        let buffer = std::fs::read(path)?; // own it here
        Ok(Self { buffer })
    }

    #[inline]
    fn is_compressed(cmd_raw: u32) -> bool {
        let flag = pb::EDemoCommands::DemIsCompressed as u32;
        (cmd_raw & flag) != 0
    }
    #[inline]
    fn strip_compressed_flag(cmd_raw: u32) -> i32 {
        let flag = pb::EDemoCommands::DemIsCompressed as u32;
        (cmd_raw & !flag) as i32
    }
    #[inline]
    fn decompress_snappy(&self, data: &[u8]) -> Result<Vec<u8>, ParserError> {
        Decoder::new()
            .decompress_vec(data)
            .map_err(|e| ParserError::Decompression(e.to_string()))
    }

    /// Verify magic + prologue
    pub fn verify(&self) -> Result<(), ParserError> {
        if self.buffer.len() < MINIMUM_SIZE {
            return Err(ParserError::TooSmall(self.buffer.len()));
        }

        let header = &self.buffer[..8];
        if header != MAGIC {
            let mut found = [0u8; 8];
            found.copy_from_slice(header);
            return Err(ParserError::WrongMagic(found));
        }
        Ok(())
    }

    fn reader_after_header(&self) -> Result<Reader<'_>, ParserError> {
        self.verify()?;
        let mut r = Reader::new(&self.buffer);
        r.align_to_byte();
        r.skip_bytes(16)?; // Skips Magic + Prologue
        Ok(r)
    }

    pub fn parse_metadata(&self) -> Result<DemoMetadata, ParserError> {
        let mut r = self.reader_after_header()?;

        let mut meta = DemoMetadata::default();

        while let Some((cmd_raw, _tick, size)) = r.read_message_header()? {
            let compressed = Self::is_compressed(cmd_raw);
            let cmd = Self::strip_compressed_flag(cmd_raw);

            let data = r.read_bytes(size);
            let payload: Cow<[u8]> = if compressed {
                Cow::Owned(self.decompress_snappy(&data)?)
            } else {
                Cow::Borrowed(&data)
            };

            if let Ok(kind) = pb::EDemoCommands::try_from(cmd) {
                match kind {
                    pb::EDemoCommands::DemFileHeader if meta.header.is_none() => {
                        meta.header = Some(pb::CDemoFileHeader::decode(payload.as_ref())?);
                    }
                    pb::EDemoCommands::DemFileInfo if meta.info.is_none() => {
                        meta.info = Some(pb::CDemoFileInfo::decode(payload.as_ref())?);
                    }
                    _ => {}
                }
            }
        }

        Ok(meta)
    }

    pub fn scan_messages(&self) -> Result<Vec<(i32, i32, u32, bool)>, ParserError> {
        let mut r = self.reader_after_header()?;

        let mut out = Vec::new();
        while let Some((cmd_raw, tick, size)) = r.read_message_header()? {
            let compressed = Self::is_compressed(cmd_raw);
            let cmd = Self::strip_compressed_flag(cmd_raw);
            out.push((cmd, tick as i32, size, compressed));
            r.align_to_byte();
            let _ = r.skip_bytes(size as usize);
        }
        Ok(out)
    }

    pub fn scan_packet_events(&self) -> Result<Vec<(i32, String)>, ParserError> {
        let mut r = self.reader_after_header()?;

        let mut out: Vec<(i32, String)> = Vec::new();
        while let Some((cmd_raw, tick, size)) = r.read_message_header()? {
            let compressed = Self::is_compressed(cmd_raw);
            let cmd = Self::strip_compressed_flag(cmd_raw);

            let data = r.read_bytes(size);
            let payload: Cow<[u8]> = if compressed {
                Cow::Owned(self.decompress_snappy(&data)?)
            } else {
                Cow::Borrowed(&data)
            };

            if let Ok(kind) = pb::EDemoCommands::try_from(cmd) {
                match kind {
                    pb::EDemoCommands::DemPacket => {
                        let packet = pb::CDemoPacket::decode(payload.as_ref())?;
                        let events = Self::get_packet_events(&packet)?;
                        out.extend(events.into_iter().map(|name| (tick as i32, name)));
                    }
                    pb::EDemoCommands::DemFullPacket => {
                        // Full packet wraps an optional CDemoPacket; decode that first
                        let full = pb::CDemoFullPacket::decode(payload.as_ref())?;
                        if let Some(packet) = full.packet {
                            let events = Self::get_packet_events(&packet)?;
                            out.extend(events.into_iter().map(|name| (tick as i32, name)));
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(out)
    }

    #[inline]
    pub fn get_packet_events(dem_packet: &pb::CDemoPacket) -> Result<Vec<String>, ParserError> {
        let data: &[u8] = dem_packet
            .data
            .as_deref()
            .ok_or_else(|| ParserError::Decode("CDemoPacket.data missing".into()))?;

        let mut r = Reader::new(data);
        let mut detected_events: Vec<String> = Vec::new();

        while r.bytes_remaining() != 0 {
            let msg_type = r.read_ubit_var() as i32;
            let msg_size = r.read_var_u32();
            let _msg_buf = r.read_bytes(msg_size);

            // Convert each recognized enum to a readable name
            // prost::Enumeration gives as_str_name()
            if let Ok(msg) = pb::CitadelUserMessageIds::try_from(msg_type) {
                detected_events.push(msg.as_str_name().to_string());
            } else if let Ok(msg) = pb::ECitadelGameEvents::try_from(msg_type) {
                detected_events.push(msg.as_str_name().to_string());
            } else if let Ok(msg) = pb::SvcMessages::try_from(msg_type) {
                detected_events.push(msg.as_str_name().to_string());
            } else if let Ok(msg) = pb::EBaseUserMessages::try_from(msg_type) {
                detected_events.push(msg.as_str_name().to_string());
            } else if let Ok(msg) = pb::EBaseGameEvents::try_from(msg_type) {
                detected_events.push(msg.as_str_name().to_string());
            } else if let Ok(msg) = pb::NetMessages::try_from(msg_type) {
                detected_events.push(msg.as_str_name().to_string());
            }
        }

        Ok(detected_events)
    }

    pub fn scan_kill_events(&self) -> Result<(), ParserError> {
        let mut r = self.reader_after_header()?;

        while let Some((cmd_raw, tick, size)) = r.read_message_header()? {
            let compressed = Self::is_compressed(cmd_raw);
            let cmd = Self::strip_compressed_flag(cmd_raw);

            let data = r.read_bytes(size);
            let payload: Cow<[u8]> = if compressed {
                Cow::Owned(self.decompress_snappy(&data)?)
            } else {
                Cow::Borrowed(&data)
            };

            if let Ok(kind) = pb::EDemoCommands::try_from(cmd) {
                match kind {
                    pb::EDemoCommands::DemPacket => {
                        let packet = pb::CDemoPacket::decode(payload.as_ref())?;
                        Self::get_kill_events(&packet)?;
                    }
                    pb::EDemoCommands::DemFullPacket => {
                        // Full packet wraps an optional CDemoPacket; decode that first
                        let full = pb::CDemoFullPacket::decode(payload.as_ref())?;
                        if let Some(packet) = full.packet {
                            Self::get_kill_events(&packet)?;
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }

    #[inline]
    pub fn get_kill_events(dem_packet: &pb::CDemoPacket) -> Result<(), ParserError> {
        let data: &[u8] = dem_packet
            .data
            .as_deref()
            .ok_or_else(|| ParserError::Decode("CDemoPacket.data missing".into()))?;

        let mut r = Reader::new(data);

        while r.bytes_remaining() != 0 {
            let msg_type = r.read_ubit_var() as i32;
            let msg_size = r.read_var_u32();
            let msg_buf = r.read_bytes(msg_size);

            // Convert each recognized enum to a readable name
            // prost::Enumeration gives as_str_name()
            if let Ok(msg) = pb::CitadelUserMessageIds::try_from(msg_type) {
                if msg == pb::CitadelUserMessageIds::KEUserMsgHeroKilled {
                    // Decode the payload into your prost-generated type
                    match pb::CCitadelUserMsgHeroKilled::decode(msg_buf.as_ref()) {
                        Ok(ev) => {
                            println!(
                                "kill: victim={:?} attacker={:?} assisters={:?} scorer={:?} respawn_reason={:?} victim_team={:?}",
                                ev.entindex_victim,
                                ev.entindex_attacker,
                                ev.entindex_assisters,
                                ev.entindex_scorer,
                                ev.respawn_reason,
                                ev.victim_team_number,
                            );
                        }
                        Err(e) => eprintln!("failed to decode CCitadelUserMsgHeroKilled: {e}"),
                    }
                }
            }
        }

        Ok(())
    }
}
