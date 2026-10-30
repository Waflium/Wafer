use crate::oshi::OshiResult;
use crate::oshi::hardware::central_processor::CentralProcessor;
use crate::oshi::hardware::{GlobalMemory, GraphicsCard, PhysicalMemory, VirtualMemory};
use crate::oshi::software::os::OSProcess;
use crate::oshi::system_info::SystemInfo;
use crate::util::java_heap::{self, MemoryUsage};
use crate::util::java_stack::{self, StackScope};
use crate::util::java_throwable::JavaThrowable;
use crate::util::{java_format_fixed, java_runtime};

pub const BYTES_PER_MEBIBYTE: i64 = 1048576;

type Lambda = Option<(&'static str, u32)>;

fn frame(method: &str, line: u32) -> &'static str {
    java_stack::frame(&format!("net.minecraft.SystemReport.{method}"), "SystemReport.java", line)
}

fn enter(method: &str, line: u32) -> StackScope {
    java_stack::enter(frame(method, line))
}

fn at(method: &str, line: u32) {
    java_stack::replace_top(frame(method, line));
}

fn null_pointer_exception(message: &str) -> JavaThrowable {
    JavaThrowable::new(format!("java.lang.NullPointerException: {message}"), java_stack::capture(&[]))
}

fn simple_class_name(class_name: &str) -> &str {
    class_name.rsplit(['.', '$']).next().unwrap_or(class_name)
}

fn error_entry_value(throwable: &JavaThrowable) -> String {
    format!("~~ERROR~~ {}: {}", simple_class_name(throwable.class_name()), throwable.message().unwrap_or("null"))
}

pub fn size_in_mib(bytes: i64) -> f32 {
    bytes as f32 / 1048576.0f32
}

fn format_f32(value: f32, decimals: i32) -> String {
    java_format_fixed(f64::from(value), decimals)
}

fn mib(bytes: i64) -> String {
    format_f32(size_in_mib(bytes), 2)
}

fn gigahertz(hertz: i64) -> String {
    format_f32(hertz as f32 / 1.0e9f32, 2)
}

pub fn operating_system() -> String {
    format!("{} ({}) version {}", java_runtime::os_name(), java_runtime::os_arch(), java_runtime::os_version())
}

pub fn java_version() -> String {
    format!("{}, {}", java_runtime::java_version(), java_runtime::JAVA_VENDOR)
}

pub fn java_vm_version() -> String {
    format!("{} ({}), {}", java_runtime::JAVA_VM_NAME, java_runtime::JAVA_VM_INFO, java_runtime::JAVA_VM_VENDOR)
}

pub fn print_memory_usage(memory_usage: MemoryUsage) -> String {
    format!(
        "init: {:03}MiB, used: {:03}MiB, committed: {:03}MiB, max: {:03}MiB",
        memory_usage.init / BYTES_PER_MEBIBYTE,
        memory_usage.used / BYTES_PER_MEBIBYTE,
        memory_usage.committed / BYTES_PER_MEBIBYTE,
        memory_usage.max / BYTES_PER_MEBIBYTE
    )
}

pub fn print_jvm_flags(selector: impl Fn(&str) -> bool) -> String {
    let all_arguments = java_runtime::input_arguments();
    let selected_arguments: Vec<&String> = all_arguments.iter().filter(|argument| selector(argument)).collect();
    format!("{} total; {}", selected_arguments.len(), selected_arguments.iter().map(|argument| argument.as_str()).collect::<Vec<_>>().join(" "))
}

