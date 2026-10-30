use super::mac_virtual_memory::MacVirtualMemory;
use crate::oshi::OshiResult;
use crate::oshi::hardware::{GlobalMemory, PhysicalMemory, VirtualMemory};
use crate::oshi::util::executing_command::run_native;
use crate::oshi::util::parse_util::{java_trim, parse_decimal_memory_size_to_binary, parse_hertz};
use crate::oshi::util::platform::macos::sysctl_util::sysctl_i64;

pub struct MacGlobalMemory;

pub fn query_phys_mem() -> i64 {
    sysctl_i64("hw.memsize", 0)
}

pub fn query_page_size() -> i64 {
    sysctl_i64("hw.pagesize", 4096)
}

pub fn query_available() -> i64 {
    let vm_stat = run_native("vm_stat");
    let page_size = vm_stat.first().and_then(|header| header.split("page size of ").nth(1)).and_then(|rest| rest.split(' ').next()).and_then(|size| size.parse::<i64>().ok()).unwrap_or_else(query_page_size);
    let pages = |label: &str| {
        vm_stat
            .iter()
            .find(|line| line.starts_with(label))
            .and_then(|line| line.rsplit(':').next())
            .and_then(|value| java_trim(value).trim_end_matches('.').parse::<i64>().ok())
            .unwrap_or(0)
    };
    (pages("Pages free") + pages("Pages inactive")) * page_size
}

pub fn physical_memory_from(sp: &[String]) -> Vec<PhysicalMemory> {
    let mut physical_memory_list = Vec::new();
    let mut bank = 0;
    let mut bank_label = "unknown".to_string();
    let (mut capacity, mut speed) = (0i64, 0i64);
    let mut manufacturer = "unknown".to_string();
    let mut memory_type = "unknown".to_string();
    let mut part_number = "unknown".to_string();
    let mut serial_number = "unknown".to_string();
    for line in sp {
        let trimmed = java_trim(line);
        if trimmed.starts_with("BANK") {
            let previous = bank;
            bank += 1;
            if previous > 0 {
                physical_memory_list.push(PhysicalMemory {
                    bank_label: bank_label.clone(),
                    capacity,
                    clock_speed: speed,
                    manufacturer: manufacturer.clone(),
                    memory_type: memory_type.clone(),
                    part_number: "unknown".to_string(),
                    serial_number: serial_number.clone(),
                });
            }
            bank_label = trimmed.to_string();
            if let Some(colon) = bank_label.rfind(':').filter(|&colon| colon > 0) {
                bank_label.truncate(colon - 1);
            }
        } else if bank > 0 {
            let mut split: Vec<&str> = trimmed.split(':').collect();
            while split.len() > 1 && split.last().is_some_and(|last| last.is_empty()) {
                split.pop();
            }
            if split.len() == 2 {
                match split[0] {
                    "Size" => capacity = parse_decimal_memory_size_to_binary(java_trim(split[1])),
                    "Type" => memory_type = java_trim(split[1]).to_string(),
                    "Speed" => speed = parse_hertz(split[1]),
                    "Manufacturer" => manufacturer = java_trim(split[1]).to_string(),
                    "Part Number" => part_number = java_trim(split[1]).to_string(),
                    "Serial Number" => serial_number = java_trim(split[1]).to_string(),
                    _ => {}
                }
            }
        }
    }
    physical_memory_list.push(PhysicalMemory { bank_label, capacity, clock_speed: speed, manufacturer, memory_type, part_number, serial_number });
    physical_memory_list
}

impl GlobalMemory for MacGlobalMemory {
    fn get_virtual_memory(&self) -> Box<dyn VirtualMemory> {
        Box::new(MacVirtualMemory)
    }

    fn get_physical_memory(&self) -> OshiResult<Vec<PhysicalMemory>> {
        Ok(physical_memory_from(&run_native("system_profiler SPMemoryDataType")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_profiler_memory_banks_are_listed() {
        let output: Vec<String> = "Memory:\n\n    Memory Slots:\n\n      ECC: Disabled\n\n        BANK 0/DIMM0:\n\n          Size: 16 GB\n          Type: DDR4\n          Speed: 2667 MHz\n          Manufacturer: Samsung\n".lines().map(ToString::to_string).collect();
        let memory = physical_memory_from(&output);
        assert_eq!(memory.len(), 1);
        assert_eq!((memory[0].capacity, memory[0].clock_speed, memory[0].memory_type.as_str()), (16 << 30, 2_667_000_000, "DDR4"));
    }
}
