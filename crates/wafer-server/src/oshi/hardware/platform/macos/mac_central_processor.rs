use std::collections::HashSet;

use crate::oshi::OshiResult;
use crate::oshi::hardware::central_processor::{CentralProcessor, ProcessorIdentifier};
use crate::oshi::util::executing_command::run_native;
use crate::oshi::util::parse_util::get_first_int_value;
use crate::oshi::util::platform::macos::sysctl_util::{sysctl_i32, sysctl_i64, sysctl_string};

const ARM_CPUTYPE: i32 = 16777228;
const M1_CPUFAMILY: i32 = 458787763;
const M2_CPUFAMILY: i32 = -634136515;
const M3_CPUFAMILY: i32 = -2023363094;
const DEFAULT_FREQUENCY: i64 = 2400000000;

pub struct MacCentralProcessor {
    logical_processor_count: i32,
    physical_processor_count: i32,
    physical_package_count: i32,
}

impl MacCentralProcessor {
    pub fn new() -> MacCentralProcessor {
        let logical_processor_count = sysctl_i32("hw.logicalcpu", 1);
        let physical_processor_count = sysctl_i32("hw.physicalcpu", 1);
        let physical_package_count = sysctl_i32("hw.packages", 1);
        let mut package_core_keys = HashSet::new();
        for i in 0..logical_processor_count {
            let core_id = i * physical_processor_count / logical_processor_count;
            let package_id = i * physical_package_count / logical_processor_count;
            package_core_keys.insert((package_id << 16) + core_id);
        }
        MacCentralProcessor { logical_processor_count, physical_processor_count: package_core_keys.len() as i32, physical_package_count }
    }
}

fn platform_expert() -> String {
    let manufacturer = run_native("ioreg -rd1 -c IOPlatformExpertDevice").into_iter().find(|line| line.contains("\"manufacturer\"")).and_then(|line| {
        let start = line.find("<\"")? + 2;
        let end = line[start..].find("\">")? + start;
        Some(line[start..end].to_string())
    });
    manufacturer.filter(|manufacturer| !manufacturer.is_empty()).unwrap_or_else(|| "Apple Inc.".to_string())
}

impl CentralProcessor for MacCentralProcessor {
    fn get_processor_identifier(&self) -> OshiResult<ProcessorIdentifier> {
        let cpu_name = sysctl_string("machdep.cpu.brand_string", "");
        let is_arm_cpu = sysctl_i32("hw.cputype", 0) == ARM_CPUTYPE;
        let (cpu_vendor, cpu_stepping, cpu_model, cpu_family);
        if cpu_name.starts_with("Apple") {
            cpu_vendor = platform_expert();
            cpu_stepping = "0".to_string();
            cpu_model = "0".to_string();
            let family = if is_arm_cpu {
                match get_first_int_value(&cpu_name) {
                    2 => M2_CPUFAMILY,
                    3 => M3_CPUFAMILY,
                    _ => M1_CPUFAMILY,
                }
            } else {
                sysctl_i32("hw.cpufamily", 0)
            };
            cpu_family = format!("0x{family:08x}");
        } else {
            cpu_vendor = sysctl_string("machdep.cpu.vendor", "");
            let value_or_empty = |name: &str| {
                let value = sysctl_i32(name, -1);
                if value < 0 { String::new() } else { value.to_string() }
            };
            cpu_stepping = value_or_empty("machdep.cpu.stepping");
            cpu_model = value_or_empty("machdep.cpu.model");
            cpu_family = value_or_empty("machdep.cpu.family");
        }
        let cpu_freq = if is_arm_cpu { DEFAULT_FREQUENCY } else { sysctl_i64("hw.cpufrequency", 0) };
        let cpu_64bit = sysctl_i32("hw.cpu64bit_capable", 0) != 0;
        Ok(ProcessorIdentifier::new(&cpu_vendor, &cpu_name, &cpu_family, &cpu_model, &cpu_stepping, cpu_64bit, cpu_freq))
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