pub fn process_loads(process: &OSProcess) -> String {
    let user_time_seconds = process.get_user_time() as f64 / 1000.0;
    let kernel_time_seconds = process.get_kernel_time() as f64 / 1000.0;
    let up_time_seconds = process.get_up_time() as f64 / 1000.0;
    let total_time_seconds = kernel_time_seconds + user_time_seconds;
    format!(
        "Uptime: {}s, user: {}s ({}%), kernel: {}s ({}%), total: {}s ({}%)",
        java_format_fixed(up_time_seconds, 0),
        java_format_fixed(user_time_seconds, 0),
        java_format_fixed(user_time_seconds / up_time_seconds * 100.0, 2),
        java_format_fixed(kernel_time_seconds, 0),
        java_format_fixed(kernel_time_seconds / up_time_seconds * 100.0, 2),
        java_format_fixed(total_time_seconds, 0),
        java_format_fixed(total_time_seconds / up_time_seconds * 100.0, 2)
    )
}

pub struct SystemReport {
    entries: Vec<(String, String)>,
}

impl SystemReport {
    pub fn new() -> SystemReport {
        SystemReport::with_system_info(SystemInfo::new)
    }

    pub fn with_system_info(system_info: impl Fn() -> SystemInfo) -> SystemReport {
        let mut report = SystemReport { entries: Vec::new() };
        let _init = enter("<init>", 43);
        report.set_detail("Minecraft Version", Some(("lambda$new$0", 43)), || Ok(wafer_data::VERSION_NAME.to_string()));
        at("<init>", 44);
        report.set_detail("Minecraft Version ID", Some(("lambda$new$1", 44)), || Ok(wafer_data::VERSION_NAME.to_string()));
        at("<init>", 45);
        report.set_detail_value("Operating System", operating_system());
        at("<init>", 46);
        report.set_detail_value("Java Version", java_version());
        at("<init>", 47);
        report.set_detail_value("Java VM Version", java_vm_version());
        at("<init>", 49);
        report.set_detail("Memory", Some(("lambda$new$2", 50)), || {
            let usage = java_heap::heap_memory_usage();
            let (max, total, free) = (usage.max, usage.committed, usage.committed - usage.used);
            Ok(format!(
                "{free} bytes ({} MiB) / {total} bytes ({} MiB) up to {max} bytes ({} MiB)",
                free / BYTES_PER_MEBIBYTE,
                total / BYTES_PER_MEBIBYTE,
                max / BYTES_PER_MEBIBYTE
            ))
        });
        at("<init>", 61);
        report.set_detail("Memory (heap)", Some(("lambda$new$3", 61)), || Ok(print_memory_usage(java_heap::heap_memory_usage())));
        at("<init>", 62);
        report.set_detail("Memory (non-heap)", Some(("lambda$new$4", 62)), || Ok(print_memory_usage(java_heap::non_heap_memory_usage())));
        at("<init>", 64);
        report.set_detail("CPUs", Some(("lambda$new$5", 64)), || Ok(java_runtime::available_processors().to_string()));
        at("<init>", 66);
        report.ignore_errors("hardware", Some(("lambda$new$6", 66)), |report| report.put_hardware(&system_info()));
        at("<init>", 68);
        report.ignore_errors("software", Some(("lambda$new$7", 68)), |report| report.put_software(&system_info()));
        at("<init>", 70);
        report.set_detail("JVM Flags", Some(("lambda$new$8", 70)), || Ok(print_jvm_flags(|argument| argument.starts_with("-X"))));
        at("<init>", 72);
        report.set_detail("Debug Flags", Some(("lambda$new$10", 72)), || Ok(print_jvm_flags(|argument| argument.starts_with("-DMC_DEBUG_"))));
        report
    }

    pub fn into_entries(self) -> Vec<(String, String)> {
        self.entries
    }

    pub fn to_line_separated_string(&self) -> String {
        let separator = if cfg!(windows) { "\r\n" } else { "\n" };
        self.entries.iter().map(|(key, value)| format!("{key}: {value}")).collect::<Vec<_>>().join(separator)
    }

    pub fn set_detail_value(&mut self, key: &str, value: impl Into<String>) {
        let _scope = enter("setDetail", 92);
        self.entries.push((key.to_string(), value.into()));
    }

