use std::sync::LazyLock;

use crate::jna::platform::win32::advapi32_util::{self, ROOT_LOCAL_MACHINE, RegistryValue};

struct PerfmonDisabled {
    perf_proc_disabled: bool,
}

static PERFMON_DISABLED: LazyLock<PerfmonDisabled> = LazyLock::new(|| {
    is_disabled("oshi.os.windows.perfos.disabled", "PerfOS");
    let perf_proc_disabled = is_disabled("oshi.os.windows.perfproc.disabled", "PerfProc");
    is_disabled("oshi.os.windows.perfdisk.disabled", "PerfDisk");
    PerfmonDisabled { perf_proc_disabled }
});

pub fn perf_proc_disabled() -> bool {
    PERFMON_DISABLED.perf_proc_disabled
}

fn is_disabled(config: &str, service: &str) -> bool {
    let perf_disabled = crate::util::java_runtime::get_property(config).unwrap_or_default();
    if !perf_disabled.is_empty() {
        return perf_disabled.eq_ignore_ascii_case("true");
    }
    let key = format!("SYSTEM\\CurrentControlSet\\Services\\{service}\\Performance");
    let value = "Disable Performance Counters";
    if !advapi32_util::registry_value_exists(ROOT_LOCAL_MACHINE, &key, value).unwrap_or(false) {
        return false;
    }
    match advapi32_util::registry_get_value(ROOT_LOCAL_MACHINE, &key, value) {
        Ok(RegistryValue::Int(disabled)) => {
            if disabled > 0 {
                warn!("{service} counters are disabled and won't return data: HKEY_LOCAL_MACHINE\\{key}\\{value} > 0.");
                return true;
            }
        }
        _ => warn!("Invalid registry value type detected for {service} counters. Should be REG_DWORD. Ignoring: HKEY_LOCAL_MACHINE\\{key}\\{value}."),
    }
    false
}
