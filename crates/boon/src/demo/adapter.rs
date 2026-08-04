use std::collections::{HashMap, HashSet};

use boon_proto::proto::{
    CDemoClassInfo, CDemoFullPacket, CDemoPacket, CDemoSendTables, CMsgSource1LegacyGameEvent,
    CMsgSource1LegacyGameEventList, CitadelUserMessageIds, CsvcMsgCreateStringTable,
    CsvcMsgFlattenedSerializer, CsvcMsgPacketEntities, CsvcMsgServerInfo, CsvcMsgUpdateStringTable,
    CsvcMsgUserMessage, EBaseUserMessages, ECitadelGameEvents,
};
use pbdems2::demo::{CommandFrame, command as demo_command};
use pbdems2::entity::{
    BareCharEncoding, ClassEntry, CreateStringTable, DecodeProfile, FlattenedField,
    FlattenedSerializer, FlattenedSerializerDefinition, PacketEntities, PreciseQAngleMode,
    StringTableEntry, UpdateStringTable,
};
use pbdems2::io::ByteReader;
use pbdems2::{CheckpointAdapter, CommandContext, DemoAdapter};
use prost::Message;

use crate::error::{Error, Result};

use super::command::{self as boon_command, ge, svc};
use super::parser::GameEvent;

const SYMBOLIC_ARRAY_LENGTHS: &[(&str, usize)] = &[
    ("MAX_ABILITY_DRAFT_ABILITIES", 48),
    ("DOTA_ABILITY_DRAFT_HEROES_PER_GAME", 10),
];
const POINTER_TYPES: &[&str] = &["CBodyComponentDCGBaseAnimating"];
const DYNAMIC_SERIALIZER_TYPES: &[&str] = &["m_SpeechBubbles", "DOTA_CombatLogQueryProgress"];

const DECODE_PROFILE: DecodeProfile =
    DecodeProfile::new(BareCharEncoding::UnsignedVarint, PreciseQAngleMode::Raw)
        .with_pitch_yaw_qangles()
        .with_symbolic_array_lengths(SYMBOLIC_ARRAY_LENGTHS)
        .with_pointer_types(POINTER_TYPES)
        .with_dynamic_serializer_types(DYNAMIC_SERIALIZER_TYPES);

#[derive(Clone)]
pub(super) struct EventDescriptor {
    name: String,
    field_names: Vec<String>,
}

#[derive(Default)]
pub(super) struct CitadelAdapter {
    packet_body: Vec<u8>,
    descriptors: HashMap<i32, EventDescriptor>,
    tick_events: Vec<GameEvent>,
    collect_events: bool,
    event_types: Option<HashSet<u32>>,
}

impl DemoAdapter for CitadelAdapter {
    type Error = Error;

