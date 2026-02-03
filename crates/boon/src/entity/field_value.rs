use std::fmt;

use crate::error::FieldValueConversionError;

/// Represents a decoded entity field value.
#[derive(Clone)]
pub enum FieldValue {
    Bool(bool),
    I32(i32),
    I64(i64),
    U32(u32),
    U64(u64),
    F32(f32),
    String(Vec<u8>),
    Vector2([f32; 2]),
    Vector3([f32; 3]),
    Vector4([f32; 4]),
    QAngle([f32; 3]),
}

impl fmt::Debug for FieldValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bool(v) => write!(f, "{v}"),
            Self::I32(v) => write!(f, "{v}"),
            Self::I64(v) => write!(f, "{v}"),
            Self::U32(v) => write!(f, "{v}"),
            Self::U64(v) => write!(f, "{v}"),
            Self::F32(v) => write!(f, "{v}"),
            Self::String(v) => write!(f, "{}", String::from_utf8_lossy(v)),
            Self::Vector2(v) => write!(f, "[{}, {}]", v[0], v[1]),
            Self::Vector3(v) => write!(f, "[{}, {}, {}]", v[0], v[1], v[2]),
            Self::Vector4(v) => write!(f, "[{}, {}, {}, {}]", v[0], v[1], v[2], v[3]),
            Self::QAngle(v) => write!(f, "QAngle({}, {}, {})", v[0], v[1], v[2]),
        }
    }
}

impl fmt::Display for FieldValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

macro_rules! impl_try_from_signed {
    ($($ty:ty),+) => {
        $(
            impl TryFrom<FieldValue> for $ty {
                type Error = FieldValueConversionError;
                fn try_from(v: FieldValue) -> std::result::Result<Self, Self::Error> {
                    match v {
                        FieldValue::I32(val) => <$ty>::try_from(val).map_err(|_| FieldValueConversionError),
                        FieldValue::I64(val) => <$ty>::try_from(val).map_err(|_| FieldValueConversionError),
                        _ => Err(FieldValueConversionError),
                    }
                }
            }
        )+
    }
}

macro_rules! impl_try_from_unsigned {
    ($($ty:ty),+) => {
        $(
            impl TryFrom<FieldValue> for $ty {
                type Error = FieldValueConversionError;
                fn try_from(v: FieldValue) -> std::result::Result<Self, Self::Error> {
                    match v {
                        FieldValue::U32(val) => <$ty>::try_from(val).map_err(|_| FieldValueConversionError),
                        FieldValue::U64(val) => <$ty>::try_from(val).map_err(|_| FieldValueConversionError),
                        _ => Err(FieldValueConversionError),
                    }
                }
            }
        )+
    }
}

impl_try_from_signed!(i8, i16, i32, i64);
impl_try_from_unsigned!(u8, u16, u32, u64);

impl TryFrom<FieldValue> for f32 {
    type Error = FieldValueConversionError;
    fn try_from(v: FieldValue) -> std::result::Result<Self, Self::Error> {
        match v {
            FieldValue::F32(val) => Ok(val),
            _ => Err(FieldValueConversionError),
        }
    }
}

impl TryFrom<FieldValue> for bool {
    type Error = FieldValueConversionError;
    fn try_from(v: FieldValue) -> std::result::Result<Self, Self::Error> {
        match v {
            FieldValue::Bool(val) => Ok(val),
            _ => Err(FieldValueConversionError),
        }
    }
}

impl TryFrom<FieldValue> for [f32; 2] {
    type Error = FieldValueConversionError;
    fn try_from(v: FieldValue) -> std::result::Result<Self, Self::Error> {
        match v {
            FieldValue::Vector2(val) => Ok(val),
            _ => Err(FieldValueConversionError),
        }
    }
}

impl TryFrom<FieldValue> for [f32; 3] {
    type Error = FieldValueConversionError;
    fn try_from(v: FieldValue) -> std::result::Result<Self, Self::Error> {
        match v {
            FieldValue::Vector3(val) | FieldValue::QAngle(val) => Ok(val),
            _ => Err(FieldValueConversionError),
        }
    }
}

impl TryFrom<FieldValue> for [f32; 4] {
    type Error = FieldValueConversionError;
    fn try_from(v: FieldValue) -> std::result::Result<Self, Self::Error> {
        match v {
            FieldValue::Vector4(val) => Ok(val),
            _ => Err(FieldValueConversionError),
        }
    }
}
