use crate::error::Result;
use crate::io::BitReader;

use super::field_value::FieldValue;
use super::quantized_float::QuantizedFloat;

/// Context passed to field decoders for values that need external state.
pub struct FieldDecodeContext {
    pub tick_interval: f32,
    pub string_buf: Vec<u8>,
}

impl FieldDecodeContext {
    pub fn new(tick_interval: f32) -> Self {
        Self {
            tick_interval,
            string_buf: Vec::with_capacity(512),
        }
    }
}

/// Describes a field decoder.
#[derive(Debug, Clone)]
pub enum Decoder {
    Bool,
    I64,
    U64,
    U64Fixed64,
    F32NoScale,
    F32SimulationTime,
    F32Coord,
    F32Normal,
    F32Quantized(QuantizedFloat),
    String,
    Vector2(Box<Decoder>),
    Vector3(Box<Decoder>),
    Vector3Normal,
    Vector4(Box<Decoder>),
    QAnglePitchYaw {
        bit_count: usize,
    },
    QAnglePrecise,
    QAngleBitCount {
        bit_count: usize,
    },
    QAngleCoord,
    /// Used as a placeholder/invalid decoder.
    Default,
}

impl Decoder {
    pub fn decode(&self, ctx: &mut FieldDecodeContext, br: &mut BitReader) -> Result<FieldValue> {
        match self {
            Decoder::Bool => Ok(FieldValue::Bool(br.read_bool()?)),

            Decoder::I64 => Ok(FieldValue::I64(br.read_varint64()?)),

            Decoder::U64 => Ok(FieldValue::U64(br.read_uvarint64()?)),

            Decoder::U64Fixed64 => {
                let mut buf = [0u8; 8];
                br.read_bytes(&mut buf)?;
                Ok(FieldValue::U64(u64::from_le_bytes(buf)))
            }

            Decoder::F32NoScale => Ok(FieldValue::F32(br.read_f32()?)),

            Decoder::F32SimulationTime => {
                let ticks = br.read_uvarint32()?;
                Ok(FieldValue::F32(ticks as f32 * ctx.tick_interval))
            }

            Decoder::F32Coord => Ok(FieldValue::F32(br.read_bitcoord()?)),

            Decoder::F32Normal => Ok(FieldValue::F32(br.read_bitnormal()?)),

            Decoder::F32Quantized(qf) => Ok(FieldValue::F32(qf.decode(br)?)),

            Decoder::String => {
                ctx.string_buf.clear();
                br.read_string_raw(&mut ctx.string_buf)?;
                Ok(FieldValue::String(ctx.string_buf.clone()))
            }

            Decoder::Vector2(inner) => {
                let x = inner.decode_f32(ctx, br)?;
                let y = inner.decode_f32(ctx, br)?;
                Ok(FieldValue::Vector2([x, y]))
            }

            Decoder::Vector3(inner) => {
                let x = inner.decode_f32(ctx, br)?;
                let y = inner.decode_f32(ctx, br)?;
                let z = inner.decode_f32(ctx, br)?;
                Ok(FieldValue::Vector3([x, y, z]))
            }

            Decoder::Vector3Normal => Ok(FieldValue::Vector3(br.read_bitvec3normal()?)),

            Decoder::Vector4(inner) => {
                let x = inner.decode_f32(ctx, br)?;
                let y = inner.decode_f32(ctx, br)?;
                let z = inner.decode_f32(ctx, br)?;
                let w = inner.decode_f32(ctx, br)?;
                Ok(FieldValue::Vector4([x, y, z, w]))
            }

            Decoder::QAnglePitchYaw { bit_count } => {
                let pitch = br.read_bitangle(*bit_count)?;
                let yaw = br.read_bitangle(*bit_count)?;
                Ok(FieldValue::QAngle([pitch, yaw, 0.0]))
            }

            Decoder::QAnglePrecise => {
                let mut v = [0.0f32; 3];
                let rx = br.read_bool()?;
                let ry = br.read_bool()?;
                let rz = br.read_bool()?;
                if rx {
                    v[0] = br.read_bitangle(20)?;
                }
                if ry {
                    v[1] = br.read_bitangle(20)?;
                }
                if rz {
                    v[2] = br.read_bitangle(20)?;
                }
                Ok(FieldValue::QAngle(v))
            }

            Decoder::QAngleBitCount { bit_count } => {
                let x = br.read_bitangle(*bit_count)?;
                let y = br.read_bitangle(*bit_count)?;
                let z = br.read_bitangle(*bit_count)?;
                Ok(FieldValue::QAngle([x, y, z]))
            }

            Decoder::QAngleCoord => Ok(FieldValue::QAngle(br.read_bitvec3coord()?)),

            Decoder::Default => Ok(FieldValue::U64(br.read_uvarint64()?)),
        }
    }