    pub fn set_detail(&mut self, key: &str, lambda: Lambda, supplier: impl FnOnce() -> OshiResult<String>) {
        let _set_detail = enter("setDetail", 96);
        let _lambda = lambda.map(|(name, line)| enter(name, line));
        match supplier() {
            Ok(value) => self.entries.push((key.to_string(), value)),
            Err(throwable) => {
                warn!("Failed to get system info for {key}\n{}", throwable.print_stack_trace().trim_end());
                self.entries.push((key.to_string(), error_entry_value(&throwable)));
            }
        }
    }

    fn put_hardware(&mut self, system_info: &SystemInfo) -> OshiResult<()> {
        let _scope = enter("putHardware", 104);
        let hardware = system_info.get_hardware();
        at("putHardware", 105);
        self.ignore_errors("processor", Some(("lambda$putHardware$0", 105)), |report| report.put_processor(hardware.get_processor()?.as_ref()));
        at("putHardware", 106);
        self.ignore_errors("graphics", Some(("lambda$putHardware$1", 106)), |report| report.put_graphics(&hardware.get_graphics_cards()?));
        at("putHardware", 107);
        self.ignore_errors("memory", Some(("lambda$putHardware$2", 107)), |report| report.put_memory(hardware.get_memory()?.as_ref()));
        at("putHardware", 108);
        self.ignore_errors("storage", None, |report| report.put_storage());
        Ok(())
    }

    fn put_software(&mut self, system_info: &SystemInfo) -> OshiResult<()> {
        let _scope = enter("putSoftware", 112);
        let os = system_info.get_operating_system();
        at("putSoftware", 114);
        self.set_detail("Operating System Version", None, || os.to_string());
        at("putSoftware", 115);
        self.set_detail("Process Elevated", None, || os.is_elevated().map(|elevated| elevated.to_string()));
        at("putSoftware", 116);
        self.ignore_errors("process", Some(("lambda$putSoftware$0", 116)), |report| report.put_process_details(os.get_current_process()?.as_ref()));
        Ok(())
    }

    fn ignore_errors(&mut self, group: &str, lambda: Lambda, action: impl FnOnce(&mut SystemReport) -> OshiResult<()>) {
        let _ignore_errors = enter("ignoreErrors", 121);
        let _lambda = lambda.map(|(name, line)| enter(name, line));
        if let Err(throwable) = action(self) {
            warn!("Failed retrieving info for group {group}\n{}", throwable.print_stack_trace().trim_end());
            self.entries.push((group.to_string(), error_entry_value(&throwable)));
        }
    }

    fn put_physical_memory(&mut self, memory_packages: &[PhysicalMemory]) -> OshiResult<()> {
        let _scope = enter("putPhysicalMemory", 133);
        for (memory_slot, physical_memory) in memory_packages.iter().enumerate() {
            at("putPhysicalMemory", 135);
            let prefix = format!("Memory slot #{memory_slot} ");
            at("putPhysicalMemory", 136);
            self.set_detail(&format!("{prefix}capacity (MiB)"), Some(("lambda$putPhysicalMemory$0", 136)), || Ok(mib(physical_memory.get_capacity())));
            at("putPhysicalMemory", 137);
            self.set_detail(&format!("{prefix}clockSpeed (GHz)"), Some(("lambda$putPhysicalMemory$1", 137)), || Ok(gigahertz(physical_memory.get_clock_speed())));
            at("putPhysicalMemory", 138);
            self.set_detail(&format!("{prefix}type"), None, || Ok(physical_memory.get_memory_type().to_string()));
        }
        Ok(())
    }

