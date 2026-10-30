use super::*;
use crate::oshi::hardware::HardwareAbstractionLayer;
use crate::oshi::hardware::central_processor::ProcessorIdentifier;
use crate::oshi::software::os::OperatingSystem;

struct FakeOperatingSystem {
    process: Option<OSProcess>,
}

impl OperatingSystem for FakeOperatingSystem {
    fn to_string(&self) -> OshiResult<String> {
        Ok("Microsoft Windows 11 build 26300".to_string())
    }

    fn is_elevated(&self) -> OshiResult<bool> {
        Ok(false)
    }

    fn get_process_id(&self) -> i32 {
        1
    }

    fn get_process(&self, _pid: i32) -> OshiResult<Option<OSProcess>> {
        Ok(self.process.clone())
    }
}

struct FakeProcessor;

impl CentralProcessor for FakeProcessor {
    fn get_processor_identifier(&self) -> OshiResult<crate::oshi::hardware::central_processor::ProcessorIdentifier> {
        Ok(ProcessorIdentifier::new("AuthenticAMD", "AMD Ryzen 9 7950X3D 16-Core Processor          ", "25", "97", "2", true, 4_200_000_000))
    }

    fn get_physical_package_count(&self) -> i32 {
        1
    }

    fn get_physical_processor_count(&self) -> i32 {
        16
    }

    fn get_logical_processor_count(&self) -> i32 {
        32
    }
}

struct FakeVirtualMemory;

impl VirtualMemory for FakeVirtualMemory {
    fn get_virtual_max(&self) -> OshiResult<i64> {
        Ok(46_345_945_088)
    }

    fn get_virtual_in_use(&self) -> OshiResult<i64> {
        Ok(34_192_998_400)
    }

    fn get_swap_total(&self) -> OshiResult<i64> {
        Ok(12_348_030_976)
    }

    fn get_swap_used(&self) -> OshiResult<i64> {
        Ok(2_102_984_704)
    }
}

struct FakeMemory;

impl GlobalMemory for FakeMemory {
    fn get_virtual_memory(&self) -> Box<dyn VirtualMemory> {
        Box::new(FakeVirtualMemory)
    }

    fn get_physical_memory(&self) -> OshiResult<Vec<PhysicalMemory>> {
        Ok(vec![
            PhysicalMemory {
                bank_label: "BANK 0".into(),
                capacity: 17_179_869_184,
                clock_speed: 5_600_000_000,
                manufacturer: "Kingston".into(),
                memory_type: "DDR5".into(),
                part_number: "x".into(),
                serial_number: "y".into(),
            },
            PhysicalMemory {
                bank_label: "BANK 2".into(),
                capacity: 17_179_869_184,
                clock_speed: 5_600_000_000,
                manufacturer: "Kingston".into(),
                memory_type: "DDR5".into(),
                part_number: "x".into(),
                serial_number: "y".into(),
            },
        ])
    }
}

struct FakeHardware {
    fail_graphics: bool,
}

impl HardwareAbstractionLayer for FakeHardware {
    fn get_processor(&self) -> OshiResult<Box<dyn CentralProcessor>> {
        Ok(Box::new(FakeProcessor))
    }

    fn get_graphics_cards(&self) -> OshiResult<Vec<GraphicsCard>> {
        if self.fail_graphics {
            return Err(JavaThrowable::new("com.sun.jna.platform.win32.Win32Exception: Access is denied.", Vec::new()));
        }
        Ok(vec![
            GraphicsCard::new("NVIDIA GeForce RTX 4080 SUPER".into(), "VideoController1".into(), "NVIDIA".into(), "32.0.16.1692".into(), 17_171_480_576),
            GraphicsCard::new("AMD Radeon(TM) Graphics".into(), "VideoController2".into(), "Advanced Micro Devices, Inc.".into(), "32.0.21043.5001".into(), 536_870_912),
        ])
    }

    fn get_memory(&self) -> OshiResult<Box<dyn GlobalMemory>> {
        Ok(Box::new(FakeMemory))
    }
}

fn report(process: Option<OSProcess>, fail_graphics: bool) -> Vec<(String, String)> {
    let process = process.clone();
    SystemReport::with_system_info(move || SystemInfo::from_parts(Box::new(FakeOperatingSystem { process: process.clone() }), Box::new(FakeHardware { fail_graphics }))).into_entries()
}

fn lines(entries: &[(String, String)]) -> Vec<String> {
    entries.iter().map(|(key, value)| format!("{key}: {value}")).collect()
}

fn keys(entries: &[(String, String)]) -> Vec<&str> {
    entries.iter().map(|(key, _)| key.as_str()).collect()
}