    /// Helper to decode a field value as f32 (used by vector decoders).
    fn decode_f32(&self, ctx: &mut FieldDecodeContext, br: &mut BitReader) -> Result<f32> {
        match self.decode(ctx, br)? {
            FieldValue::F32(v) => Ok(v),
            _ => Ok(0.0),
        }
    }

    /// Skip a field value without fully decoding it - just advances the bit reader.
    /// This is faster than decode() when we don't need the value.
    #[allow(clippy::only_used_in_recursion)]
    pub fn skip(&self, ctx: &mut FieldDecodeContext, br: &mut BitReader) -> Result<()> {
        match self {
            Decoder::Bool => {
                br.skip_bits(1)?;
            }

            Decoder::I64 => {
                br.skip_varint()?;
            }

            Decoder::U64 => {
                br.skip_varint()?;
            }

            Decoder::U64Fixed64 => {
                br.skip_bits(64)?;
            }

            Decoder::F32NoScale => {
                br.skip_bits(32)?;
            }

            Decoder::F32SimulationTime => {
                br.skip_varint()?;
            }

            Decoder::F32Coord => {
                br.skip_bitcoord()?;
            }

            Decoder::F32Normal => {
                br.skip_bitnormal()?;
            }

            Decoder::F32Quantized(qf) => {
                qf.skip(br)?;
            }

            Decoder::String => {
                br.skip_string()?;
            }

            Decoder::Vector2(inner) => {
                inner.skip(ctx, br)?;
                inner.skip(ctx, br)?;
            }

            Decoder::Vector3(inner) => {
                inner.skip(ctx, br)?;
                inner.skip(ctx, br)?;
                inner.skip(ctx, br)?;
            }

            Decoder::Vector3Normal => {
                br.skip_bitvec3normal()?;
            }

            Decoder::Vector4(inner) => {
                inner.skip(ctx, br)?;
                inner.skip(ctx, br)?;
                inner.skip(ctx, br)?;
                inner.skip(ctx, br)?;
            }

            Decoder::QAnglePitchYaw { bit_count } => {
                br.skip_bits(*bit_count * 2)?;
            }

            Decoder::QAnglePrecise => {
                let rx = br.read_bool()?;
                let ry = br.read_bool()?;
                let rz = br.read_bool()?;
                if rx {
                    br.skip_bits(20)?;
                }
                if ry {
                    br.skip_bits(20)?;
                }
                if rz {
                    br.skip_bits(20)?;
                }
            }

            Decoder::QAngleBitCount { bit_count } => {
                br.skip_bits(*bit_count * 3)?;
            }

            Decoder::QAngleCoord => {
                br.skip_bitvec3coord()?;
            }

            Decoder::Default => {
                br.skip_varint()?;
            }
        }
        Ok(())
    }
}

/// Special descriptor for fields that need non-standard handling.
#[derive(Debug, Clone)]
pub enum FieldSpecialDescriptor {
    FixedArray { length: usize },
    DynamicArray { inner_decoder: Decoder },
    DynamicSerializerArray,
    Pointer,
}

/// Metadata about how to decode a field.
#[derive(Debug, Clone)]
pub struct FieldMetadata {
    pub decoder: Decoder,
    pub special: Option<FieldSpecialDescriptor>,
}

impl Default for FieldMetadata {
    fn default() -> Self {
        Self {
            decoder: Decoder::Default,
            special: None,
        }
    }
}

impl FieldMetadata {
    pub fn is_dynamic_array(&self) -> bool {
        matches!(
            self.special,
            Some(FieldSpecialDescriptor::DynamicArray { .. })
                | Some(FieldSpecialDescriptor::DynamicSerializerArray)
        )
    }

    pub fn is_fixed_array(&self) -> bool {
        matches!(
            self.special,
            Some(FieldSpecialDescriptor::FixedArray { .. })
        )
    }

