use windows_sys::Win32::System::SystemInformation::{GetNativeSystemInfo, SYSTEM_INFO};

use crate::jna::platform::win32::advapi32_util::{ROOT_LOCAL_MACHINE, registry_get_int_value, registry_get_keys, registry_get_string_value};
use crate::oshi::OshiResult;
use crate::oshi::driver::windows::logical_processor_information::{get_logical_processor_information_ex, physical_package_count};
use crate::oshi::hardware::central_processor::{CentralProcessor, ProcessorIdentifier};
use crate::oshi::util::parse_util::split_whitespaces;

const CPU_REGISTRY_ROOT: &str = "HARDWARE\\DESCRIPTION\\System\\CentralProcessor\\";
const PROCESSOR_ARCHITECTURE_AMD64: u16 = 9;
const PROCESSOR_ARCHITECTURE_ARM64: u16 = 12;
const PROCESSOR_ARCHITECTURE_IA64: u16 = 6;

pub struct WindowsCentralProcessor {
    physical_package_count: i32,
    physical_processor_count: i32,
    logical_processor_count: i32,
}

impl WindowsCentralProcessor {
    pub fn new() -> OshiResult<WindowsCentralProcessor> {
        let information = get_logical_processor_information_ex()?;
        Ok(WindowsCentralProcessor {
            physical_package_count: physical_package_count(&information.logical_processors) as i32,
            physical_processor_count: information.physical_processor_count as i32,
            logical_processor_count: information.logical_processors.len() as i32,
        })
    }
}

fn parse_identifier(identifier: &str, key: &str) -> String {
    let mut found = false;
    for token in split_whitespaces(identifier) {
        if found {
            return token.to_string();
        }
        found = token == key;
    }
    String::new()
}

impl CentralProcessor for WindowsCentralProcessor {
    fn get_processor_identifier(&self) -> OshiResult<ProcessorIdentifier> {
        let (mut cpu_vendor, mut cpu_name, mut cpu_identifier) = (String::new(), String::new(), String::new());
        let (mut cpu_family, mut cpu_model, mut cpu_stepping) = (String::new(), String::new(), String::new());
        let mut cpu_vendor_freq = 0i64;
        let processor_ids = registry_get_keys(ROOT_LOCAL_MACHINE, CPU_REGISTRY_ROOT)?;
        if let Some(first) = processor_ids.first() {
            let cpu_registry_path = format!("{CPU_REGISTRY_ROOT}{first}");
            cpu_vendor = registry_get_string_value(ROOT_LOCAL_MACHINE, &cpu_registry_path, "VendorIdentifier")?;
            cpu_name = registry_get_string_value(ROOT_LOCAL_MACHINE, &cpu_registry_path, "ProcessorNameString")?;
            cpu_identifier = registry_get_string_value(ROOT_LOCAL_MACHINE, &cpu_registry_path, "Identifier")?;
            if let Ok(megahertz) = registry_get_int_value(ROOT_LOCAL_MACHINE, &cpu_registry_path, "~MHz") {
                cpu_vendor_freq = i64::from(megahertz) * 1_000_000;
            }
        }
        if !cpu_identifier.is_empty() {
            cpu_family = parse_identifier(&cpu_identifier, "Family");
            cpu_model = parse_identifier(&cpu_identifier, "Model");
            cpu_stepping = parse_identifier(&cpu_identifier, "Stepping");
        }
        let mut system_info: SYSTEM_INFO = unsafe { std::mem::zeroed() };
        unsafe { GetNativeSystemInfo(&mut system_info) };
        let processor_architecture = unsafe { system_info.Anonymous.Anonymous.wProcessorArchitecture };
        let cpu_64bit = matches!(processor_architecture, PROCESSOR_ARCHITECTURE_AMD64 | PROCESSOR_ARCHITECTURE_ARM64 | PROCESSOR_ARCHITECTURE_IA64);
        Ok(ProcessorIdentifier::new(&cpu_vendor, &cpu_name, &cpu_family, &cpu_model, &cpu_stepping, cpu_64bit, cpu_vendor_freq))
    }

    fn get_physical_package_count(&self) -> i32 {
        self.physical_package_count
    }

    fn get_physical_processor_count(&self) -> i32 {
        self.physical_processor_count
    }

    fn get_logical_processor_count(&self) -> i32 {
        self.logical_processor_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_split_into_family_model_and_stepping() {
        assert_eq!(parse_identifier("AMD64 Family 25 Model 97 Stepping 2", "Family"), "25");
        assert_eq!(parse_identifier("AMD64 Family 25 Model 97 Stepping 2", "Model"), "97");
        assert_eq!(parse_identifier("AMD64 Family 25 Model 97 Stepping 2", "Stepping"), "2");
        assert_eq!(parse_identifier("Intel64", "Family"), "");
    }

    #[test]
    fn this_machine_has_a_processor_identifier() {
        let processor = WindowsCentralProcessor::new().ok().unwrap();
        let identifier = processor.get_processor_identifier().ok().unwrap();
        assert!(!identifier.get_vendor().is_empty());
        assert!(identifier.get_identifier().contains(" Family "));
        assert!(processor.get_logical_processor_count() >= processor.get_physical_processor_count());
        assert!(processor.get_physical_package_count() >= 1);
    }
}
