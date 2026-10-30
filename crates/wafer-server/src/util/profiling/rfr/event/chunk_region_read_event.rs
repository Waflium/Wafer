use crate::rfr::EventType;

pub static TYPE: EventType = EventType::new(101, "minecraft.ChunkRegionRead", "Region File Read", &["Minecraft", "Storage"], true, true, super::chunk_region_io_event::FIELDS);
