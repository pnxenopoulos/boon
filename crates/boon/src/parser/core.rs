//! Minimal demo parser:
//! - verify: checks magic and advances past 8-byte prologue
//! - prologue: consume messages until CDemoSyncTick; store CDemoFileHeader if seen.

use std::borrow::Cow;
use std::convert::TryFrom;
use std::path::Path;

use super::ParserError;
use crate::reader::Reader;

// Generated prost types
use boon_proto::generated as pb;
use prost::Message;

/// The expected 8-byte magic header: `b"PBDEMS2\0"`.
const MAGIC: [u8; 8] = *b"PBDEMS2\0";
const MINIMUM_SIZE: usize = 16;

#[derive(Debug, Clone)]
pub struct Parser {
    bytes: Vec<u8>,
}

/// Plain, prost-free data the CLI can print
#[derive(Debug, Clone, Default)]
pub struct PrologueMeta {
    pub header: Option<pb::CDemoFileHeader>,
    pub info: Option<pb::CDemoFileInfo>,
}

impl Parser {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, ParserError> {
        Ok(Self { bytes: std::fs::read(path)? })
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

    pub fn verify(&self) -> Result<(), ParserError> {
        if self.bytes.len() < MINIMUM_SIZE {
            return Err(ParserError::TooSmall(self.bytes.len()));
        }
        let mut r = Reader::new(&self.bytes);
        let magic: [u8; 8] = r.read_slice(8)?.try_into().unwrap();
        if magic != MAGIC {
            return Err(ParserError::WrongMagic(magic));
        }
        let _prologue8: [u8; 8] = r.read_slice(8)?.try_into().unwrap();
        Ok(())
    }

    /// Library-internal Snappy. CLI never sees 'snap'.
    fn decompress_snappy(&self, data: &[u8]) -> Result<Vec<u8>, ParserError> {
        let out = snap::raw::Decoder::new()
                .decompress_vec(data)
                .map_err(|e| ParserError::Decompression(e.to_string()))?;
        
        Ok(out)
    }

    /// High-level helper for CLI: scan prologue and return plain Rust structs.
    pub fn prologue_meta(&self) -> Result<PrologueMeta, ParserError> {
        self.verify()?;
        let mut r = Reader::new(&self.bytes);
        r.seek(16)?;

        let mut meta = PrologueMeta::default();

        while let Some((cmd_raw, _tick, size)) = r.read_message_header()? {
            let compressed = Self::is_compressed(cmd_raw);
            let cmd = Self::strip_compressed_flag(cmd_raw);

            let data = r.read_slice(size as usize)?;
            let payload: Cow<[u8]> = if compressed {
                Cow::Owned(self.decompress_snappy(data)?)
            } else {
                Cow::Borrowed(data)
            };

            if let Ok(kind) = pb::EDemoCommands::try_from(cmd) {
                match kind {
                    pb::EDemoCommands::DemFileHeader if meta.header.is_none() => {
                        meta.header = Some(pb::CDemoFileHeader::decode(payload.as_ref()).unwrap());
                    }
                    pb::EDemoCommands::DemFileInfo if meta.info.is_none() => {
                        println!("HELLOOOOOOOOOOO");
                        meta.info = Some(pb::CDemoFileInfo::decode(payload.as_ref()).unwrap());
                    }
                    pb::EDemoCommands::DemStop => break,
                    _ => {}
                }
            }
        }

        Ok(meta)
    }

    /// Also expose a light-weight event scan so CLI can print without decoding.
    pub fn scan(&self) -> Result<Vec<(i32, i32, u32, bool)>, ParserError> {
        self.verify()?;
        let mut r = Reader::new(&self.bytes);
        r.seek(16)?;
        let mut out = Vec::new();
        while let Some((cmd_raw, tick, size)) = r.read_message_header()? {
            let compressed = Self::is_compressed(cmd_raw);
            let cmd = Self::strip_compressed_flag(cmd_raw);
            out.push((cmd, tick as i32, size, compressed));
            r.skip(size as usize)?;
            if let Ok(pb::EDemoCommands::DemStop) = pb::EDemoCommands::try_from(cmd) {
                break;
            }
        }
        Ok(out)
    }
}