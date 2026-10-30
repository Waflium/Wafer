use crate::rfr::EventType;

pub static TYPE: EventType = EventType::new(108, "minecraft.LoadWorld", "Create/Load World", &["Minecraft", "World Generation"], true, true, &[]);
