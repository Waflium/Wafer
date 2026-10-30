use crate::rfr::EventType;

pub static TYPE: EventType = EventType::new(104, "minecraft.PacketReceived", "Network Packet Received", &["Minecraft", "Network"], true, true, super::packet_event::FIELDS);
