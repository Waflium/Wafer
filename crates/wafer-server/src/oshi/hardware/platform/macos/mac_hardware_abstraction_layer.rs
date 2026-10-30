use super::mac_central_processor::MacCentralProcessor;
use super::mac_global_memory::MacGlobalMemory;
use super::mac_graphics_card;
use crate::oshi::OshiResult;
use crate::oshi::hardware::central_processor::CentralProcessor;
use crate::oshi::hardware::{GlobalMemory, GraphicsCard, HardwareAbstractionLayer};

pub struct MacHardwareAbstractionLayer;

impl HardwareAbstractionLayer for MacHardwareAbstractionLayer {
    fn get_processor(&self) -> OshiResult<Box<dyn CentralProcessor>> {
        Ok(Box::new(MacCentralProcessor::new()))
    }

    fn get_graphics_cards(&self) -> OshiResult<Vec<GraphicsCard>> {
        Ok(mac_graphics_card::get_graphics_cards())
    }

    fn get_memory(&self) -> OshiResult<Box<dyn GlobalMemory>> {
        Ok(Box::new(MacGlobalMemory))
    }
}