    fn put_virtual_memory(&mut self, virtual_memory: &dyn VirtualMemory) -> OshiResult<()> {
        let _scope = enter("putVirtualMemory", 143);
        self.set_detail("Virtual memory max (MiB)", Some(("lambda$putVirtualMemory$0", 143)), || Ok(mib(virtual_memory.get_virtual_max()?)));
        at("putVirtualMemory", 144);
        self.set_detail("Virtual memory used (MiB)", Some(("lambda$putVirtualMemory$1", 144)), || Ok(mib(virtual_memory.get_virtual_in_use()?)));
        at("putVirtualMemory", 145);
        self.set_detail("Swap memory total (MiB)", Some(("lambda$putVirtualMemory$2", 145)), || Ok(mib(virtual_memory.get_swap_total()?)));
        at("putVirtualMemory", 146);
        self.set_detail("Swap memory used (MiB)", Some(("lambda$putVirtualMemory$3", 146)), || Ok(mib(virtual_memory.get_swap_used()?)));
        Ok(())
    }

    fn put_memory(&mut self, memory: &dyn GlobalMemory) -> OshiResult<()> {
        let _scope = enter("putMemory", 150);
        self.ignore_errors("physical memory", Some(("lambda$putMemory$0", 150)), |report| report.put_physical_memory(&memory.get_physical_memory()?));
        at("putMemory", 151);
        self.ignore_errors("virtual memory", Some(("lambda$putMemory$1", 151)), |report| report.put_virtual_memory(memory.get_virtual_memory().as_ref()));
        Ok(())
    }

    fn put_graphics(&mut self, graphics_cards: &[GraphicsCard]) -> OshiResult<()> {
        let _scope = enter("putGraphics", 155);
        for (gpu_index, graphics_card) in graphics_cards.iter().enumerate() {
            at("putGraphics", 157);
            let prefix = format!("Graphics card #{gpu_index} ");
            at("putGraphics", 158);
            self.set_detail(&format!("{prefix}name"), None, || Ok(graphics_card.get_name().to_string()));
            at("putGraphics", 159);
            self.set_detail(&format!("{prefix}vendor"), None, || Ok(graphics_card.get_vendor().to_string()));
            at("putGraphics", 160);
            self.set_detail(&format!("{prefix}VRAM (MiB)"), Some(("lambda$putGraphics$0", 160)), || Ok(mib(graphics_card.get_vram())));
            at("putGraphics", 161);
            self.set_detail(&format!("{prefix}deviceId"), None, || Ok(graphics_card.get_device_id().to_string()));
            at("putGraphics", 162);
            self.set_detail(&format!("{prefix}versionInfo"), None, || Ok(graphics_card.get_version_info().to_string()));
        }
        Ok(())
    }

    fn put_processor(&mut self, processor: &dyn CentralProcessor) -> OshiResult<()> {
        let _scope = enter("putProcessor", 167);
        let processor_identifier = processor.get_processor_identifier()?;
        at("putProcessor", 169);
        self.set_detail("Processor Vendor", None, || Ok(processor_identifier.get_vendor().to_string()));
        at("putProcessor", 170);
        self.set_detail("Processor Name", None, || Ok(processor_identifier.get_name().to_string()));
        at("putProcessor", 171);
        self.set_detail("Identifier", None, || Ok(processor_identifier.get_identifier().to_string()));
        at("putProcessor", 172);
        self.set_detail("Microarchitecture", None, || Ok(processor_identifier.get_microarchitecture()));
        at("putProcessor", 173);
        self.set_detail("Frequency (GHz)", Some(("lambda$putProcessor$0", 173)), || Ok(gigahertz(processor_identifier.get_vendor_freq())));
        at("putProcessor", 175);
        self.set_detail("Number of physical packages", Some(("lambda$putProcessor$1", 175)), || Ok(processor.get_physical_package_count().to_string()));
        at("putProcessor", 176);
        self.set_detail("Number of physical CPUs", Some(("lambda$putProcessor$2", 176)), || Ok(processor.get_physical_processor_count().to_string()));
        at("putProcessor", 177);
        self.set_detail("Number of logical CPUs", Some(("lambda$putProcessor$3", 177)), || Ok(processor.get_logical_processor_count().to_string()));
        Ok(())
    }

