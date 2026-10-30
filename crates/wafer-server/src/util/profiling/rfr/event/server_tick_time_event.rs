use crate::rfr::{ContentType, EventType, FieldType, ValueDescriptor};

pub static TYPE: EventType = EventType::new(
    106,
    "minecraft.ServerTickTime",
    "Server Tick Time",
    &["Minecraft", "Ticking"],
    true,
    true,
    &[ValueDescriptor::new("averageTickDuration", "Average Server Tick Duration", FieldType::Long).with_content(ContentType::Timespan)],
);
