use std::collections::{BTreeMap, HashSet};

use crate::oshi::OshiResult;
use crate::oshi::driver::linux::lshw;
use crate::oshi::hardware::central_processor::{CentralProcessor, LogicalProcessor, ProcessorIdentifier};
use crate::oshi::software::os::platform::linux::linux_operating_system::statics;
use crate::oshi::util::executing_command::run_native;
use crate::oshi::util::file_util::{get_int_from_file, get_string_from_file, read_file};
use crate::oshi::util::parse_util::{get_first_int_value, java_trim, parse_hertz, parse_last_int, split_whitespaces_colon_whitespace};

const CPUINFO: &str = "/proc/cpuinfo";
const MODEL: &str = "/proc/device-tree/model";
const SYS_CPU: &str = "/sys/devices/system/cpu/";

pub struct LinuxCentralProcessor {
    logical_processors: Vec<LogicalProcessor>,
    physical_processor_count: i32,
}

type Topology = (Vec<LogicalProcessor>, BTreeMap<i32, i32>);

impl LinuxCentralProcessor {
    pub fn new() -> LinuxCentralProcessor {
        statics();
        let mut topology = read_topology_from_sysfs();
        if topology.0.is_empty() {
            topology = read_topology_from_cpuinfo(&read_file(CPUINFO), &map_numa_nodes_from_lscpu());
        }
        let (mut logical_processors, mut core_efficiency_map) = topology;
        if logical_processors.is_empty() {
            logical_processors.push(LogicalProcessor { processor_number: 0, physical_processor_number: 0, physical_package_number: 0, numa_node: 0, processor_group: 0 });
        }
        if core_efficiency_map.is_empty() {
            core_efficiency_map.insert(0, 0);
        }
        logical_processors.sort_by_key(|logical| logical.processor_number);
        LinuxCentralProcessor { logical_processors, physical_processor_count: core_efficiency_map.len() as i32 }
    }
}

fn read_topology_from_sysfs() -> Topology {
    let mut logical_processors = Vec::new();
    let mut core_efficiency_map = BTreeMap::new();
    match std::fs::read_dir(SYS_CPU) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if name.len() > 3 && name.starts_with("cpu") && name[3..].chars().all(|character| character.is_ascii_digit()) {
                    let syspath = format!("{}{name}", SYS_CPU);
                    logical_processors.push(logical_processor_from_syspath(&syspath, &mut core_efficiency_map));
                }
            }
        }
        Err(_) => warn!("Unable to find CPU information in sysfs at path {SYS_CPU}"),
    }
    (logical_processors, core_efficiency_map)
}

fn logical_processor_from_syspath(syspath: &str, core_efficiency_map: &mut BTreeMap<i32, i32>) -> LogicalProcessor {
    let processor = get_first_int_value(syspath);
    let core_id = get_int_from_file(&format!("{syspath}/topology/core_id"));
    let package_id = get_int_from_file(&format!("{syspath}/topology/physical_package_id"));
    let package_core_key = (package_id << 16) + core_id;
    core_efficiency_map.insert(package_core_key, get_int_from_file(&format!("{syspath}/cpu_capacity")));
    let node_prefix = format!("{syspath}/node");
    let mut node_id = 0;
    if let Ok(entries) = std::fs::read_dir(syspath) {
        let first = entries.flatten().map(|entry| entry.path()).find(|path| path.to_string_lossy().starts_with(&node_prefix));
        if let Some(first) = first {
            node_id = get_first_int_value(&first.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default());
        }
    }
    LogicalProcessor { processor_number: processor, physical_processor_number: core_id, physical_package_number: package_id, numa_node: node_id, processor_group: 0 }
}

fn map_numa_nodes_from_lscpu() -> BTreeMap<i32, i32> {
    let mut numa_node_map = BTreeMap::new();
    for line in run_native("lscpu -p=cpu,node") {
        if !line.starts_with('#')
            && let Some(position) = line.find(',').filter(|&position| position > 0)
        {
            numa_node_map.insert(line[..position].parse().unwrap_or(0), line[position + 1..].parse().unwrap_or(0));
        }
    }
    numa_node_map
}