    pub fn fixed_array_length(&self) -> Option<usize> {
        match &self.special {
            Some(FieldSpecialDescriptor::FixedArray { length }) => Some(*length),
            _ => None,
        }
    }

    pub fn is_dynamic_serializer_array(&self) -> bool {
        matches!(
            self.special,
            Some(FieldSpecialDescriptor::DynamicSerializerArray)
        )
    }

    pub fn is_pointer(&self) -> bool {
        matches!(self.special, Some(FieldSpecialDescriptor::Pointer))
    }

    pub fn dynamic_array_inner_metadata(&self) -> FieldMetadata {
        match &self.special {
            Some(FieldSpecialDescriptor::DynamicArray { inner_decoder }) => FieldMetadata {
                decoder: inner_decoder.clone(),
                special: None,
            },
            _ => FieldMetadata::default(),
        }
    }
}

/// Build a float decoder based on field properties.
fn build_f32_decoder(
    var_name: &str,
    bit_count: Option<i32>,
    low_value: Option<f32>,
    high_value: Option<f32>,
    encode_flags: Option<i32>,
    var_encoder: Option<&str>,
) -> Decoder {
    // Simulation time special case
    if var_name == "m_flSimulationTime" || var_name == "m_flAnimTime" {
        return Decoder::F32SimulationTime;
    }

    // Check var_encoder
    if let Some(encoder) = var_encoder {
        match encoder {
            "coord" => return Decoder::F32Coord,
            "normal" => return Decoder::F32Normal,
            _ => {}
        }
    }

    let bc = bit_count.unwrap_or(0);
    if bc == 0 || bc == 32 {
        return Decoder::F32NoScale;
    }

    // Quantized float
    Decoder::F32Quantized(QuantizedFloat::new(
        bc,
        encode_flags.unwrap_or(0),
        low_value.unwrap_or(0.0),
        high_value.unwrap_or(0.0),
    ))
}

