pub mod central_processor;
pub mod platform;

use super::OshiResult;
use central_processor::CentralProcessor;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphicsCard {
    pub name: String,
    pub device_id: String,
    pub vendor: String,
    pub version_info: String,
    pub vram: i64,
}

impl GraphicsCard {
    pub fn new(name: String, device_id: String, vendor: String, version_info: String, vram: i64) -> GraphicsCard {
        GraphicsCard { name, device_id, vendor, version_info, vram }
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn get_device_id(&self) -> &str {
        &self.device_id
    }

    pub fn get_vendor(&self) -> &str {
        &self.vendor
    }

    pub fn get_version_info(&self) -> &str {
        &self.version_info
    }

    pub fn get_vram(&self) -> i64 {
        self.vram
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PhysicalMemory {
    pub bank_label: String,
    pub capacity: i64,
    pub clock_speed: i64,
    pub manufacturer: String,
    pub memory_type: String,
    pub part_number: String,
    pub serial_number: String,
}

impl PhysicalMemory {
    pub fn get_capacity(&self) -> i64 {
        self.capacity
    }

    pub fn get_clock_speed(&self) -> i64 {
        self.clock_speed
    }

    pub fn get_memory_type(&self) -> &str {
        &self.memory_type
    }
}

pub trait VirtualMemory {
    fn get_virtual_max(&self) -> OshiResult<i64>;
    fn get_virtual_in_use(&self) -> OshiResult<i64>;
    fn get_swap_total(&self) -> OshiResult<i64>;
    fn get_swap_used(&self) -> OshiResult<i64>;
}

pub trait GlobalMemory {
    fn get_virtual_memory(&self) -> Box<dyn VirtualMemory>;
    fn get_physical_memory(&self) -> OshiResult<Vec<PhysicalMemory>>;
}

pub trait HardwareAbstractionLayer {
    fn get_processor(&self) -> OshiResult<Box<dyn CentralProcessor>>;
    fn get_graphics_cards(&self) -> OshiResult<Vec<GraphicsCard>>;
    fn get_memory(&self) -> OshiResult<Box<dyn GlobalMemory>>;
}