pub fn read_topology_from_cpuinfo(proc_cpu: &[String], numa_node_map: &BTreeMap<i32, i32>) -> Topology {
    let mut logical_processors = Vec::new();
    let mut core_efficiency_map = BTreeMap::new();
    let (mut current_processor, mut current_core, mut current_package) = (0, 0, 0);
    let mut first = true;
    let node = |processor: i32| numa_node_map.get(&processor).copied().unwrap_or(0);
    for cpu in proc_cpu {
        if cpu.starts_with("processor") {
            if first {
                first = false;
            } else {
                logical_processors.push(LogicalProcessor {
                    processor_number: current_processor,
                    physical_processor_number: current_core,
                    physical_package_number: current_package,
                    numa_node: node(current_processor),
                    processor_group: 0,
                });
                core_efficiency_map.insert((current_package << 16) + current_core, 0);
            }
            current_processor = parse_last_int(cpu, 0);
        } else if cpu.starts_with("core id") || cpu.starts_with("cpu number") {
            current_core = parse_last_int(cpu, 0);
        } else if cpu.starts_with("physical id") {
            current_package = parse_last_int(cpu, 0);
        }
    }
    logical_processors.push(LogicalProcessor {
        processor_number: current_processor,
        physical_processor_number: current_core,
        physical_package_number: current_package,
        numa_node: node(current_processor),
        processor_group: 0,
    });
    core_efficiency_map.insert((current_package << 16) + current_core, 0);
    (logical_processors, core_efficiency_map)
}

pub struct ProcessorIdentifierFields {
    pub vendor: String,
    pub name: String,
    pub family: String,
    pub model: String,
    pub stepping: String,
    pub frequency: i64,
    pub cpu_64bit: bool,
}

pub fn parse_cpuinfo(lines: &[String]) -> ProcessorIdentifierFields {
    let mut fields = ProcessorIdentifierFields {
        vendor: String::new(),
        name: String::new(),
        family: String::new(),
        model: String::new(),
        stepping: String::new(),
        frequency: 0,
        cpu_64bit: false,
    };
    let mut arm_stepping = String::new();
    for line in lines {
        let split_line = split_whitespaces_colon_whitespace(line);
        if split_line.len() >= 2 {
            match split_line[0].to_lowercase().as_str() {
                "vendor_id" | "cpu implementer" => fields.vendor = split_line[1].to_string(),
                "model name" | "processor" => {
                    if !(!split_line[1].is_empty() && split_line[1].chars().all(|character| character.is_ascii_digit())) {
                        fields.name = split_line[1].to_string();
                    }
                }
                "flags" => {
                    if split_line[1].to_lowercase().split(' ').any(|flag| flag == "lm") {
                        fields.cpu_64bit = true;
                    }
                }
                "stepping" => fields.stepping = split_line[1].to_string(),
                "cpu variant" => {
                    if !arm_stepping.starts_with('r') {
                        arm_stepping.insert_str(0, &format!("r{}", parse_last_int(split_line[1], 0)));
                    }
                }
                "cpu revision" => {
                    if !arm_stepping.contains('p') {
                        arm_stepping.push('p');
                        arm_stepping.push_str(split_line[1]);
                    }
                }
                "model" | "cpu part" => fields.model = split_line[1].to_string(),
                "cpu family" => fields.family = split_line[1].to_string(),
                "cpu mhz" => fields.frequency = parse_hertz(split_line[1]),
                _ => {}
            }
        } else if line.starts_with("CPU architecture: ") {
            fields.family = java_trim(&line.replace("CPU architecture: ", "")).to_string();
        }
    }
    if fields.stepping.is_empty() {
        fields.stepping = arm_stepping;
    }
    fields
}

