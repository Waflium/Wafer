use crate::rfr::{FieldType, ValueDescriptor};

pub const FIELDS: &[ValueDescriptor] = &[
    ValueDescriptor::new("regionPosX", "Region X Position", FieldType::Int),
    ValueDescriptor::new("regionPosZ", "Region Z Position", FieldType::Int),
    ValueDescriptor::new("localPosX", "Local X Position", FieldType::Int),
    ValueDescriptor::new("localPosZ", "Local Z Position", FieldType::Int),
    ValueDescriptor::new("chunkPosX", "Chunk X Position", FieldType::Int),
    ValueDescriptor::new("chunkPosZ", "Chunk Z Position", FieldType::Int),
    ValueDescriptor::new("level", "Level Id", FieldType::String),
    ValueDescriptor::new("dimension", "Dimension", FieldType::String),
    ValueDescriptor::new("type", "Type", FieldType::String),
    ValueDescriptor::new("compression", "Compression", FieldType::String),
    ValueDescriptor::new("bytes", "Bytes", FieldType::Int),
];
