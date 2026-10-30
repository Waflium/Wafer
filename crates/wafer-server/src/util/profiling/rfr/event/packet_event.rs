use crate::rfr::{ContentType, FieldType, ValueDescriptor};

pub const FIELDS: &[ValueDescriptor] = &[
    ValueDescriptor::new("protocolId", "Protocol Id", FieldType::String),
    ValueDescriptor::new("packetDirection", "Packet Direction", FieldType::String),
    ValueDescriptor::new("packetId", "Packet Id", FieldType::String),
    ValueDescriptor::new("remoteAddress", "Remote Address", FieldType::String),
    ValueDescriptor::new("bytes", "Bytes", FieldType::Int).with_content(ContentType::DataAmount),
];
