use super::linux_virtual_memory::LinuxVirtualMemory;
use crate::oshi::OshiResult;
use crate::oshi::hardware::{GlobalMemory, PhysicalMemory, VirtualMemory};
use crate::oshi::util::executing_command::run_native;
use crate::oshi::util::file_util::read_file;
use crate::oshi::util::parse_util::{java_trim, parse_decimal_memory_size_to_binary, parse_speed, split_whitespaces_limit};

pub struct LinuxGlobalMemory;

pub fn read_mem_info_from(lines: &[String]) -> (i64, i64) {
    let (mut mem_free, mut active_file, mut inactive_file, mut s_reclaimable, mut mem_total) = (0, 0, 0, 0, 0);
    for check_line in lines {
        let memory_split = split_whitespaces_limit(check_line, 2);
        if memory_split.len() > 1 {
            match memory_split[0] {
                "MemTotal:" => mem_total = parse_decimal_memory_size_to_binary(memory_split[1]),
                "MemAvailable:" => return (parse_decimal_memory_size_to_binary(memory_split[1]), mem_total),
                "MemFree:" => mem_free = parse_decimal_memory_size_to_binary(memory_split[1]),
                "Active(file):" => active_file = parse_decimal_memory_size_to_binary(memory_split[1]),
                "Inactive(file):" => inactive_file = parse_decimal_memory_size_to_binary(memory_split[1]),
                "SReclaimable:" => s_reclaimable = parse_decimal_memory_size_to_binary(memory_split[1]),
                _ => {}
            }
        }
    }
    (mem_free + active_file + inactive_file + s_reclaimable, mem_total)
}

pub fn read_mem_info() -> (i64, i64) {
    read_mem_info_from(&read_file("/proc/meminfo"))
}

fn split_colon(line: &str) -> Vec<&str> {
    let mut parts: Vec<&str> = java_trim(line).split(':').collect();
    while parts.len() > 1 && parts.last().is_some_and(|last| last.is_empty()) {
        parts.pop();
    }
    parts
}

pub fn physical_memory_from(dmi: &[String]) -> Vec<PhysicalMemory> {
    let mut physical_memory_list = Vec::new();
    let mut bank = 0;
    let mut bank_label = "unknown".to_string();
    let mut locator = String::new();
    let (mut capacity, mut speed) = (0i64, 0i64);
    let mut manufacturer = "unknown".to_string();
    let mut memory_type = "unknown".to_string();
    let mut part_number = "unknown".to_string();
    let mut serial_number = "unknown".to_string();
    for line in dmi {
        if java_trim(line).contains("DMI type 17") {
            let previous_bank = bank;
            bank += 1;
            if previous_bank > 0 {
                if capacity > 0 {
                    physical_memory_list.push(PhysicalMemory {
                        bank_label: format!("{bank_label}{locator}"),
                        capacity,
                        clock_speed: speed,
                        manufacturer: manufacturer.clone(),
                        memory_type: memory_type.clone(),
                        part_number: part_number.clone(),
                        serial_number: serial_number.clone(),
                    });
                }
                bank_label = "unknown".to_string();
                locator = String::new();
                capacity = 0;
                speed = 0;
            }
        } else if bank > 0 {
            let split = split_colon(line);
            if split.len() == 2 {
                match split[0] {
                    "Bank Locator" => bank_label = java_trim(split[1]).to_string(),
                    "Locator" => locator = format!("/{}", java_trim(split[1])),
                    "Size" => capacity = parse_decimal_memory_size_to_binary(java_trim(split[1])),
                    "Type" => memory_type = java_trim(split[1]).to_string(),
                    "Speed" => speed = parse_speed(split[1]),
                    "Manufacturer" => manufacturer = java_trim(split[1]).to_string(),
                    "PartNumber" | "Part Number" => part_number = java_trim(split[1]).to_string(),
                    "Serial Number" => serial_number = java_trim(split[1]).to_string(),
                    _ => {}
                }
            }
        }
    }
    if capacity > 0 {
        physical_memory_list.push(PhysicalMemory { bank_label: format!("{bank_label}{locator}"), capacity, clock_speed: speed, manufacturer, memory_type, part_number, serial_number });
    }
    physical_memory_list
}

impl GlobalMemory for LinuxGlobalMemory {
    fn get_virtual_memory(&self) -> Box<dyn VirtualMemory> {
        Box::new(LinuxVirtualMemory)
    }

    fn get_physical_memory(&self) -> OshiResult<Vec<PhysicalMemory>> {
        Ok(physical_memory_from(&run_native("dmidecode --type 17")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(text: &str) -> Vec<String> {
        text.lines().map(ToString::to_string).collect()
    }

    #[test]
    fn meminfo_prefers_memavailable() {
        assert_eq!(read_mem_info_from(&lines("MemTotal:       32768000 kB\nMemFree:         1000 kB\nMemAvailable:   20000000 kB\n")), (20000000 << 10, 32768000 << 10));
        assert_eq!(read_mem_info_from(&lines("MemTotal: 100 kB\nMemFree: 10 kB\nActive(file): 20 kB\nInactive(file): 30 kB\nSReclaimable: 5 kB\n")), (65 << 10, 100 << 10));
    }

    #[test]
    fn dmidecode_memory_devices_are_listed() {
        let output = lines(
            "# dmidecode 3.5\nHandle 0x0010, DMI type 17, 40 bytes\nMemory Device\n\tSize: 16 GB\n\tLocator: DIMM_A1\n\tBank Locator: BANK 0\n\tType: DDR5\n\tSpeed: 5600 MT/s\n\tManufacturer: Kingston\n\tSerial Number: 1234\n\tPart Number: KF560\n\nHandle 0x0011, DMI type 17, 40 bytes\nMemory Device\n\tSize: No Module Installed\n\tLocator: DIMM_A2\n",
        );
        let memory = physical_memory_from(&output);
        assert_eq!(memory.len(), 1);
        assert_eq!(memory[0].bank_label, "BANK 0/DIMM_A1");
        assert_eq!((memory[0].capacity, memory[0].clock_speed), (16 << 30, 5_600_000_000));
        assert_eq!(memory[0].memory_type, "DDR5");
    }
}
