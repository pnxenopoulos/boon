use crate::error::Result;
use crate::io::BitReader;

const QFE_ROUNDDOWN: i32 = 1 << 0;
const QFE_ROUNDUP: i32 = 1 << 1;
const QFE_ENCODE_ZERO_EXACTLY: i32 = 1 << 2;
const QFE_ENCODE_INTEGERS_EXACTLY: i32 = 1 << 3;

const EQUAL_EPSILON: f32 = 0.001;

fn close_enough(a: f32, b: f32, epsilon: f32) -> bool {
    (a - b).abs() <= epsilon
}

fn compute_encode_flags(encode_flags: i32, low_value: f32, high_value: f32) -> i32 {
    let mut efs = encode_flags;

    if efs == 0 {
        return efs;
    }

    if (low_value == 0.0 && (efs & QFE_ROUNDDOWN) != 0)
        || (high_value == 0.0 && (efs & QFE_ROUNDUP) != 0)
    {
        efs &= !QFE_ENCODE_ZERO_EXACTLY;
    }

    if low_value == 0.0 && (efs & QFE_ENCODE_ZERO_EXACTLY) != 0 {
        efs |= QFE_ROUNDDOWN;
        efs &= !QFE_ENCODE_ZERO_EXACTLY;
    }
    if high_value == 0.0 && (efs & QFE_ENCODE_ZERO_EXACTLY) != 0 {
        efs |= QFE_ROUNDUP;
        efs &= !QFE_ENCODE_ZERO_EXACTLY;
    }

    if !(low_value < 0.0 && high_value > 0.0) {
        efs &= !QFE_ENCODE_ZERO_EXACTLY;
    }

    if (efs & QFE_ENCODE_INTEGERS_EXACTLY) != 0 {
        efs &= !(QFE_ROUNDUP | QFE_ROUNDDOWN | QFE_ENCODE_ZERO_EXACTLY);
    }

    efs
}

fn assign_range_multiplier(bit_count: i32, range: f64) -> f32 {
    let high_value: u32 = if bit_count == 32 {
        0xFFFFFFFE
    } else {
        (1u32 << bit_count) - 1
    };

    let mut high_low_mul = if close_enough(range as f32, 0.0, EQUAL_EPSILON) {
        high_value as f32
    } else {
        (high_value as f64 / range) as f32
    };

    if (high_low_mul as f64 * range) as u32 > high_value
        || (high_low_mul as f64 * range) > high_value as f64
    {
        const MULTIPLIERS: [f32; 5] = [0.9999, 0.99, 0.9, 0.8, 0.7];
        for &mult in &MULTIPLIERS {
            high_low_mul = (high_value as f64 / range) as f32 * mult;
            if !((high_low_mul as f64 * range) as u32 > high_value
                || (high_low_mul as f64 * range) > high_value as f64)
            {
                break;
            }
        }
    }

    high_low_mul
}

fn num_bits_for_count(n_max_elements: i32) -> i32 {
    let mut n_bits = 0;
    let mut n = n_max_elements;
    while n > 0 {
        n_bits += 1;
        n >>= 1;
    }
    n_bits
}

/// Quantized float decoder with configurable precision.
#[derive(Debug, Clone)]
pub struct QuantizedFloat {
    bit_count: i32,
    encode_flags: i32,
    low_value: f32,
    high_value: f32,
    high_low_mul: f32,
    decode_mul: f32,
}

impl QuantizedFloat {
    pub fn new(bit_count: i32, encode_flags: i32, low_value: f32, high_value: f32) -> Self {
        assert!(bit_count > 0 && bit_count < 32);

        let mut qf = Self {
            bit_count,
            encode_flags,
            low_value,
            high_value,
            high_low_mul: 0.0,
            decode_mul: 0.0,
        };

        qf.encode_flags = compute_encode_flags(qf.encode_flags, qf.low_value, qf.high_value);
        let mut steps = 1i32 << qf.bit_count;

        let range = qf.high_value - qf.low_value;
        let offset = range / steps as f32;
        if qf.encode_flags & QFE_ROUNDDOWN != 0 {
            qf.high_value -= offset;
        } else if qf.encode_flags & QFE_ROUNDUP != 0 {
            qf.low_value += offset;
        }

        if qf.encode_flags & QFE_ENCODE_INTEGERS_EXACTLY != 0 {
            let delta = (qf.low_value as i32 - qf.high_value as i32).max(1);
            let int_range = 1 << num_bits_for_count(delta);

            let mut bc = qf.bit_count;
            while (1 << bc) < int_range {
                bc += 1;
            }
            if bc > qf.bit_count {
                qf.bit_count = bc;
                steps = 1 << bc;
            }

            let offset = int_range as f32 / steps as f32;
            qf.high_value = qf.low_value + int_range as f32 - offset;
        }

        let range = qf.high_value - qf.low_value;
        qf.high_low_mul = assign_range_multiplier(qf.bit_count, range as f64);
        qf.decode_mul = 1.0 / (steps - 1) as f32;

        // Remove unnecessary flags
        if (qf.encode_flags & QFE_ROUNDDOWN) != 0 && qf.quantize(qf.low_value) == qf.low_value {
            qf.encode_flags &= !QFE_ROUNDDOWN;
        }
        if (qf.encode_flags & QFE_ROUNDUP) != 0 && qf.quantize(qf.high_value) == qf.high_value {
            qf.encode_flags &= !QFE_ROUNDUP;
        }
        if (qf.encode_flags & QFE_ENCODE_ZERO_EXACTLY) != 0 && qf.quantize(0.0) == 0.0 {
            qf.encode_flags &= !QFE_ENCODE_ZERO_EXACTLY;
        }

        qf
    }

    fn quantize(&self, value: f32) -> f32 {
        let v = if value < self.low_value {
            self.low_value
        } else if value > self.high_value {
            self.high_value
        } else {
            value
        };

        let range = self.high_value - self.low_value;
        let i = ((v - self.low_value) * self.high_low_mul) as i32;
        self.low_value + range * (i as f32 * self.decode_mul)
    }

    pub fn decode(&self, br: &mut BitReader) -> Result<f32> {
        if (self.encode_flags & QFE_ROUNDDOWN) != 0 && br.read_bool()? {
            return Ok(self.low_value);
        }

        if (self.encode_flags & QFE_ROUNDUP) != 0 && br.read_bool()? {
            return Ok(self.high_value);
        }

        if (self.encode_flags & QFE_ENCODE_ZERO_EXACTLY) != 0 && br.read_bool()? {
            return Ok(0.0);
        }

        let range = self.high_value - self.low_value;
        let value = br.read_bits(self.bit_count as usize)?;
        Ok(self.low_value + range * (value as f32 * self.decode_mul))
    }

    /// Skip past a quantized float value without decoding it.
    pub fn skip(&self, br: &mut BitReader) -> Result<()> {
        if (self.encode_flags & QFE_ROUNDDOWN) != 0 && br.read_bool()? {
            return Ok(());
        }

        if (self.encode_flags & QFE_ROUNDUP) != 0 && br.read_bool()? {
            return Ok(());
        }

        if (self.encode_flags & QFE_ENCODE_ZERO_EXACTLY) != 0 && br.read_bool()? {
            return Ok(());
        }

        br.skip_bits(self.bit_count as usize)
    }
}
