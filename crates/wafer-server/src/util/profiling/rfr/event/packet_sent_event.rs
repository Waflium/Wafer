use crate::rfr::EventType;

pub static TYPE: EventType = EventType::new(105, "minecraft.PacketSent", "Network Packet Sent", &["Minecraft", "Network"], true, true, super::packet_event::FIELDS);
