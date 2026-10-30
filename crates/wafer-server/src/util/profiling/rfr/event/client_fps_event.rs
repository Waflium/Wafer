use crate::rfr::{EventType, FieldType, ValueDescriptor};

pub static TYPE: EventType = EventType::new(103, "minecraft.ClientFps", "Client fps", &["Minecraft", "Ticking"], true, true, &[ValueDescriptor::new("fps", "Client fps", FieldType::Int)]);
