use super::linux_global_memory::read_mem_info;
use crate::oshi::OshiResult;
use crate::oshi::hardware::VirtualMemory;
use crate::oshi::util::file_util::read_file;
use crate::oshi::util::parse_util::{parse_long_or_default, split_whitespaces};

pub struct LinuxVirtualMemory;

fn parse_meminfo(memory_split: &[&str]) -> i64 {
    if memory_split.len() < 2 {
        return 0;
    }
    let mut memory = parse_long_or_default(memory_split[1], 0);
    if memory_split.len() > 2 && memory_split[2] == "kB" {
        memory *= 1024;
    }
    memory
}

pub fn query_mem_info_from(lines: &[String]) -> (i64, i64, i64) {
    let (mut swap_free, mut swap_total, mut commit_limit) = (0, 0, 0);
    for check_line in lines {
        let memory_split = split_whitespaces(check_line);
        if memory_split.len() > 1 {
            match memory_split[0] {
                "SwapTotal:" => swap_total = parse_meminfo(&memory_split),
                "SwapFree:" => swap_free = parse_meminfo(&memory_split),
                "CommitLimit:" => commit_limit = parse_meminfo(&memory_split),
                _ => {}
            }
        }
    }
    (swap_total - swap_free, swap_total, commit_limit)
}

fn query_mem_info() -> (i64, i64, i64) {
    query_mem_info_from(&read_file("/proc/meminfo"))
}

impl VirtualMemory for LinuxVirtualMemory {
    fn get_virtual_max(&self) -> OshiResult<i64> {
        Ok(query_mem_info().2)
    }

    fn get_virtual_in_use(&self) -> OshiResult<i64> {
        let (available, total) = read_mem_info();
        Ok(total - available + query_mem_info().0)
    }

    fn get_swap_total(&self) -> OshiResult<i64> {
        Ok(query_mem_info().1)
    }

    fn get_swap_used(&self) -> OshiResult<i64> {
        Ok(query_mem_info().0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swap_and_commit_limit_come_from_meminfo() {
        let lines: Vec<String> = ["MemTotal:       32768000 kB", "SwapTotal:       2097152 kB", "SwapFree:        1048576 kB", "CommitLimit:    18481152 kB"].iter().map(ToString::to_string).collect();
        assert_eq!(query_mem_info_from(&lines), (1048576 * 1024, 2097152 * 1024, 18481152 * 1024));
    }
}