    fn handle_command(
        &mut self,
        frame: &CommandFrame<'_>,
        body: &[u8],
        context: &mut CommandContext<'_, '_>,
    ) -> Result<()> {
        let tick = frame.header().tick;
        match frame.header().cmd {
            demo_command::SEND_TABLES => {
                let command = CDemoSendTables::decode(body)?;
                context.install_serializers(decode_send_tables(command)?, DECODE_PROFILE)?;
            }
            demo_command::CLASS_INFO => {
                let command = CDemoClassInfo::decode(body)?;
                context.install_class_info(command.classes.into_iter().map(|class| {
                    ClassEntry::new(
                        class.class_id.unwrap_or_default(),
                        class.network_name.unwrap_or_default(),
                        class.table_name.unwrap_or_default(),
                    )
                }))?;
            }
            demo_command::PACKET | demo_command::SIGNON_PACKET => {
                let command = CDemoPacket::decode(body)?;
                self.handle_packet(command.data.as_deref().unwrap_or_default(), tick, context)?;
            }
            demo_command::FULL_PACKET => {
                let command = CDemoFullPacket::decode(body)?;
                if let Some(tables) = command.string_table {
                    context.apply_full_string_tables(tables.tables.into_iter().map(|table| {
                        let entries = table
                            .items
                            .into_iter()
                            .map(|item| StringTableEntry::new(item.str, item.data))
                            .collect();
                        (table.table_name.unwrap_or_default(), entries)
                    }))?;
                }
                if let Some(packet) = command.packet {
                    self.handle_packet(packet.data.as_deref().unwrap_or_default(), tick, context)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}

impl CheckpointAdapter for CitadelAdapter {
    type Checkpoint = HashMap<i32, EventDescriptor>;

    fn checkpoint(&self) -> Self::Checkpoint {
        self.descriptors.clone()
    }

    fn from_checkpoint(checkpoint: &Self::Checkpoint) -> Self {
        Self {
            descriptors: checkpoint.clone(),
            ..Self::default()
        }
    }
}

impl CitadelAdapter {
    pub(super) fn enable_events(&mut self) {
        self.collect_events = true;
        self.event_types = None;
    }

    pub(super) fn enable_event_types(&mut self, event_types: &HashSet<u32>) {
        self.collect_events = true;
        self.event_types = Some(event_types.clone());
    }

    fn wants_event_type(&self, message_type: u32) -> bool {
        self.collect_events
            && self
                .event_types
                .as_ref()
                .is_none_or(|types| types.contains(&message_type))
    }

    pub(super) fn tick_events(&self) -> &[GameEvent] {
        &self.tick_events
    }

    pub(super) fn clear_tick_events(&mut self) {
        self.tick_events.clear();
    }

    fn handle_packet(
        &mut self,
        data: &[u8],
        tick: i32,
        context: &mut CommandContext<'_, '_>,
    ) -> Result<()> {
        for message in context.packet_messages(data) {
            let message = message?;
            let message_type = message.message_type();
            let is_entity_message = matches!(
                message_type,
                svc::CREATE_STRING_TABLE
                    | svc::UPDATE_STRING_TABLE
                    | svc::SERVER_INFO
                    | svc::PACKET_ENTITIES
            );
            let is_descriptor = message_type == ge::SOURCE1_LEGACY_GAME_EVENT_LIST;
            let is_legacy_event = message_type == ge::SOURCE1_LEGACY_GAME_EVENT
                && self.wants_event_type(message_type);
            // Wrapped user messages expose their final type only after decoding
            // the small outer envelope, so they remain candidates under a filter.
            let is_wrapped_event = message_type == svc::USER_MESSAGE && self.collect_events;
            let mut direct_name = self
                .wants_event_type(message_type)
                .then(|| direct_event_name(message_type))
                .flatten();
            let is_collected_event = is_legacy_event || is_wrapped_event || direct_name.is_some();
            if !(is_descriptor || is_collected_event || is_entity_message) {
                continue;
            }

            let payload = if let Some(payload) = message.payload() {
                payload
            } else {
                message.copy_payload(&mut self.packet_body)?;
                &self.packet_body
            };

            match message_type {
                svc::CREATE_STRING_TABLE => {
                    let message = CsvcMsgCreateStringTable::decode(payload)?;
                    context.create_string_table(create_string_table(message))?;
                }
                svc::UPDATE_STRING_TABLE => {
                    let message = CsvcMsgUpdateStringTable::decode(payload)?;
                    context.update_string_table(UpdateStringTable::new(
                        message.table_id.unwrap_or_default(),
                        message.num_changed_entries.unwrap_or_default(),
                        message.string_data.unwrap_or_default(),
                    ))?;
                }
                svc::SERVER_INFO => {
                    let message = CsvcMsgServerInfo::decode(payload)?;
                    if let Some(tick_interval) = message.tick_interval {
                        context.set_tick_interval(tick_interval)?;
                    }
                }
                svc::PACKET_ENTITIES => {
                    let message = CsvcMsgPacketEntities::decode(payload)?;
                    context.apply_packet_entities(PacketEntities::new(
                        message.updated_entries.unwrap_or_default(),
                        message.entity_data.as_deref().unwrap_or_default(),
                        message.has_pvs_vis_bits_deprecated.unwrap_or_default(),
                    ))?;
                }
                ge::SOURCE1_LEGACY_GAME_EVENT_LIST => {
                    let message = CMsgSource1LegacyGameEventList::decode(payload)?;
                    for descriptor in message.descriptors {
                        let event_id = descriptor.eventid.unwrap_or_default();
                        let name = descriptor.name.unwrap_or_default();
                        let field_names = descriptor
                            .keys
                            .iter()
                            .map(|key| key.name.clone().unwrap_or_default())
                            .collect();
                        self.descriptors
                            .insert(event_id, EventDescriptor { name, field_names });
                    }
                }
                ge::SOURCE1_LEGACY_GAME_EVENT if self.collect_events => {
                    let message = CMsgSource1LegacyGameEvent::decode(payload)?;
                    let event_id = message.eventid.unwrap_or_default();
                    let (name, keys) = if let Some(descriptor) = self.descriptors.get(&event_id) {
                        let keys = descriptor
                            .field_names
                            .iter()
                            .zip(message.keys.iter())
                            .map(|(name, key)| (name.clone(), format_event_key(key)))
                            .collect();
                        (descriptor.name.clone(), keys)
                    } else {
                        (
                            message
                                .event_name
                                .unwrap_or_else(|| format!("event_{event_id}")),
                            Vec::new(),
                        )
                    };
                    self.tick_events.push(GameEvent {
                        tick,
                        name,
                        msg_type: message_type,
                        keys,
                        payload: payload.to_vec(),
                    });
                }
                svc::USER_MESSAGE if self.collect_events => {
                    let message = CsvcMsgUserMessage::decode(payload)?;
                    let inner_type = message.msg_type.unwrap_or_default();
                    if self.wants_event_type(inner_type as u32) {
                        self.tick_events.push(GameEvent {
                            tick,
                            name: boon_command::user_message_name(inner_type),
                            msg_type: inner_type as u32,
                            keys: Vec::new(),
                            payload: message.msg_data.unwrap_or_default(),
                        });
                    }
                }
                _ if direct_name.is_some() => {
                    self.tick_events.push(GameEvent {
                        tick,
                        name: direct_name.take().expect("checked above"),
                        msg_type: message_type,
                        keys: Vec::new(),
                        payload: payload.to_vec(),
                    });
                }
                _ => {}
            }
        }
        Ok(())
    }
}

fn direct_event_name(message_type: u32) -> Option<String> {
    let message_type = message_type as i32;
    CitadelUserMessageIds::try_from(message_type)
        .map(|event| event.as_str_name().to_string())
        .or_else(|_| {
            ECitadelGameEvents::try_from(message_type).map(|event| event.as_str_name().to_string())
        })
        .or_else(|_| {
            EBaseUserMessages::try_from(message_type).map(|event| event.as_str_name().to_string())
        })
        .ok()
}

fn format_event_key(key: &boon_proto::proto::c_msg_source1_legacy_game_event::KeyT) -> String {
    if let Some(ref value) = key.val_string {
        return value.clone();
    }
    if let Some(value) = key.val_float {
        return value.to_string();
    }
    if let Some(value) = key.val_long {
        return value.to_string();
    }
    if let Some(value) = key.val_short {
        return value.to_string();
    }
    if let Some(value) = key.val_byte {
        return value.to_string();
    }
    if let Some(value) = key.val_bool {
        return value.to_string();
    }
    if let Some(value) = key.val_uint64 {
        return value.to_string();
    }
    String::new()
}

fn decode_send_tables(command: CDemoSendTables) -> Result<FlattenedSerializer> {
    let data = command.data.unwrap_or_default();
    let mut reader = ByteReader::new(&data);
    let _encoded_size = reader.read_uvarint64()?;
    let remaining = reader.read_bytes(reader.remaining())?;
    let message = CsvcMsgFlattenedSerializer::decode(remaining)?;

    Ok(FlattenedSerializer::new(
        message
            .serializers
            .into_iter()
            .map(|serializer| {
                FlattenedSerializerDefinition::new(
                    serializer.serializer_name_sym,
                    serializer.fields_index,
                )
            })
            .collect(),
        message.symbols,
        message
            .fields
            .into_iter()
            .map(|field| {
                FlattenedField::new(field.var_type_sym, field.var_name_sym)
                    .with_bit_count(field.bit_count)
                    .with_range(field.low_value, field.high_value)
                    .with_encode_flags(field.encode_flags)
                    .with_serializer_name_sym(field.field_serializer_name_sym)
                    .with_send_node_sym(field.send_node_sym)
                    .with_encoder_sym(field.var_encoder_sym)
                    .with_polymorphic(!field.polymorphic_types.is_empty())
            })
            .collect(),
    ))
}

fn create_string_table(message: CsvcMsgCreateStringTable) -> CreateStringTable {
    let mut table = CreateStringTable::new(
        message.name.unwrap_or_default(),
        message.num_entries.unwrap_or_default(),
        message.string_data.unwrap_or_default(),
    )
    .with_flags(message.flags.unwrap_or_default());
    if message.user_data_fixed_size.unwrap_or_default() {
        table = table.with_fixed_user_data(
            message.user_data_size.unwrap_or_default(),
            message.user_data_size_bits.unwrap_or_default(),
        );
    }
    if message.data_compressed.unwrap_or_default() {
        table = table.with_compressed_data();
    }
    if message.using_varint_bitcounts.unwrap_or_default() {
        table = table.with_varint_bitcounts();
    }
    table
}
