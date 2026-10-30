use crate::rfr::consumer::recording_file::RecordedEvent;
use crate::util::java_collections::java_string_hash_code;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PacketIdentification {
    pub direction: Option<String>,
    pub protocol_id: Option<String>,
    pub packet_id: Option<String>,
}

impl PacketIdentification {
    pub fn from(event: &RecordedEvent) -> PacketIdentification {
        PacketIdentification { direction: event.get_string("packetDirection"), protocol_id: event.get_string("protocolId"), packet_id: event.get_string("packetId") }
    }

    pub fn hash_code(&self) -> i32 {
        let mut result = 0i32;
        for value in [&self.direction, &self.protocol_id, &self.packet_id] {
            result = result.wrapping_mul(31).wrapping_add(java_string_hash_code(value.as_deref()));
        }
        result
    }
}
