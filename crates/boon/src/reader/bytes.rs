//! A compact byte reader for Source-like demo files.
use bitter::{BitReader, LittleEndianReader};

use super::ReadError;

/// Counts
const UBV_COUNT: [u8; 4] = [0, 4, 8, 28];

/// Forward reader over a borrowed buffer.
#[derive(Debug, Clone)]
pub struct Reader<'a> {
    buffer: &'a [u8],
    little_endian_reader: LittleEndianReader<'a>,
    position: usize,
}

impl<'a> Reader<'a> {
    /// Create a reader over `buffer`.
    pub fn new(buffer: &'a [u8]) -> Self {
        Self {
            buffer,
            little_endian_reader: LittleEndianReader::new(buffer),
            position: 0,
        }
    }

    #[inline]
    pub fn reset(&mut self) {
        self.little_endian_reader = LittleEndianReader::new(self.buffer);
        self.position = 0;
    }

    #[inline]
    pub fn bytes_remaining(&self) -> usize {
        self.little_endian_reader.bytes_remaining()
    }

    #[inline]
    pub fn has_no_more_bytes(&self) -> bool {
        self.bytes_remaining() == 0
    }

    #[inline]
    pub fn refill(&mut self) {
        self.little_endian_reader.refill_lookahead();
    }

    #[inline]
    pub fn align_to_byte(&mut self) {
        let mis = self.position & 7; // bits past the last boundary
        if mis != 0 {
            let need = (8 - mis) as u32; // bits to consume to reach the boundary
            self.little_endian_reader.consume(need);
            self.position += need as usize;
        }
    }

    #[inline]
    pub fn read_bits(&mut self, n: u32) -> u32 {
        self.refill();
        self.read_bits_no_refill(n)
    }

    #[inline]
    pub fn read_bits_no_refill(&mut self, n: u32) -> u32 {
        let bits = self.little_endian_reader.peek(n);
        self.little_endian_reader.consume(n);
        self.position += n as usize; // keep position in sync
        bits as u32
    }

    #[inline]
    pub fn read_bytes(&mut self, n: u32) -> Vec<u8> {
        let mut bytes = vec![0; n as usize];
        self.little_endian_reader.read_bytes(&mut bytes);
        self.position += (n as usize) * 8; // 8 bits per byte
        bytes
    }

    #[inline]
    pub fn skip_bytes(&mut self, n: usize) -> Result<(), ReadError> {
        let mut bits_left: u32 = (n as u32) * 8;

        while bits_left > 0 {
            let mut la = self.little_endian_reader.lookahead_bits();
            if la == 0 {
                // Pull more bits into lookahead; if none remain, it's EOF.
                if self.little_endian_reader.bytes_remaining() == 0 {
                    return Err(ReadError::Eof);
                }
                self.little_endian_reader.refill_lookahead();
                la = self.little_endian_reader.lookahead_bits();
                if la == 0 {
                    // Defensive: if refill didn't add bits, treat as EOF.
                    return Err(ReadError::Eof);
                }
            }

            let step = la.min(bits_left);
            self.little_endian_reader.consume(step);
            self.position += step as usize;
            bits_left -= step;
        }

        Ok(())
    }

    #[inline]
    pub fn is_byte_aligned(&self) -> bool {
        (self.position & 7) == 0
    }

    #[inline]
    pub fn read_ubit_var(&mut self) -> u32 {
        self.refill();

        let prefix = self.read_bits_no_refill(6);
        let prefix_class = prefix >> 4;
        if prefix == 0 {
            return prefix_class;
        }
        (prefix & 15) | (self.read_bits_no_refill(UBV_COUNT[prefix_class as usize] as u32) << 4)
    }

    #[inline]
    pub fn read_var_u32(&mut self) -> u32 {
        let mut x: u32 = 0;
        let mut y: u32 = 0;
        self.refill();
        loop {
            let byte = self.read_bits_no_refill(8);

            x |= (byte & 0x7F) << y;
            y += 7;

            if (byte & 0x80) == 0 || y == 35 {
                return x;
            }
        }
    }

    /// Demo-specific methods
    #[inline]
    pub fn read_message_header(&mut self) -> Result<Option<(u32, u32, u32)>, ReadError> {
        // If no more bytes, return None
        if self.has_no_more_bytes() {
            return Ok(None);
        }

        // Read the command, tick, and its size
        let cmd = self.read_var_u32();
        let tick = self.read_var_u32();
        let size = self.read_var_u32();
        Ok(Some((cmd, tick, size)))
    }
}
