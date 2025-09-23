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
        let flag = pb::EDemoCommands::DemIsCompressed as i32;
        ((cmd_raw as i32) & flag) != 0
    }
    #[inline]
    fn strip_compressed_flag(cmd_raw: u32) -> i32 {
        let flag = pb::EDemoCommands::DemIsCompressed as i32;
        (cmd_raw as i32) & !flag
    }
    #[inline]
    fn decompress_snappy(&self, data: &[u8]) -> Result<Vec<u8>, ParserError> {
        Decoder::new()
            .decompress_vec(data)
            .map_err(|e| ParserError::Decompression(e.to_string()))
    }

    /// Verify magic + prologue
    pub fn verify(&self) -> Result<(), ParserError> {
        let mut r = Reader::new(&self.buffer); // local reader borrowing self.buffer

        if r.bytes_remaining() < MINIMUM_SIZE {
            return Err(ParserError::TooSmall(r.bytes_remaining()));
        }

        let magic = r.read_bytes(8);
        if magic.as_slice() != MAGIC {
            // Convert to [u8;8] only for error payload
            let found: [u8; 8] = magic
                .clone()
                .try_into()
                .map_err(|_| ParserError::TooSmall(magic.len()))?;
            return Err(ParserError::WrongMagic(found));
        }

        // skip prologue (8 more bytes)
        let _prologue = r.read_bytes(8);
        Ok(())
    }

    pub fn parse_metadata(&self) -> Result<DemoMetadata, ParserError> {
        self.verify()?; // checks file
        let mut r = Reader::new(&self.buffer);
        r.align_to_byte();
        let _ = r.skip_bytes(16); // magic + prologue

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

    pub fn scan(&self) -> Result<Vec<(i32, i32, u32, bool)>, ParserError> {
        self.verify()?;
        let mut r = Reader::new(&self.buffer);
        r.align_to_byte();
        let _ = r.skip_bytes(16); // magic + prologue

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

    pub fn test_parse(&self) -> Result<(), ParserError> {
        self.verify()?;
        let mut r = Reader::new(&self.buffer);
        r.align_to_byte();
        let _ = r.skip_bytes(16);

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
                    pb::EDemoCommands::DemPacket => {
                        let packet = pb::CDemoPacket::decode(payload.as_ref())?;
                        Self::parse_dem_packet(&packet)?;
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }

    #[inline]
    fn parse_dem_packet(dem_packet: &pb::CDemoPacket) -> Result<(), ParserError> {
        // Option<Vec<u8>> -> &[u8]
        let data: &[u8] = dem_packet
            .data
            .as_deref() // Option<&[u8]>
            .ok_or_else(|| ParserError::Decode("CDemoPacket.data missing".into()))?;

        let mut r = Reader::new(data);

        while r.bytes_remaining() != 0 {
            let msg_type = r.read_ubit_var() as i32;
            let msg_size = r.read_var_u32();
            let _msg_buf = r.read_bytes(msg_size);

            println!("Got id {}, size {}", msg_type, msg_size);

            if let Ok(msg) = pb::CitadelUserMessageIds::try_from(msg_type) {
                println!("Received {:#?}", msg);
                continue;
            } else if let Ok(msg) = pb::ECitadelGameEvents::try_from(msg_type) {
                println!("Received {:#?}", msg);
                continue;
            }
        }
        Ok(())
    }
}