#[test]
fn entries_come_in_vanillas_order() {
    let entries = report(None, false);
    let expected_keys = [
        "Minecraft Version",
        "Minecraft Version ID",
        "Operating System",
        "Java Version",
        "Java VM Version",
        "Memory",
        "Memory (heap)",
        "Memory (non-heap)",
        "CPUs",
        "Processor Vendor",
        "Processor Name",
        "Identifier",
        "Microarchitecture",
        "Frequency (GHz)",
        "Number of physical packages",
        "Number of physical CPUs",
        "Number of logical CPUs",
        "Graphics card #0 name",
        "Graphics card #0 vendor",
        "Graphics card #0 VRAM (MiB)",
        "Graphics card #0 deviceId",
        "Graphics card #0 versionInfo",
        "Graphics card #1 name",
        "Graphics card #1 vendor",
        "Graphics card #1 VRAM (MiB)",
        "Graphics card #1 deviceId",
        "Graphics card #1 versionInfo",
        "Memory slot #0 capacity (MiB)",
        "Memory slot #0 clockSpeed (GHz)",
        "Memory slot #0 type",
        "Memory slot #1 capacity (MiB)",
        "Memory slot #1 clockSpeed (GHz)",
        "Memory slot #1 type",
        "Virtual memory max (MiB)",
        "Virtual memory used (MiB)",
        "Swap memory total (MiB)",
        "Swap memory used (MiB)",
        "Space in storage for jna.tmpdir (MiB)",
        "Space in storage for org.lwjgl.system.SharedLibraryExtractPath (MiB)",
        "Space in storage for io.netty.native.workdir (MiB)",
        "Space in storage for java.io.tmpdir (MiB)",
        "Space in storage for workdir (MiB)",
        "Operating System Version",
        "Process Elevated",
        "Process Loads",
        "Process Virtual Size (MiB)",
        "Process Resident Size (MiB)",
        "JVM Flags",
        "Debug Flags",
    ];
    assert_eq!(keys(&entries), expected_keys);
}

#[test]
fn hardware_values_are_formatted_like_vanilla() {
    let lines = lines(&report(None, false));
    for expected in [
        "Processor Vendor: AuthenticAMD",
        "Processor Name: AMD Ryzen 9 7950X3D 16-Core Processor          ",
        "Identifier: AuthenticAMD Family 25 Model 97 Stepping 2",
        "Microarchitecture: Zen 3",
        "Frequency (GHz): 4.20",
        "Number of physical packages: 1",
        "Number of physical CPUs: 16",
        "Number of logical CPUs: 32",
        "Graphics card #0 name: NVIDIA GeForce RTX 4080 SUPER",
        "Graphics card #0 vendor: NVIDIA",
        "Graphics card #0 VRAM (MiB): 16376.00",
        "Graphics card #0 deviceId: VideoController1",
        "Graphics card #0 versionInfo: 32.0.16.1692",
        "Graphics card #1 VRAM (MiB): 512.00",
        "Memory slot #0 capacity (MiB): 16384.00",
        "Memory slot #0 clockSpeed (GHz): 5.60",
        "Memory slot #0 type: DDR5",
        "Virtual memory max (MiB): 44198.94",
        "Virtual memory used (MiB): 32608.98",
        "Swap memory total (MiB): 11776.00",
        "Swap memory used (MiB): 2005.56",
        "Operating System Version: Microsoft Windows 11 build 26300",
        "Process Elevated: false",
        "JVM Flags: 0 total; ",
        "Debug Flags: 0 total; ",
    ] {
        assert!(lines.contains(&expected.to_string()), "missing {expected:?} in {lines:#?}");
    }
}

#[test]
fn memory_lines_use_the_emulated_heap() {
    let lines = lines(&report(None, false));
    assert!(lines.contains(&"Memory: 503316480 bytes (480 MiB) / 536870912 bytes (512 MiB) up to 8589934592 bytes (8192 MiB)".to_string()), "{lines:#?}");
    assert!(lines.contains(&"Memory (heap): init: 512MiB, used: 032MiB, committed: 512MiB, max: 8192MiB".to_string()));
    assert!(lines.contains(&"Memory (non-heap): init: 007MiB, used: 128MiB, committed: 136MiB, max: 000MiB".to_string()));
    assert!(lines.contains(&format!("CPUs: {}", java_runtime::available_processors())));
}

#[test]
fn java_fields_describe_the_bundled_runtime() {
    let lines = lines(&report(None, false));
    assert!(lines.contains(&format!("Java Version: {}, Microsoft", java_runtime::java_version())));
    assert!(lines.contains(&"Java VM Version: OpenJDK 64-Bit Server VM (mixed mode, sharing), Microsoft".to_string()));
    assert!(lines.iter().any(|line| line.starts_with("Operating System: ") && line.contains(") version ")));
    assert!(lines.contains(&format!("Minecraft Version: {}", wafer_data::VERSION_NAME)));
}

#[test]
fn a_missing_process_fails_with_null_pointer_exceptions() {
    let lines = lines(&report(None, false));
    assert!(lines.contains(&"Process Loads: ~~ERROR~~ NullPointerException: Cannot invoke \"oshi.software.os.OSProcess.getUserTime()\" because \"process\" is null".to_string()));
    assert!(lines.contains(&"Process Virtual Size (MiB): ~~ERROR~~ NullPointerException: Cannot invoke \"oshi.software.os.OSProcess.getVirtualSize()\" because \"process\" is null".to_string()));
    assert!(lines.contains(&"Process Resident Size (MiB): ~~ERROR~~ NullPointerException: Cannot invoke \"oshi.software.os.OSProcess.getResidentSetSize()\" because \"process\" is null".to_string()));
}

