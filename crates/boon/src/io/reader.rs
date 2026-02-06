use crate::error::{Error, Result};

/// Byte-level cursor reader for the outer demo command stream.
pub struct ByteReader<'a> {
    data: &'a [u8],
    position: usize,
}

impl<'a> ByteReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, position: 0 }
    }

    #[inline]
    pub fn position(&self) -> usize {
        self.position
    }

    #[inline]
    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.position)
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.position >= self.data.len()
    }

    pub fn seek(&mut self, position: usize) -> Result<()> {
        if position > self.data.len() {
            return Err(Error::Overflow {
                needed: position,
                available: self.data.len(),
            });
        }
        self.position = position;
        Ok(())
    }

    fn check_remaining(&self, n: usize) -> Result<()> {
        if self.position + n > self.data.len() {
            return Err(Error::Overflow {
                needed: n,
                available: self.remaining(),
            });
        }
        Ok(())
    }

    pub fn read_u8(&mut self) -> Result<u8> {
        self.check_remaining(1)?;
        let val = self.data[self.position];
        self.position += 1;
        Ok(val)
    }

    pub fn read_u16(&mut self) -> Result<u16> {
        self.check_remaining(2)?;
        let val = u16::from_le_bytes([self.data[self.position], self.data[self.position + 1]]);
        self.position += 2;
        Ok(val)
    }

    pub fn read_u32(&mut self) -> Result<u32> {
        self.check_remaining(4)?;
        let val = u32::from_le_bytes([
            self.data[self.position],
            self.data[self.position + 1],
            self.data[self.position + 2],
            self.data[self.position + 3],
        ]);
        self.position += 4;
        Ok(val)
    }

    pub fn read_i32(&mut self) -> Result<i32> {
        self.check_remaining(4)?;
        let val = i32::from_le_bytes([
            self.data[self.position],
            self.data[self.position + 1],
            self.data[self.position + 2],
            self.data[self.position + 3],
        ]);
        self.position += 4;
        Ok(val)
    }

    pub fn read_bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        self.check_remaining(n)?;
        let slice = &self.data[self.position..self.position + n];
        self.position += n;
        Ok(slice)
    }

    /// Read an unsigned varint32 from the byte stream.
    pub fn read_uvarint32(&mut self) -> Result<u32> {
        let mut result: u32 = 0;
        for i in 0..5 {
            let byte = self.read_u8()? as u32;
            result |= (byte & 0x7F) << (7 * i);
            if byte & 0x80 == 0 {
                return Ok(result);
            }
        }
        Ok(result)
    }

    /// Read an unsigned varint64 from the byte stream.
    pub fn read_uvarint64(&mut self) -> Result<u64> {
        let mut result: u64 = 0;
        for i in 0..10 {
            let byte = self.read_u8()? as u64;
            result |= (byte & 0x7F) << (7 * i);
            if byte & 0x80 == 0 {
                return Ok(result);
            }
        }
        Ok(result)
    }

    /// Skip forward by N bytes.
    pub fn skip(&mut self, n: usize) -> Result<()> {
        self.check_remaining(n)?;
        self.position += n;
        Ok(())
    }

    /// Get the underlying data slice.
    pub fn data(&self) -> &'a [u8] {
        self.data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_u32() {
        let data = 0x12345678u32.to_le_bytes();
        let mut r = ByteReader::new(&data);
        assert_eq!(r.read_u32().unwrap(), 0x12345678);
    }

    #[test]
    fn test_read_varint() {
        let data = [0xAC, 0x02];
        let mut r = ByteReader::new(&data);
        assert_eq!(r.read_uvarint32().unwrap(), 300);
    }

    #[test]
    fn test_read_bytes() {
        let data = [1, 2, 3, 4, 5];
        let mut r = ByteReader::new(&data);
        let bytes = r.read_bytes(3).unwrap();
        assert_eq!(bytes, &[1, 2, 3]);
        assert_eq!(r.remaining(), 2);
    }

    #[test]
    fn test_overflow() {
        let data = [1];
        let mut r = ByteReader::new(&data);
        r.read_u8().unwrap();
        assert!(r.read_u8().is_err());
    }
}