impl CentralProcessor for LinuxCentralProcessor {
    fn get_processor_identifier(&self) -> OshiResult<ProcessorIdentifier> {
        let mut fields = parse_cpuinfo(&read_file(CPUINFO));
        if fields.name.is_empty() {
            fields.name = java_trim(&get_string_from_file(MODEL)).to_string();
        }
        if fields.name.contains("Hz") {
            fields.frequency = -1;
        } else {
            let cpu_capacity = lshw::query_cpu_capacity();
            if cpu_capacity > fields.frequency {
                fields.frequency = cpu_capacity;
            }
        }
        if fields.vendor.starts_with("0x") || fields.model.is_empty() || fields.name.is_empty() {
            for line in run_native("lscpu") {
                if line.starts_with("Architecture:") && fields.vendor.starts_with("0x") {
                    fields.vendor = java_trim(&line.replace("Architecture:", "")).to_string();
                } else if line.starts_with("Vendor ID:") {
                    fields.vendor = java_trim(&line.replace("Vendor ID:", "")).to_string();
                } else if line.starts_with("Model name:") {
                    let model_name = java_trim(&line.replace("Model name:", "")).to_string();
                    if fields.model.is_empty() {
                        fields.model = model_name.clone();
                    }
                    if fields.name.is_empty() {
                        fields.name = model_name;
                    }
                }
            }
        }
        Ok(ProcessorIdentifier::new(&fields.vendor, &fields.name, &fields.family, &fields.model, &fields.stepping, fields.cpu_64bit, fields.frequency))
    }

    fn get_physical_package_count(&self) -> i32 {
        self.logical_processors.iter().map(|logical| logical.physical_package_number).collect::<HashSet<_>>().len() as i32
    }

    fn get_physical_processor_count(&self) -> i32 {
        self.physical_processor_count
    }

    fn get_logical_processor_count(&self) -> i32 {
        self.logical_processors.len() as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(text: &str) -> Vec<String> {
        text.lines().map(ToString::to_string).collect()
    }

    const X86_CPUINFO: &str = "processor\t: 0\nvendor_id\t: AuthenticAMD\ncpu family\t: 25\nmodel\t\t: 97\nmodel name\t: AMD Ryzen 9 7950X3D 16-Core Processor\nstepping\t: 2\ncpu MHz\t\t: 4200.000\nflags\t\t: fpu vme de lm\n\nprocessor\t: 1\nvendor_id\t: AuthenticAMD\n";

    #[test]
    fn x86_cpuinfo_provides_the_identifier_fields() {
        let fields = parse_cpuinfo(&lines(X86_CPUINFO));
        assert_eq!((fields.vendor.as_str(), fields.family.as_str(), fields.model.as_str(), fields.stepping.as_str()), ("AuthenticAMD", "25", "97", "2"));
        assert_eq!(fields.name, "AMD Ryzen 9 7950X3D 16-Core Processor");
        assert!(fields.cpu_64bit);
        assert_eq!(fields.frequency, -1);
    }

    #[test]
    fn arm_cpuinfo_builds_the_stepping_from_variant_and_revision() {
        let fields = parse_cpuinfo(&lines("processor\t: 0\nBogoMIPS\t: 50.00\nCPU implementer\t: 0x41\nCPU architecture: 8\nCPU variant\t: 0x3\nCPU part\t: 0xd0c\nCPU revision\t: 1\n"));
        assert_eq!((fields.vendor.as_str(), fields.family.as_str(), fields.model.as_str(), fields.stepping.as_str()), ("0x41", "8", "0xd0c", "r3p1"));
    }

    #[test]
    fn topology_from_cpuinfo_counts_cores_and_packages() {
        let cpuinfo = lines("processor : 0\nphysical id : 0\ncore id : 0\n\nprocessor : 1\nphysical id : 0\ncore id : 0\n\nprocessor : 2\nphysical id : 1\ncore id : 0\n");
        let (logical, cores) = read_topology_from_cpuinfo(&cpuinfo, &BTreeMap::new());
        assert_eq!(logical.len(), 3);
        assert_eq!(cores.len(), 2);
        assert_eq!(logical[2].physical_package_number, 1);
    }

    #[test]
    fn this_machine_has_processors() {
        let processor = LinuxCentralProcessor::new();
        assert!(processor.get_logical_processor_count() >= 1);
        assert!(processor.get_physical_processor_count() >= 1);
    }
}