    fn put_storage(&mut self) -> OshiResult<()> {
        let _scope = enter("putStorage", 181);
        self.put_space_for_property("jna.tmpdir");
        at("putStorage", 182);
        self.put_space_for_property("org.lwjgl.system.SharedLibraryExtractPath");
        at("putStorage", 183);
        self.put_space_for_property("io.netty.native.workdir");
        at("putStorage", 184);
        self.put_space_for_property("java.io.tmpdir");
        at("putStorage", 185);
        self.put_space_for_path("workdir", Some(("lambda$putStorage$0", 185)), || Some(String::new()));
        Ok(())
    }

    fn put_process_details(&mut self, process: Option<&OSProcess>) -> OshiResult<()> {
        let _scope = enter("putProcessDetails", 189);
        self.set_detail("Process Loads", Some(("lambda$putProcessDetails$0", 190)), || match process {
            Some(process) => Ok(process_loads(process)),
            None => Err(null_pointer_exception("Cannot invoke \"oshi.software.os.OSProcess.getUserTime()\" because \"process\" is null")),
        });
        at("putProcessDetails", 202);
        self.set_detail("Process Virtual Size (MiB)", Some(("lambda$putProcessDetails$1", 202)), || match process {
            Some(process) => Ok(mib(process.get_virtual_size())),
            None => Err(null_pointer_exception("Cannot invoke \"oshi.software.os.OSProcess.getVirtualSize()\" because \"process\" is null")),
        });
        at("putProcessDetails", 203);
        self.set_detail("Process Resident Size (MiB)", Some(("lambda$putProcessDetails$2", 203)), || match process {
            Some(process) => Ok(mib(process.get_resident_set_size())),
            None => Err(null_pointer_exception("Cannot invoke \"oshi.software.os.OSProcess.getResidentSetSize()\" because \"process\" is null")),
        });
        Ok(())
    }

    fn put_space_for_property(&mut self, env: &str) {
        let _scope = enter("putSpaceForProperty", 207);
        self.put_space_for_path(env, Some(("lambda$putSpaceForProperty$0", 207)), || system_property(env));
    }

    fn put_space_for_path(&mut self, id: &str, lambda: Lambda, path_supplier: impl FnOnce() -> Option<String>) {
        let _scope = enter("putSpaceForPath", 211);
        let key = format!("Space in storage for {id} (MiB)");
        at("putSpaceForPath", 214);
        let path = {
            let _lambda = lambda.map(|(name, line)| enter(name, line));
            path_supplier()
        };
        let Some(path) = path else {
            at("putSpaceForPath", 216);
            self.set_detail_value(&key, "<path not set>");
            return;
        };
        at("putSpaceForPath", 220);
        match crate::util::java_file_store::get_space(&path) {
            Ok(space) => {
                at("putSpaceForPath", 221);
                self.set_detail(&key, Some(("lambda$putSpaceForPath$0", 221)), || {
                    Ok(format!("available: {}, total: {}", mib(space.usable as i64), mib(space.total as i64)))
                });
            }
            Err(crate::util::java_file_store::FileStoreError::InvalidPath(throwable)) => {
                warn!("{id} is not a path\n{}", throwable.print_stack_trace().trim_end());
                self.set_detail_value(&key, "<invalid path>");
            }
            Err(crate::util::java_file_store::FileStoreError::NoSuchFile) => self.set_detail_value(&key, "<no such file>"),
            Err(crate::util::java_file_store::FileStoreError::Other(throwable)) => {
                warn!("Failed retrieving storage space for {id}\n{}", throwable.print_stack_trace().trim_end());
                self.set_detail_value(&key, "ERR");
            }
        }
    }
}

fn system_property(name: &str) -> Option<String> {
    match name {
        "java.io.tmpdir" => java_runtime::tmpdir(),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
