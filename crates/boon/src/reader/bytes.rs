//! Minimal bit/byte reader for Source-like demo streams.
//! Wraps `bitter::LittleEndianReader` but provides clearer, checked APIs.

use bitter::BitReader;
use bitter::LittleEndianReader;

use super::ReadError;

/// Extra-bit counts for Source UBitVar by prefix class (00,01,10,11).
const UBITVAR_EXTRA: [u8; 4] = [0, 4, 8, 28];

/// Forward reader over a borrowed buffer.
#[derive(Debug, Clone)]
pub struct Reader<'a> {
    buf: &'a [u8],
    r: LittleEndianReader<'a>,
    /// Bit position from start of stream.
    bit_pos: usize,
}

impl<'a> Reader<'a> {
    /// Create a reader over `buffer`.
    pub fn new(buffer: &'a [u8]) -> Self {
        Self {
            buf: buffer,
            r: LittleEndianReader::new(buffer),
            bit_pos: 0,
        }
    }

    /// Reset to the start of the stream.
    #[inline]
    pub fn reset(&mut self) {
        self.r = LittleEndianReader::new(self.buf);
        self.bit_pos = 0;
    }

    /// Total bytes still unread from the underlying buffer.
    #[inline]
    pub fn bytes_remaining(&self) -> usize {
        self.r.bytes_remaining()
    }

    #[inline]
    pub fn is_eof(&self) -> bool {
        self.r.lookahead_bits() == 0 && self.r.bytes_remaining() == 0
    }

    /// Total bits remaining including lookahead.
    #[inline]
    pub fn bits_remaining_total(&self) -> usize {
        self.r.lookahead_bits() as usize + 8 * self.r.bytes_remaining()
    }

    /// Whether the current bit position is at a byte boundary.
    #[inline]
    pub fn is_byte_aligned(&self) -> bool {
        (self.bit_pos & 7) == 0
    }

    /// Ensure at least `need` bits in lookahead or report EOF.
    #[inline]
    fn need_bits(&mut self, need: u32) -> Result<(), ReadError> {
        if self.r.lookahead_bits() < need {
            if self.r.bytes_remaining() == 0 {
                return Err(ReadError::Eof);
            }
            self.r.refill_lookahead();
            if self.r.lookahead_bits() < need {
                // Defensive: refill but still not enough.
                return Err(ReadError::Eof);
            }
        }
        Ok(())
    }

    /// Align to next byte boundary by consuming padding bits.
    #[inline]
    pub fn align_to_byte(&mut self) -> Result<(), ReadError> {
        let mis = (self.bit_pos & 7) as u32;
        if mis != 0 {
            let pad = 8 - mis;
            self.need_bits(pad)?;
            self.r.consume(pad);
            self.bit_pos += pad as usize;
        }
        Ok(())
    }

    /// Read `n` bits (n ∈ [0,32]) as a u32.
    #[inline]
    pub fn read_bits(&mut self, n: u32) -> Result<u32, ReadError> {
        if n == 0 {
            return Ok(0);
        }
        self.need_bits(n)?;
        let v = self.r.peek(n) as u32;
        self.r.consume(n);
        self.bit_pos += n as usize;
        Ok(v)
    }

    /// Read a single bit as bool.
    #[inline]
    pub fn read_bool(&mut self) -> Result<bool, ReadError> {
        Ok(self.read_bits(1)? == 1)
    }

    /// Read exactly `n` bytes. Stream must be byte-aligned.
    #[inline]
    pub fn read_bytes(&mut self, n: usize) -> Result<Vec<u8>, ReadError> {
        self.align_to_byte()?;
        if self.bytes_remaining() < n {
            // If not enough whole bytes, treat as EOF.
            return Err(ReadError::Eof);
        }
        // `LittleEndianReader::read_bytes` consumes from lookahead+buffer.
        let mut out = vec![0u8; n];
        self.r.read_bytes(&mut out);
        self.bit_pos += n * 8;
        Ok(out)
    }

    /// Skip exactly `n` bytes.
    #[inline]
    pub fn skip_bytes(&mut self, n: usize) -> Result<(), ReadError> {
        self.align_to_byte()?;
        // Consume in bit chunks through lookahead to avoid re-implementing internals.
        let mut bits_left = (n as u32) * 8;
        while bits_left > 0 {
            let have = self.r.lookahead_bits();
            if have == 0 {
                if self.r.bytes_remaining() == 0 {
                    return Err(ReadError::Eof);
                }
                self.r.refill_lookahead();
                let again = self.r.lookahead_bits();
                if again == 0 {
                    return Err(ReadError::Eof);
                }
            }
            let step = self.r.lookahead_bits().min(bits_left);
            self.r.consume(step);
            self.bit_pos += step as usize;
            bits_left -= step;
        }
        Ok(())
    }

    /// Read a 32-bit IEEE754 float.
    #[inline]
    pub fn read_f32(&mut self) -> Result<f32, ReadError> {
        Ok(f32::from_bits(self.read_bits(32)?))
    }

    /// Read an unsigned LEB128 up to 32 bits. Stops at MSB=0 or when 5 bytes consumed.
    #[inline]
    pub fn read_var_u32(&mut self) -> Result<u32, ReadError> {
        let mut x: u32 = 0;
        let mut shift: u32 = 0;
        // At most 5 bytes for 32-bit.
        for _ in 0..5 {
            let byte = self.read_bits(8)?;
            x |= (byte & 0x7F) << shift;
            if (byte & 0x80) == 0 {
                return Ok(x);
            }
            shift += 7;
        }
        // If we hit here, we consumed 5 bytes and the last had MSB=1; treat as overflow.
        Err(ReadError::Overflow)
    }

    /// Read signed varint via zig-zag decoding of the above.
    #[inline]
    pub fn read_var_i32(&mut self) -> Result<i32, ReadError> {
        let ux = self.read_var_u32()?;
        // Zig-zag: LSB is sign.
        let val = ((ux >> 1) as i32) ^ -((ux & 1) as i32);
        Ok(val)
    }

    /// Read Source UBitVar as used in S2 demos.
    ///
    /// Layout: read 6 bits. Top 2 bits are class C∈{0,1,2,3}. Low 4 bits are the low nibble V.
    /// Then read EXTRA[C] bits as the high part and return: V | (high << 4).
    #[inline]
    pub fn read_ubit_var(&mut self) -> Result<u32, ReadError> {
        let prefix = self.read_bits(6)?; // [C1 C0 v3 v2 v1 v0]
        let class = (prefix >> 4) as usize; // C in 0..3
        let low = prefix & 0x0F; // v in 0..15
        let extra = UBITVAR_EXTRA[class] as u32; // 0,4,8,28
        if extra == 0 {
            return Ok(low);
        }
        let high = self.read_bits(extra)?; // high part
        Ok(low | (high << 4))
    }

    /// Demo-specific: read (cmd, tick, size) if available, or None at clean EOF.
    #[inline]
    pub fn read_message_header(&mut self) -> Result<Option<(u32, u32, u32)>, ReadError> {
        if self.is_eof() {
            return Ok(None);
        }
        let cmd = self.read_var_u32()?;
        let tick = self.read_var_u32()?;
        let size = self.read_var_u32()?;
        Ok(Some((cmd, tick, size)))
    }
}
