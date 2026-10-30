use super::hardware::HardwareAbstractionLayer;
use super::software::os::OperatingSystem;

pub struct SystemInfo {
    operating_system: Box<dyn OperatingSystem>,
    hardware: Box<dyn HardwareAbstractionLayer>,
}

impl SystemInfo {
    pub fn new() -> SystemInfo {
        SystemInfo::from_parts(current_operating_system(), current_hardware())
    }

    pub fn from_parts(operating_system: Box<dyn OperatingSystem>, hardware: Box<dyn HardwareAbstractionLayer>) -> SystemInfo {
        SystemInfo { operating_system, hardware }
    }

    pub fn get_operating_system(&self) -> &dyn OperatingSystem {
        self.operating_system.as_ref()
    }

    pub fn get_hardware(&self) -> &dyn HardwareAbstractionLayer {
        self.hardware.as_ref()
    }
}

#[cfg(windows)]
fn current_operating_system() -> Box<dyn OperatingSystem> {
    Box::new(super::software::os::platform::windows::windows_operating_system::WindowsOperatingSystem::new())
}

#[cfg(windows)]
fn current_hardware() -> Box<dyn HardwareAbstractionLayer> {
    Box::new(super::hardware::platform::windows::windows_hardware_abstraction_layer::WindowsHardwareAbstractionLayer)
}

#[cfg(target_os = "linux")]
fn current_operating_system() -> Box<dyn OperatingSystem> {
    Box::new(super::software::os::platform::linux::linux_operating_system::LinuxOperatingSystem::new())
}

#[cfg(target_os = "linux")]
fn current_hardware() -> Box<dyn HardwareAbstractionLayer> {
    Box::new(super::hardware::platform::linux::linux_hardware_abstraction_layer::LinuxHardwareAbstractionLayer)
}

#[cfg(target_os = "macos")]
fn current_operating_system() -> Box<dyn OperatingSystem> {
    Box::new(super::software::os::platform::macos::mac_operating_system::MacOperatingSystem::new())
}

#[cfg(target_os = "macos")]
fn current_hardware() -> Box<dyn HardwareAbstractionLayer> {
    Box::new(super::hardware::platform::macos::mac_hardware_abstraction_layer::MacHardwareAbstractionLayer)
}
