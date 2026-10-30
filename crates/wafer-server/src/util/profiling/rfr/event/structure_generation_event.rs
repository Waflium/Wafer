use crate::rfr::{EventType, FieldType, ValueDescriptor};

pub static TYPE: EventType = EventType::new(
    107,
    "minecraft.StructureGeneration",
    "Structure Generation",
    &["Minecraft", "World Generation"],
    true,
    true,
    &[
        ValueDescriptor::new("chunkPosX", "Chunk X Position", FieldType::Int),
        ValueDescriptor::new("chunkPosZ", "Chunk Z Position", FieldType::Int),
        ValueDescriptor::new("structure", "Structure", FieldType::String),
        ValueDescriptor::new("level", "Level", FieldType::String),
        ValueDescriptor::new("success", "Success", FieldType::Boolean),
    ],
);
