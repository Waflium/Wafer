use crate::rfr::{EventType, FieldType, ValueDescriptor};

pub static TYPE: EventType = EventType::new(
    100,
    "minecraft.ChunkGeneration",
    "Chunk Generation",
    &["Minecraft", "World Generation"],
    true,
    true,
    &[
        ValueDescriptor::new("worldPosX", "First Block X World Position", FieldType::Int),
        ValueDescriptor::new("worldPosZ", "First Block Z World Position", FieldType::Int),
        ValueDescriptor::new("chunkPosX", "Chunk X Position", FieldType::Int),
        ValueDescriptor::new("chunkPosZ", "Chunk Z Position", FieldType::Int),
        ValueDescriptor::new("status", "Status", FieldType::String),
        ValueDescriptor::new("level", "Level", FieldType::String),
    ],
);
