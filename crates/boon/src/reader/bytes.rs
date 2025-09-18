//! A compact byte reader for Source-like demo files.
//!
//! Features:
//! - forward-only cursor over a byte slice
//! - varint (LEB128) u32/u64, zigzag i32
//! - little-endian fixed-width reads (u8/u32/u64)
//! - slice/bytes reads
//! - helper to read a standard (cmd, tick, size) message header

use snap::raw::Decoder;
use std::{fmt, path::Path};

use super::ReadError;

/// Forward reader over a borrowed buffer.
#[derive(Clone)]
pub struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> fmt::Debug for Reader<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Reader")
            .field("len", &self.buf.len())
            .field("pos", &self.pos)
            .finish()
    }
}

impl<'a> Reader<'a> {
    /// Create a reader over `buf`.
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    /// Convenience: read a whole file; you keep the Vec alive, then borrow it:
    ///
    /// ```no_run
    /// # use boon::reader::Reader;
    /// # fn demo() -> Result<(), Box<dyn std::error::Error>> {
    /// let data = Reader::load_file("path/to/demo.dem")?;
    /// let mut r = Reader::new(&data);
    /// # Ok(()) }
    /// ```
    pub fn load_file(path: impl AsRef<Path>) -> Result<Vec<u8>, ReadError> {
        Ok(std::fs::read(path)?)
    }

    /// Cursor position (bytes consumed).
    #[inline]
    pub fn position(&self) -> usize {
        self.pos
    }
    /// Bytes remaining.
    #[inline]
    pub fn remaining(&self) -> usize {
        self.buf.len().saturating_sub(self.pos)
    }
    /// True if nothing left to read.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.remaining() == 0
    }

    /// Seek to absolute `pos`.
    pub fn seek(&mut self, pos: usize) -> Result<(), ReadError> {
        if pos > self.buf.len() {
            return Err(ReadError::Eof);
        }
        self.pos = pos;
        Ok(())
    }

    /// Skip `n` bytes.
    pub fn skip(&mut self, n: usize) -> Result<(), ReadError> {
        self.seek(self.pos.checked_add(n).ok_or(ReadError::Eof)?)
    }

    /// Read exactly `n` bytes as a borrowed slice.
    pub fn read_slice(&mut self, n: usize) -> Result<&'a [u8], ReadError> {
        let end = self.pos.checked_add(n).ok_or(ReadError::Eof)?;
        if end > self.buf.len() {
            return Err(ReadError::Eof);
        }
        let out = &self.buf[self.pos..end];
        self.pos = end;
        Ok(out)
    }

    /// Read exactly `n` bytes, owned.
    pub fn read_bytes(&mut self, n: usize) -> Result<Vec<u8>, ReadError> {
        Ok(self.read_slice(n)?.to_vec())
    }

    /// Expand a Snappy-compressed payload into owned bytes.
    pub fn decompress_snappy(&self, data: &[u8]) -> Result<Vec<u8>, ReadError> {
        Decoder::new()
            .decompress_vec(data)
            .map_err(|err| ReadError::Decompress(err.to_string()))
    }

    pub fn read_u8(&mut self) -> Result<u8, ReadError> {
        Ok(*self.read_slice(1)?.first().unwrap())
    }
    pub fn read_u32_le(&mut self) -> Result<u32, ReadError> {
        let s = self.read_slice(4)?;
        Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }
    pub fn read_u64_le(&mut self) -> Result<u64, ReadError> {
        let s = self.read_slice(8)?;
        Ok(u64::from_le_bytes([
            s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7],
        ]))
    }

    /// Decode LEB128 varint (u32).
    pub fn read_var_u32(&mut self) -> Result<u32, ReadError> {
        let mut out: u32 = 0;
        let mut shift = 0u32;
        while shift < 35 {
            let b = self.read_u8()?;
            out |= ((b & 0x7F) as u32) << shift;
            if (b & 0x80) == 0 {
                return Ok(out);
            }
            shift += 7;
        }
        Err(ReadError::VarintTooLong)
    }

    /// Decode LEB128 varint (u64).
    pub fn read_var_u64(&mut self) -> Result<u64, ReadError> {
        let mut out: u64 = 0;
        let mut shift = 0u32;
        while shift < 70 {
            let b = self.read_u8()?;
            out |= ((b & 0x7F) as u64) << shift;
            if (b & 0x80) == 0 {
                return Ok(out);
            }
            shift += 7;
        }
        Err(ReadError::VarintTooLong)
    }

    /// Zigzag decode signed i32 from an unsigned varint.
    pub fn read_var_i32(&mut self) -> Result<i32, ReadError> {
        let u = self.read_var_u32()?;
        Ok(((u >> 1) as i32) ^ -((u & 1) as i32))
    }

    /// Read boolean from one byte (non-zero = true).
    pub fn read_bool(&mut self) -> Result<bool, ReadError> {
        Ok(self.read_u8()? != 0)
    }

    // Demo message helpers

    /// Read the standard demo header triple: `(cmd, tick, size)` as varints.
    /// Returns `Ok(None)` on clean EOF (no bytes remaining).
    pub fn read_message_header(&mut self) -> Result<Option<(u32, u32, u32)>, ReadError> {
        if self.is_empty() {
            return Ok(None);
        }
        let cmd = self.read_var_u32()?;
        let tick = self.read_var_u32()?;
        let size = self.read_var_u32()?;
        Ok(Some((cmd, tick, size)))
    }

    /// Read a message payload of `size` bytes.
    pub fn read_message_bytes(&mut self, size: u32) -> Result<Vec<u8>, ReadError> {
        self.read_bytes(size as usize)
    }
}