/// Determine the field metadata (decoder + special descriptor) for a field.
#[allow(clippy::too_many_arguments)]
pub fn get_field_metadata(
    var_type: &str,
    var_name: &str,
    bit_count: Option<i32>,
    low_value: Option<f32>,
    high_value: Option<f32>,
    encode_flags: Option<i32>,
    var_encoder: Option<&str>,
    has_field_serializer: bool,
) -> FieldMetadata {
    // Parse the type to determine category
    let trimmed = var_type.trim();

    // Pointer types
    if trimmed.ends_with('*') {
        return FieldMetadata {
            decoder: Decoder::Bool,
            special: Some(FieldSpecialDescriptor::Pointer),
        };
    }

    // Array types: type[length]
    if let Some(bracket_pos) = trimmed.find('[')
        && trimmed.ends_with(']')
    {
        let base = trimmed[..bracket_pos].trim();
        let len_str = trimmed[bracket_pos + 1..trimmed.len() - 1].trim();

        // char[N] is a string
        if base == "char" {
            return FieldMetadata {
                decoder: Decoder::String,
                special: None,
            };
        }

        let length = len_str.parse::<usize>().unwrap_or(match len_str {
            "MAX_ABILITY_DRAFT_ABILITIES" => 48,
            "DOTA_ABILITY_DRAFT_HEROES_PER_GAME" => 10,
            _ => 64,
        });

        let inner = get_field_metadata(
            base,
            var_name,
            bit_count,
            low_value,
            high_value,
            encode_flags,
            var_encoder,
            has_field_serializer,
        );

        return FieldMetadata {
            decoder: inner.decoder,
            special: Some(FieldSpecialDescriptor::FixedArray { length }),
        };
    }

    // Generic/template types: CNetworkUtlVectorBase< T >
    if let Some(angle_pos) = trimmed.find('<')
        && let Some(close_pos) = trimmed.rfind('>')
    {
        let base = trimmed[..angle_pos].trim();
        let inner_type = trimmed[angle_pos + 1..close_pos].trim();

        let is_vector_base = matches!(
            base,
            "CNetworkUtlVectorBase" | "CUtlVectorEmbeddedNetworkVar" | "CUtlVector"
        );

        if is_vector_base {
            if has_field_serializer {
                return FieldMetadata {
                    decoder: Decoder::U64,
                    special: Some(FieldSpecialDescriptor::DynamicSerializerArray),
                };
            }

            let inner = get_field_metadata(
                inner_type,
                var_name,
                bit_count,
                low_value,
                high_value,
                encode_flags,
                var_encoder,
                has_field_serializer,
            );

            return FieldMetadata {
                decoder: Decoder::U64,
                special: Some(FieldSpecialDescriptor::DynamicArray {
                    inner_decoder: inner.decoder,
                }),
            };
        }

        // For non-vector templates, decode as the base type
        return get_field_metadata(
            base,
            var_name,
            bit_count,
            low_value,
            high_value,
            encode_flags,
            var_encoder,
            has_field_serializer,
        );
    }

    // Identify the base type
    match trimmed {
        // Primitives
        "int8" | "int16" | "int32" | "int64" => FieldMetadata {
            decoder: Decoder::I64,
            special: None,
        },

        "bool" => FieldMetadata {
            decoder: Decoder::Bool,
            special: None,
        },

        "float32" | "CNetworkedQuantizedFloat" | "GameTime_t" => {
            let decoder = build_f32_decoder(
                var_name,
                bit_count,
                low_value,
                high_value,
                encode_flags,
                var_encoder,
            );
            FieldMetadata {
                decoder,
                special: None,
            }
        }

        // Pointer types (entity body components)
        "CBodyComponentDCGBaseAnimating"
        | "CBodyComponentBaseAnimating"
        | "CBodyComponentBaseAnimatingOverlay"
        | "CBodyComponentBaseModelEntity"
        | "CBodyComponent"
        | "CBodyComponentSkeletonInstance"
        | "CBodyComponentPoint"
        | "CLightComponent"
        | "CRenderComponent"
        | "C_BodyComponentBaseAnimating"
        | "C_BodyComponentBaseAnimatingOverlay"
        | "CPhysicsComponent" => FieldMetadata {
            decoder: Decoder::Bool,
            special: Some(FieldSpecialDescriptor::Pointer),
        },

        // String types
        "CUtlSymbolLarge" | "CUtlString" => FieldMetadata {
            decoder: Decoder::String,
            special: None,
        },

        // Angle type
        "QAngle" => {
            let bc = bit_count.unwrap_or(0) as usize;
            let decoder = if let Some(encoder) = var_encoder {
                match encoder {
                    "qangle_pitch_yaw" => Decoder::QAnglePitchYaw { bit_count: bc },
                    "qangle_precise" => Decoder::QAnglePrecise,
                    _ => {
                        if bc == 0 {
                            Decoder::QAngleCoord
                        } else {
                            Decoder::QAngleBitCount { bit_count: bc }
                        }
                    }
                }
            } else if bc == 0 {
                Decoder::QAngleCoord
            } else {
                Decoder::QAngleBitCount { bit_count: bc }
            };
            FieldMetadata {
                decoder,
                special: None,
            }
        }

        // Vector types
        "Vector" | "VectorWS" => {
            if var_encoder == Some("normal") {
                FieldMetadata {
                    decoder: Decoder::Vector3Normal,
                    special: None,
                }
            } else {
                let inner = build_f32_decoder(
                    var_name,
                    bit_count,
                    low_value,
                    high_value,
                    encode_flags,
                    var_encoder,
                );
                FieldMetadata {
                    decoder: Decoder::Vector3(Box::new(inner)),
                    special: None,
                }
            }
        }

        "Vector2D" => {
            let inner = build_f32_decoder(
                var_name,
                bit_count,
                low_value,
                high_value,
                encode_flags,
                var_encoder,
            );
            FieldMetadata {
                decoder: Decoder::Vector2(Box::new(inner)),
                special: None,
            }
        }

        "Vector4D" => {
            let inner = build_f32_decoder(
                var_name,
                bit_count,
                low_value,
                high_value,
                encode_flags,
                var_encoder,
            );
            FieldMetadata {
                decoder: Decoder::Vector4(Box::new(inner)),
                special: None,
            }
        }

        // Dynamic serializer arrays (special cases)
        "m_SpeechBubbles" | "DOTA_CombatLogQueryProgress" => FieldMetadata {
            decoder: Decoder::U64,
            special: Some(FieldSpecialDescriptor::DynamicSerializerArray),
        },

        // Default: unsigned integer
        _ => {
            let decoder = if var_encoder == Some("fixed64") {
                Decoder::U64Fixed64
            } else {
                Decoder::U64
            };
            FieldMetadata {
                decoder,
                special: None,
            }
        }
    }
}
