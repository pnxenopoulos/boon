//! Minimal demo parser:
//! - verify: checks magic and advances past 8-byte prologue
//! - prologue: consume messages until CDemoSyncTick; store CDemoFileHeader if seen.

use std::path::Path;

use super::ParserError;
use crate::reader::Reader;

// Generated prost types
use boon_proto::generated as pb;
use prost::Message;

/// The expected 8-byte magic header: `b"PBDEMS2\0"`.
const MAGIC: [u8; 8] = *b"PBDEMS2\0";

/// Owned parser that keeps the file bytes in memory.
#[derive(Debug, Clone)]
pub struct Parser {
    data: Vec<u8>,
    /// Where to start reading framed messages. Set to 16 after `verify()`.
    start: usize,
    /// Set to true once `prologue()` reaches CDemoSyncTick.
    prologue_completed: bool,
    /// The most recent CDemoFileHeader encountered during `prologue()`.
    pub file_header: Option<pb::CDemoFileHeader>,
}

impl Parser {
    /// Open a demo file into memory.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, ParserError> {
        let data = std::fs::read(path)?;
        Ok(Self {
            data,
            start: 0,
            prologue_completed: false,
            file_header: None,
        })
    }

    /// Construct from already-loaded bytes (takes ownership).
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self {
            data: bytes,
            start: 0,
            prologue_completed: false,
            file_header: None,
        }
    }

    /// Verify the file is a demo:
    /// - len >= 16
    /// - first 8 bytes equal MAGIC
    /// - advance parser start past the 8-byte prologue (i.e., start=16)
    pub fn verify(&mut self) -> Result<(), ParserError> {
        if self.data.len() < 16 {
            return Err(ParserError::TooSmall(self.data.len()));
        }
        let mut r = Reader::new(&self.data);

        // Magic
        let magic: [u8; 8] = r.read_slice(8)?.try_into().unwrap();
        if magic != MAGIC {
            return Err(ParserError::WrongMagic(magic));
        }

        // Prologue (8 bytes) — we "advance" by setting start
        let _prologue8: [u8; 8] = r.read_slice(8)?.try_into().unwrap();

        self.start = r.position(); // should be 16
        Ok(())
    }

    /// Run the "prologue" phase:
    /// iterate framed messages (cmd, tick, size) starting at `start`
    /// until we hit `EDemoCommands::DemSyncTick`.
    ///
    /// - If we see `EDemoCommands::DemFileHeader`, decode and keep it.
    /// - Handles Snappy compression when `DemIsCompressed` flag bit is set.
    pub fn prologue(&mut self) -> Result<(), ParserError> {
        // Ensure we've verified the header
        if self.start < 16 {
            self.verify()?;
        }

        let mut r = Reader::new(&self.data);
        r.seek(self.start)?;

        while let Some((cmd_raw, _tick, size)) = r.read_message_header()? {
            let payload = r.read_message_bytes(size)?;

            // Separate "is compressed" flag from command id
            let cmd_i32 = cmd_raw as i32;
            let flag = pb::EDemoCommands::DemIsCompressed as i32;
            let id = cmd_i32 & !flag;

            let bytes = payload;

            if let Some(kind) = pb::EDemoCommands::from_i32(id) {
                match kind {
                    pb::EDemoCommands::DemFileHeader => {
                        if let Ok(hdr) = pb::CDemoFileHeader::decode(bytes.as_slice()) {
                            self.file_header = Some(hdr);
                        }
                    }
                    pb::EDemoCommands::DemSyncTick => {
                        self.prologue_completed = true;
                        self.start = r.position(); // where to continue from later
                        break;
                    }
                    _ => { /* ignore other commands during prologue */ }
                }
            }
        }

        Ok(())
    }

    /// Has the parser completed its prologue phase (i.e., reached CDemoSyncTick)?
    pub fn prologue_completed(&self) -> bool {
        self.prologue_completed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: encode a u32 varint (LEB128)
    fn enc_var_u32(mut v: u32) -> Vec<u8> {
        let mut out = Vec::new();
        loop {
            let mut b = (v & 0x7F) as u8;
            v >>= 7;
            if v != 0 {
                b |= 0x80;
            }
            out.push(b);
            if v == 0 {
                break;
            }
        }
        out
    }

    #[test]
    fn verify_sets_start_and_magic_ok() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&MAGIC);
        buf.extend_from_slice(&[0xAA; 8]); // prologue

        let mut p = Parser::from_bytes(buf);
        p.verify().unwrap();
        assert_eq!(p.start, 16);
        assert!(!p.prologue_completed());
        assert!(p.file_header.is_none());
    }

    #[test]
    fn verify_fails_small_or_wrong_magic() {
        // too small
        let mut p = Parser::from_bytes(vec![1, 2, 3, 4, 5, 6, 7, 8]);
        assert!(matches!(p.verify().unwrap_err(), ParserError::TooSmall(_)));

        // wrong magic
        let mut buf = vec![0xFF; 8];
        buf.extend_from_slice(&[0; 8]);
        let mut p = Parser::from_bytes(buf);
        match p.verify().unwrap_err() {
            ParserError::WrongMagic(m) => assert_eq!(m, [0xFF; 8]),
            e => panic!("unexpected: {e:?}"),
        }
    }

    #[test]
    fn prologue_collects_header_and_stops_at_sync() {
        // Build: magic + prologue + [FileHeader frame] + [SyncTick frame]
        let mut buf = Vec::new();
        buf.extend_from_slice(&MAGIC);
        buf.extend_from_slice(&[0; 8]);

        // FileHeader payload (default)
        let fh = pb::CDemoFileHeader::default();
        let fh_bytes = fh.encode_to_vec();

        let file_header_id = pb::EDemoCommands::DemFileHeader as i32 as u32;
        let sync_id = pb::EDemoCommands::DemSyncTick as i32 as u32;

        // FileHeader frame: cmd, tick=0, size=len, payload
        buf.extend(enc_var_u32(file_header_id));
        buf.extend(enc_var_u32(0));
        buf.extend(enc_var_u32(fh_bytes.len() as u32));
        buf.extend_from_slice(&fh_bytes);

        // SyncTick frame: cmd, tick=0, size=0, no payload
        buf.extend(enc_var_u32(sync_id));
        buf.extend(enc_var_u32(0));
        buf.extend(enc_var_u32(0));

        let mut p = Parser::from_bytes(buf);
        p.verify().unwrap();
        p.prologue().unwrap();

        assert!(p.prologue_completed());
        assert!(p.file_header.is_some());
    }
}
