use std::sync::atomic::{AtomicI32, AtomicI64, Ordering};

use crate::rfr::{ContentType, EventType, FieldType, Value, ValueDescriptor};

pub static TYPE: EventType = EventType::new(
    109,
    "minecraft.NetworkSummary",
    "Network Summary",
    &["Minecraft", "Network"],
    true,
    true,
    &[
        ValueDescriptor::new("remoteAddress", "Remote Address", FieldType::String),
        ValueDescriptor::new("sentBytes", "Sent Bytes", FieldType::Long).with_content(ContentType::DataAmount),
        ValueDescriptor::new("sentPackets", "Sent Packets", FieldType::Int),
        ValueDescriptor::new("receivedBytes", "Received Bytes", FieldType::Long).with_content(ContentType::DataAmount),
        ValueDescriptor::new("receivedPackets", "Received Packets", FieldType::Int),
    ],
);

pub struct SumAggregation {
    sent_bytes: AtomicI64,
    sent_packets: AtomicI32,
    received_bytes: AtomicI64,
    received_packets: AtomicI32,
    remote_address: String,
    start_ticks: i64,
}

impl SumAggregation {
    pub fn new(remote_address: String) -> SumAggregation {
        SumAggregation { sent_bytes: AtomicI64::new(0), sent_packets: AtomicI32::new(0), received_bytes: AtomicI64::new(0), received_packets: AtomicI32::new(0), remote_address, start_ticks: crate::rfr::ticks() }
    }

    pub fn track_sent_packet(&self, size: i32) {
        self.sent_packets.fetch_add(1, Ordering::Relaxed);
        self.sent_bytes.fetch_add(size as i64, Ordering::Relaxed);
    }

    pub fn track_received_packet(&self, size: i32) {
        self.received_packets.fetch_add(1, Ordering::Relaxed);
        self.received_bytes.fetch_add(size as i64, Ordering::Relaxed);
    }

    pub fn commit_event(&self) {
        TYPE.commit(
            self.start_ticks,
            crate::rfr::ticks(),
            &[
                Value::String(&self.remote_address),
                Value::Long(self.sent_bytes.load(Ordering::Relaxed)),
                Value::Int(self.sent_packets.load(Ordering::Relaxed)),
                Value::Long(self.received_bytes.load(Ordering::Relaxed)),
                Value::Int(self.received_packets.load(Ordering::Relaxed)),
            ],
        );
    }
}
