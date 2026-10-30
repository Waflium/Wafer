use crate::rfr::EventType;

pub static TYPE: EventType = EventType::new(102, "minecraft.ChunkRegionWrite", "Region File Write", &["Minecraft", "Storage"], true, true, super::chunk_region_io_event::FIELDS);
