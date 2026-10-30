use std::collections::HashSet;

use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::System::SystemInformation::{GetLogicalProcessorInformationEx, SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX};

use crate::jna::platform::win32::win32_exception::Win32Exception;
use crate::oshi::hardware::central_processor::LogicalProcessor;

const RELATION_ALL: i32 = 65535;
const RELATION_PROCESSOR_CORE: i32 = 0;
const RELATION_NUMA_NODE: i32 = 1;
const RELATION_PROCESSOR_PACKAGE: i32 = 3;
const GROUP_MASK_OFFSET: usize = 32;
const GROUP_AFFINITY_SIZE: usize = 16;
const GET_LOGICAL_PROCESSOR_INFORMATION_EX_FRAME: &str = "com.sun.jna.platform.win32.Kernel32Util.getLogicalProcessorInformationEx(Kernel32Util.java:1163)";

#[derive(Clone, Copy)]
struct GroupAffinity {
    mask: u64,
    group: u16,
}

impl GroupAffinity {
    fn sort_key(&self) -> u64 {
        u64::from(self.group) * 64 + u64::from(self.mask.trailing_zeros())
    }

    fn has(&self, group: u16, processor: u32) -> bool {
        self.mask & (1u64 << processor) != 0 && self.group == group
    }
}

pub struct LogicalProcessorInformation {
    pub logical_processors: Vec<LogicalProcessor>,
    pub physical_processor_count: usize,
}

fn read_u32(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap())
}

fn read_u16(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(data[offset..offset + 2].try_into().unwrap())
}

fn read_group_affinity(data: &[u8], offset: usize) -> GroupAffinity {
    GroupAffinity { mask: u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap()), group: read_u16(data, offset + 8) }
}

fn query_buffer() -> Result<Vec<u8>, Win32Exception> {
    let mut length = 0u32;
    unsafe { GetLogicalProcessorInformationEx(RELATION_ALL, std::ptr::null_mut(), &mut length) };
    let mut buffer = vec![0u8; length as usize];
    if unsafe { GetLogicalProcessorInformationEx(RELATION_ALL, buffer.as_mut_ptr().cast::<SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX>(), &mut length) } == 0 {
        return Err(Win32Exception::new(unsafe { GetLastError() } as i32, &[GET_LOGICAL_PROCESSOR_INFORMATION_EX_FRAME]));
    }
    buffer.truncate(length as usize);
    Ok(buffer)
}

pub fn get_logical_processor_information_ex() -> Result<LogicalProcessorInformation, Win32Exception> {
    Ok(parse_logical_processor_information(&query_buffer()?))
}

fn parse_logical_processor_information(buffer: &[u8]) -> LogicalProcessorInformation {
    let mut packages: Vec<Vec<GroupAffinity>> = Vec::new();
    let mut cores: Vec<GroupAffinity> = Vec::new();
    let mut numa_nodes: Vec<(u32, GroupAffinity)> = Vec::new();
    let mut offset = 0;
    while offset + 8 <= buffer.len() {
        let relationship = read_u32(buffer, offset) as i32;
        let size = read_u32(buffer, offset + 4) as usize;
        if size == 0 || offset + size > buffer.len() {
            break;
        }
        let record = &buffer[offset..offset + size];
        match relationship {
            RELATION_PROCESSOR_CORE => cores.push(read_group_affinity(record, GROUP_MASK_OFFSET)),
            RELATION_NUMA_NODE => numa_nodes.push((read_u32(record, 8), read_group_affinity(record, GROUP_MASK_OFFSET))),
            RELATION_PROCESSOR_PACKAGE => {
                let group_count = read_u16(record, 30) as usize;
                packages.push((0..group_count).map(|group| read_group_affinity(record, GROUP_MASK_OFFSET + group * GROUP_AFFINITY_SIZE)).collect());
            }
            _ => {}
        }
        offset += size;
    }
    cores.sort_by_key(GroupAffinity::sort_key);
    packages.sort_by_key(|package| package[0].sort_key());
    numa_nodes.sort_by_key(|(node, _)| *node);
    let mut logical_processors = Vec::new();
    for (node_number, node) in &numa_nodes {
        if node.mask == 0 {
            continue;
        }
        let low_bit = node.mask.trailing_zeros();
        let high_bit = 63 - node.mask.leading_zeros();
        for processor in low_bit..=high_bit {
            if node.mask & (1u64 << processor) != 0 {
                let core_id = cores.iter().position(|core| core.has(node.group, processor)).unwrap_or(0);
                let package_id = packages.iter().position(|package| package.iter().any(|affinity| affinity.has(node.group, processor))).unwrap_or(0);
                logical_processors.push(LogicalProcessor {
                    processor_number: processor as i32,
                    physical_processor_number: core_id as i32,
                    physical_package_number: package_id as i32,
                    numa_node: *node_number as i32,
                    processor_group: i32::from(node.group),
                });
            }
        }
    }
    LogicalProcessorInformation { logical_processors, physical_processor_count: cores.len() }
}

pub fn physical_package_count(logical_processors: &[LogicalProcessor]) -> usize {
    logical_processors.iter().map(|logical| logical.physical_package_number).collect::<HashSet<_>>().len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(relationship: i32, node: u32, group_masks: &[(u64, u16)]) -> Vec<u8> {
        let mut record = vec![0u8; GROUP_MASK_OFFSET + group_masks.len().max(1) * GROUP_AFFINITY_SIZE];
        record[0..4].copy_from_slice(&relationship.to_le_bytes());
        let size = record.len() as u32;
        record[4..8].copy_from_slice(&size.to_le_bytes());
        record[8..12].copy_from_slice(&node.to_le_bytes());
        record[30..32].copy_from_slice(&(group_masks.len() as u16).to_le_bytes());
        for (index, (mask, group)) in group_masks.iter().enumerate() {
            let at = GROUP_MASK_OFFSET + index * GROUP_AFFINITY_SIZE;
            record[at..at + 8].copy_from_slice(&mask.to_le_bytes());
            record[at + 8..at + 10].copy_from_slice(&group.to_le_bytes());
        }
        record
    }

    #[test]
    fn two_packages_with_hyperthreading_are_counted() {
        let mut buffer = Vec::new();
        buffer.extend(record(RELATION_PROCESSOR_CORE, 0, &[(0b0011, 0)]));
        buffer.extend(record(RELATION_PROCESSOR_CORE, 0, &[(0b1100, 0)]));
        buffer.extend(record(RELATION_PROCESSOR_CORE, 0, &[(0b0011_0000, 0)]));
        buffer.extend(record(RELATION_PROCESSOR_PACKAGE, 0, &[(0b1111, 0)]));
        buffer.extend(record(RELATION_PROCESSOR_PACKAGE, 0, &[(0b0011_0000, 0)]));
        buffer.extend(record(RELATION_NUMA_NODE, 0, &[(0b0011_1111, 0)]));
        let information = parse_logical_processor_information(&buffer);
        assert_eq!(information.logical_processors.len(), 6);
        assert_eq!(information.physical_processor_count, 3);
        assert_eq!(physical_package_count(&information.logical_processors), 2);
        assert_eq!(information.logical_processors[5].physical_package_number, 1);
        assert_eq!(information.logical_processors[2].physical_processor_number, 1);
    }

    #[test]
    fn this_machine_reports_processors() {
        let information = get_logical_processor_information_ex().ok().unwrap();
        assert!(!information.logical_processors.is_empty());
        assert!(information.physical_processor_count >= 1);
    }
}
