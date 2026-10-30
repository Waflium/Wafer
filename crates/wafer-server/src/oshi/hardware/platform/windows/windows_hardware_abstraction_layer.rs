use super::windows_central_processor::WindowsCentralProcessor;
use super::windows_global_memory::WindowsGlobalMemory;
use super::windows_graphics_card;
use crate::oshi::OshiResult;
use crate::oshi::hardware::central_processor::CentralProcessor;
use crate::oshi::hardware::{GlobalMemory, GraphicsCard, HardwareAbstractionLayer};

pub struct WindowsHardwareAbstractionLayer;

impl HardwareAbstractionLayer for WindowsHardwareAbstractionLayer {
    fn get_processor(&self) -> OshiResult<Box<dyn CentralProcessor>> {
        Ok(Box::new(WindowsCentralProcessor::new()?))
    }

    fn get_graphics_cards(&self) -> OshiResult<Vec<GraphicsCard>> {
        windows_graphics_card::get_graphics_cards()
    }

    fn get_memory(&self) -> OshiResult<Box<dyn GlobalMemory>> {
        Ok(Box::new(WindowsGlobalMemory))
    }
}