#[test]
fn process_details_are_formatted_like_vanilla() {
    let process = OSProcess { virtual_size: 2_147_483_648, resident_set_size: 536_870_912, kernel_time: 12_345, user_time: 67_890, up_time: 123_456 };
    let lines = lines(&report(Some(process), false));
    assert!(lines.contains(&"Process Loads: Uptime: 123s, user: 68s (54.99%), kernel: 12s (10.00%), total: 80s (64.99%)".to_string()), "{lines:#?}");
    assert!(lines.contains(&"Process Virtual Size (MiB): 2048.00".to_string()));
    assert!(lines.contains(&"Process Resident Size (MiB): 512.00".to_string()));
}

#[test]
fn failing_groups_are_recorded_under_their_name() {
    let entries = report(None, true);
    assert!(entries.contains(&("graphics".to_string(), "~~ERROR~~ Win32Exception: Access is denied.".to_string())));
    assert!(!keys(&entries).iter().any(|key| key.starts_with("Graphics card")));
    assert!(keys(&entries).contains(&"Memory slot #0 type"));
}

#[test]
fn unset_properties_and_missing_paths_are_described() {
    let lines = lines(&report(None, false));
    assert!(lines.contains(&"Space in storage for jna.tmpdir (MiB): <path not set>".to_string()));
    assert!(lines.contains(&"Space in storage for org.lwjgl.system.SharedLibraryExtractPath (MiB): <path not set>".to_string()));
    assert!(lines.contains(&"Space in storage for io.netty.native.workdir (MiB): <path not set>".to_string()));
    assert!(lines.iter().any(|line| line.starts_with("Space in storage for workdir (MiB): available: ")));
}

#[test]
fn memory_usage_is_zero_padded() {
    assert_eq!(
        print_memory_usage(MemoryUsage { init: 532_676_608, used: 269_484_032, committed: 448_790_528, max: 4_294_967_296 }),
        "init: 508MiB, used: 257MiB, committed: 428MiB, max: 4096MiB"
    );
    assert_eq!(print_memory_usage(MemoryUsage { init: 7_667_712, used: 148_897_792, committed: 159_383_552, max: -1 }), "init: 007MiB, used: 142MiB, committed: 152MiB, max: 000MiB");
}

#[test]
fn error_entries_use_the_simple_class_name() {
    assert_eq!(
        error_entry_value(&JavaThrowable::new("com.sun.jna.platform.win32.PdhUtil$PdhException: Pdh call failed with error code 0xC0000BB8", Vec::new())),
        "~~ERROR~~ PdhException: Pdh call failed with error code 0xC0000BB8"
    );
    assert_eq!(error_entry_value(&JavaThrowable::new("java.lang.IllegalStateException", Vec::new())), "~~ERROR~~ IllegalStateException: null");
}

#[test]
fn failures_print_the_vanilla_stack_below_the_lambda() {
    let captured = std::cell::RefCell::new(Vec::new());
    {
        let _scopes = java_stack::enter_all(&[
            "net.minecraft.CrashReport.<init>(CrashReport.java:35)",
            "net.minecraft.CrashReport.preload(CrashReport.java:252)",
            "net.minecraft.server.Main.main(Main.java:104)",
        ]);
        let mut system_report = SystemReport { entries: Vec::new() };
        let _init = enter("<init>", 68);
        system_report.ignore_errors("software", Some(("lambda$new$7", 68)), |report| {
            let _software = enter("putSoftware", 116);
            report.ignore_errors("process", Some(("lambda$putSoftware$0", 116)), |report| {
                let _details = enter("putProcessDetails", 189);
                report.set_detail("Process Loads", Some(("lambda$putProcessDetails$0", 190)), || {
                    *captured.borrow_mut() = java_stack::capture(&[]);
                    Ok(String::new())
                });
                Ok(())
            });
            Ok(())
        });
    }
    assert_eq!(
        captured.into_inner(),
        vec![
            "net.minecraft.SystemReport.lambda$putProcessDetails$0(SystemReport.java:190)",
            "net.minecraft.SystemReport.setDetail(SystemReport.java:96)",
            "net.minecraft.SystemReport.putProcessDetails(SystemReport.java:189)",
            "net.minecraft.SystemReport.lambda$putSoftware$0(SystemReport.java:116)",
            "net.minecraft.SystemReport.ignoreErrors(SystemReport.java:121)",
            "net.minecraft.SystemReport.putSoftware(SystemReport.java:116)",
            "net.minecraft.SystemReport.lambda$new$7(SystemReport.java:68)",
            "net.minecraft.SystemReport.ignoreErrors(SystemReport.java:121)",
            "net.minecraft.SystemReport.<init>(SystemReport.java:68)",
            "net.minecraft.CrashReport.<init>(CrashReport.java:35)",
            "net.minecraft.CrashReport.preload(CrashReport.java:252)",
            "net.minecraft.server.Main.main(Main.java:104)",
        ]
    );
}
