use super::linux_central_processor::LinuxCentralProcessor;
use super::linux_global_memory::LinuxGlobalMemory;
use super::linux_graphics_card;
use crate::oshi::OshiResult;
use crate::oshi::hardware::central_processor::CentralProcessor;
use crate::oshi::hardware::{GlobalMemory, GraphicsCard, HardwareAbstractionLayer};

pub struct LinuxHardwareAbstractionLayer;

impl HardwareAbstractionLayer for LinuxHardwareAbstractionLayer {
    fn get_processor(&self) -> OshiResult<Box<dyn CentralProcessor>> {
        Ok(Box::new(LinuxCentralProcessor::new()))
    }

    fn get_graphics_cards(&self) -> OshiResult<Vec<GraphicsCard>> {
        Ok(linux_graphics_card::get_graphics_cards())
    }

    fn get_memory(&self) -> OshiResult<Box<dyn GlobalMemory>> {
        Ok(Box::new(LinuxGlobalMemory))
    }
}
