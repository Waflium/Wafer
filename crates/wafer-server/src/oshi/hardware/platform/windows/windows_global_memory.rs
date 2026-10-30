use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::System::ProcessStatus::{GetPerformanceInfo, PERFORMANCE_INFORMATION};

use super::windows_virtual_memory::WindowsVirtualMemory;
use crate::oshi::OshiResult;
use crate::oshi::driver::windows::wmi::win32_physical_memory;
use crate::oshi::hardware::{GlobalMemory, PhysicalMemory, VirtualMemory};

pub struct WindowsGlobalMemory;

pub fn read_perf_info() -> (i64, i64, i64) {
    let mut performance_info: PERFORMANCE_INFORMATION = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<PERFORMANCE_INFORMATION>() as u32;
    performance_info.cb = size;
    if unsafe { GetPerformanceInfo(&mut performance_info, size) } == 0 {
        error!("Failed to get Performance Info. Error code: {}", unsafe { GetLastError() });
        return (0, 0, 4098);
    }
    let page_size = performance_info.PageSize as i64;
    (page_size * performance_info.PhysicalAvailable as i64, page_size * performance_info.PhysicalTotal as i64, page_size)
}

impl GlobalMemory for WindowsGlobalMemory {
    fn get_virtual_memory(&self) -> Box<dyn VirtualMemory> {
        Box::new(WindowsVirtualMemory::new())
    }

    fn get_physical_memory(&self) -> OshiResult<Vec<PhysicalMemory>> {
        let bank_map = win32_physical_memory::queryphysical_memory();
        let mut physical_memory_list = Vec::new();
        for index in 0..bank_map.get_result_count() {
            let bank_label = bank_map.get_string(index, win32_physical_memory::BANKLABEL);
            let capacity = bank_map.get_uint64(index, win32_physical_memory::CAPACITY);
            let speed = i64::from(bank_map.get_int(index, win32_physical_memory::SPEED)).wrapping_mul(1_000_000);
            let manufacturer = bank_map.get_string(index, win32_physical_memory::MANUFACTURER);
            let memory_type = sm_bios_memory_type(bank_map.get_int(index, win32_physical_memory::SMBIOSMEMORYTYPE));
            let part_number = bank_map.get_string(index, win32_physical_memory::PARTNUMBER);
            let serial_number = bank_map.get_string(index, win32_physical_memory::SERIALNUMBER);
            physical_memory_list.push(PhysicalMemory {
                bank_label,
                capacity,
                clock_speed: speed,
                manufacturer,
                memory_type: memory_type.to_string(),
                part_number,
                serial_number,
            });
        }
        Ok(physical_memory_list)
    }
}

pub fn sm_bios_memory_type(memory_type: i32) -> &'static str {
    match memory_type {
        1 => "Other",
        3 => "DRAM",
        4 => "EDRAM",
        5 => "VRAM",
        6 => "SRAM",
        7 => "RAM",
        8 => "ROM",
        9 => "FLASH",
        10 => "EEPROM",
        11 => "FEPROM",
        12 => "EPROM",
        13 => "CDRAM",
        14 => "3DRAM",
        15 => "SDRAM",
        16 => "SGRAM",
        17 => "RDRAM",
        18 => "DDR",
        19 => "DDR2",
        20 => "DDR2 FB-DIMM",
        24 => "DDR3",
        25 => "FBD2",
        26 => "DDR4",
        27 => "LPDDR",
        28 => "LPDDR2",
        29 => "LPDDR3",
        30 => "LPDDR4",
        31 => "Logical non-volatile device",
        32 => "HBM",
        33 => "HBM2",
        34 => "DDR5",
        35 => "LPDDR5",
        36 => "HBM3",
        _ => "Unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smbios_memory_types_are_named() {
        assert_eq!(sm_bios_memory_type(34), "DDR5");
        assert_eq!(sm_bios_memory_type(26), "DDR4");
        assert_eq!(sm_bios_memory_type(2), "Unknown");
        assert_eq!(sm_bios_memory_type(0), "Unknown");
        assert_eq!(sm_bios_memory_type(99), "Unknown");
    }

    #[test]
    fn performance_info_reports_memory() {
        let (available, total, page_size) = read_perf_info();
        assert!(total > 0 && available <= total && page_size >= 4096);
    }
}
