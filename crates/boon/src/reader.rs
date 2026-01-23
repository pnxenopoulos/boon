use bitter::{BitReader, LittleEndianReader};

pub type Result<T> = std::result::Result<T, BitBufferError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BitBufferError {
    UnexpectedEof {
        requested_bits: usize,
        remaining_bits: usize,
    },
    Overflow(&'static str),
    InvalidArgument(&'static str),
}

impl std::fmt::Display for BitBufferError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BitBufferError::UnexpectedEof { requested_bits, remaining_bits } => {
                write!(
                    f,
                    "unexpected EOF: requested {requested_bits} bits, only {remaining_bits} bits remaining"
                )
            }
            BitBufferError::Overflow(msg) => write!(f, "overflow: {msg}"),
            BitBufferError::InvalidArgument(msg) => write!(f, "invalid argument: {msg}"),
        }
    }
}
impl std::error::Error for BitBufferError {}

#[derive(Clone)]
pub struct BitBuffer<'a> {
    data: &'a [u8],
    reader: LittleEndianReader<'a>,
    bit_pos: usize,
    total_bits: usize,
}

impl<'a> BitBuffer<'a> {
    pub const BITS_PER_BYTE: usize = 8;

    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            reader: LittleEndianReader::new(data),
            bit_pos: 0,
            total_bits: data.len() * Self::BITS_PER_BYTE,
        }
    }

    #[inline]
    pub fn get_read_count(&self) -> usize {
        self.bit_pos
    }

    #[inline]
    pub fn get_unread_count(&self) -> usize {
        self.total_bits.saturating_sub(self.bit_pos)
    }

    pub fn reset(&mut self) {
        self.reader = LittleEndianReader::new(self.data);
        self.bit_pos = 0;
    }

    /// Move forward/backward by `delta_bits` (negative = backwards).
    pub fn move_bits(&mut self, delta_bits: isize) -> Result<()> {
        if delta_bits == 0 {
            return Ok(());
        }

        if delta_bits > 0 {
            let delta = delta_bits as usize;
            self.skip_bits(delta)
        } else {
            let delta = (-delta_bits) as usize;
            if delta > self.bit_pos {
                return Err(BitBufferError::InvalidArgument(
                    "cannot move backward past start",
                ));
            }
            let new_pos = self.bit_pos - delta;
            self.seek_to(new_pos)
        }
    }

    pub fn seek_to(&mut self, new_bit_pos: usize) -> Result<()> {
        if new_bit_pos > self.total_bits {
            return Err(BitBufferError::InvalidArgument("seek beyond end"));
        }

        self.reader = LittleEndianReader::new(self.data);
        self.bit_pos = 0;
        self.skip_bits(new_bit_pos)
    }

    pub fn read_bit(&mut self) -> Result<bool> {
        self.ensure_bits(1)?;
        let v = self
            .reader
            .read_bit()
            .ok_or(BitBufferError::UnexpectedEof {
                requested_bits: 1,
                remaining_bits: 0,
            })?;
        self.bit_pos += 1;
        Ok(v)
    }

    /// Read up to 64 bits, returning an unsigned value whose LSB is the earliest bit read.
    pub fn read_bits(&mut self, bits: u32) -> Result<u64> {
        if bits > 64 {
            return Err(BitBufferError::InvalidArgument("read_bits > 64"));
        }
        let bits_usize = bits as usize;
        self.ensure_bits(bits_usize)?;
        let v = self
            .reader
            .read_bits(bits)
            .ok_or(BitBufferError::UnexpectedEof {
                requested_bits: bits_usize,
                remaining_bits: self.get_unread_count(),
            })?;
        self.bit_pos += bits_usize;
        Ok(v)
    }

    pub fn read_u8(&mut self) -> Result<u8> {
        self.ensure_bits(8)?;
        let v = self
            .reader
            .read_u8()
            .ok_or(BitBufferError::UnexpectedEof {
                requested_bits: 8,
                remaining_bits: self.get_unread_count(),
            })?;
        self.bit_pos += 8;
        Ok(v)
    }

    pub fn read_f32(&mut self) -> Result<f32> {
        self.ensure_bits(32)?;
        let v = self
            .reader
            .read_f32()
            .ok_or(BitBufferError::UnexpectedEof {
                requested_bits: 32,
                remaining_bits: self.get_unread_count(),
            })?;
        self.bit_pos += 32;
        Ok(v)
    }

    /// Returns bytes (little-endian), with unused high bits zeroed.
    pub fn read_bytes(&mut self, bits: u32) -> Result<Vec<u8>> {
        let nbytes = ((bits as usize) + 7) / 8;
        let mut out = vec![0u8; nbytes];
        self.read_into(bits, &mut out)?;
        Ok(out)
    }

    pub fn read_angle(&mut self, n: u32) -> Result<f32> {
        if n > 30 {
            return Err(BitBufferError::InvalidArgument("angle bits too large"));
        }
        let value = self.read_bits(n)? as u32;
        Ok((value as f32) * 360.0 / ((1u32 << n) as f32))
    }

    pub fn read_coordinate(&mut self) -> Result<f32> {
        let has_integer = self.read_bit()?;
        let has_fractional = self.read_bit()?;

        if !(has_integer || has_fractional) {
            return Ok(0.0);
        }

        let sign = self.read_bit()?;

        let mut integer: u32 = 0;
        if has_integer {
            integer = (self.read_bits(14)? as u32) + 1;
        }

        let mut fractional: u32 = 0;
        if has_fractional {
            fractional = self.read_bits(5)? as u32;
        }

        let mut value = (integer as f32) + (fractional as f32) * (1.0 / (1u32 << 5) as f32);
        if sign {
            value = -value;
        }
        Ok(value)
    }

    pub fn read_coordinate_precise(&mut self) -> Result<f32> {
        let value = self.read_bits(20)? as u32;
        Ok((value as f32) * (360.0 / (1u32 << 20) as f32) - 180.0)
    }

    pub fn read_normal(&mut self) -> Result<f32> {
        let sign = self.read_bit()?;
        let length = self.read_bits(11)? as u32;
        let value = (length as f32) * (1.0 / ((1u32 << 11) - 1) as f32);
        Ok(if sign { -value } else { value })
    }

    pub fn read_normal_vector(&mut self) -> Result<[f32; 3]> {
        let mut v = [0.0f32; 3];

        let has_x = self.read_bit()?;
        let has_y = self.read_bit()?;

        if has_x {
            v[0] = self.read_normal()?;
        }
        if has_y {
            v[1] = self.read_normal()?;
        }

        let negative_z = self.read_bit()?;
        let sum = v[0] * v[0] + v[1] * v[1];

        v[2] = if sum < 1.0 { (1.0 - sum).sqrt() } else { 0.0 };
        if negative_z {
            v[2] = -v[2];
        }

        Ok(v)
    }

    /// Reads a null-terminated string, with an optional max length in bytes.
    pub fn read_string(&mut self, max_len: Option<usize>) -> Result<String> {
        let mut bytes = Vec::new();
        loop {
            if let Some(limit) = max_len {
                if bytes.len() >= limit {
                    break;
                }
            }
            let b = self.read_u8()?;
            if b == 0 {
                break;
            }
            bytes.push(b);
        }
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// Source 2 custom bit-packed uvarint (6-bit prefix format).
    pub fn read_uvarint(&mut self) -> Result<u32> {
        let mut result = self.read_bits(6)? as u32;

        match result & 48 {
            16 => {
                let value = self.read_bits(4)? as u32;
                result = (result & 15) | (value << 4);
            }
            32 => {
                let value = self.read_bits(8)? as u32;
                result = (result & 15) | (value << 4);
            }
            48 => {
                let value = self.read_bits(28)? as u32;
                result = (result & 15) | (value << 4);
            }
            _ => {}
        }

        Ok(result)
    }

    /// Base-128 (protobuf-style) uvarint32.
    pub fn read_uvarint32(&mut self) -> Result<u32> {
        const MAX_BYTES: usize = 5;

        let mut result: u32 = 0;
        let mut offset: u32 = 0;

        for _ in 0..MAX_BYTES {
            let byte = self.read_u8()? as u32;
            result |= (byte & 0x7F) << (offset * 7);

            if (byte & 0x80) == 0 {
                return Ok(result);
            }
            offset += 1;
        }

        Err(BitBufferError::Overflow("uvarint32"))
    }

    pub fn read_varint32(&mut self) -> Result<i32> {
        let u = self.read_uvarint32()?;
        let decoded = ((u >> 1) as i32) ^ (-((u & 1) as i32));
        Ok(decoded)
    }

    /// Base-128 uvarint64 (max 10 bytes).
    pub fn read_uvarint64(&mut self) -> Result<u64> {
        let mut value: u64 = 0;
        for i in 0..10usize {
            let byte = self.read_u8()? as u64;

            if i > 9 || (i == 9 && byte > 1) {
                return Err(BitBufferError::Overflow("uvarint64"));
            }

            value |= (byte & 0x7F) << (7 * i);
            if (byte & 0x80) == 0 {
                return Ok(value);
            }
        }

        Err(BitBufferError::Overflow("uvarint64"))
    }

    pub fn read_varint64(&mut self) -> Result<i64> {
        let u = self.read_uvarint64()?;
        let decoded = ((u >> 1) as i64) ^ (-((u & 1) as i64));
        Ok(decoded)
    }

    pub fn read_uvarint_field_path(&mut self) -> Result<u32> {
        if self.read_bit()? {
            return Ok(self.read_bits(2)? as u32);
        }
        if self.read_bit()? {
            return Ok(self.read_bits(4)? as u32);
        }
        if self.read_bit()? {
            return Ok(self.read_bits(10)? as u32);
        }
        if self.read_bit()? {
            return Ok(self.read_bits(17)? as u32);
        }
        Ok(self.read_bits(31)? as u32)
    }

    // --- internals ---

    #[inline]
    fn ensure_bits(&self, bits: usize) -> Result<()> {
        let remaining = self.get_unread_count();
        if bits > remaining {
            return Err(BitBufferError::UnexpectedEof {
                requested_bits: bits,
                remaining_bits: remaining,
            });
        }
        Ok(())
    }

    fn skip_bits(&mut self, mut bits: usize) -> Result<()> {
        self.ensure_bits(bits)?;
        while bits >= 64 {
            // discard
            let _ = self.read_bits(64)?;
            bits -= 64;
        }
        if bits > 0 {
            let _ = self.read_bits(bits as u32)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitter_endian_matches_lsb0_expectation() {
        // Mirrors the docs quickstart behavior for LittleEndianReader:
        // data = [0xff, 0x04]
        // read_bit() => true (LSB of 0xff)
        // read_u8()  => 0x7f (next 8 bits)
        // read_bits(7) => 0x02
        let mut b = BitBuffer::new(&[0xff, 0x04]);
        assert_eq!(b.read_bit().unwrap(), true);
        assert_eq!(b.read_u8().unwrap(), 0x7f);
        assert_eq!(b.read_bits(7).unwrap(), 0x02);
    }

    #[test]
    fn move_backwards_rebuilds() {
        let mut b = BitBuffer::new(&[0xAA, 0x55]); // 1010_1010, 0101_0101
        let x = b.read_u8().unwrap();
        b.move_bits(-8).unwrap();
        let y = b.read_u8().unwrap();
        assert_eq!(x, y);
    }

    #[test]
    fn read_bytes_matches_le_packing() {
        let mut b = BitBuffer::new(&[0x34, 0x12, 0xAB]);
        let bytes = b.read_bytes(20).unwrap(); // 3 bytes, top 4 bits of last byte unused
        assert_eq!(bytes.len(), 3);
        // This is the same byte order as the underlying stream when starting at bit 0.
        assert_eq!(bytes[0], 0x34);
        assert_eq!(bytes[1], 0x12);
        // Last byte should contain only low 4 bits from 0xAB => 0x0B
        assert_eq!(bytes[2] & 0xF0, 0x00);
    }
}
