use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::{ContentType, EventType, FieldType, Value, ValueDescriptor, allocation, recording};

pub static CPU_LOAD: EventType = EventType::new(
    200,
    "rust.CPULoad",
    "CPU Load",
    &["Operating System", "Processor"],
    false,
    false,
    &[
        ValueDescriptor::new("rustUser", "Rust User", FieldType::Float).with_content(ContentType::Percentage),
        ValueDescriptor::new("rustSystem", "Rust System", FieldType::Float).with_content(ContentType::Percentage),
        ValueDescriptor::new("machineTotal", "Machine Total", FieldType::Float).with_content(ContentType::Percentage),
    ],
)
.with_description("Information about the recent CPU usage of the Rust process");

pub static THREAD_ALLOCATION_STATISTICS: EventType = EventType::new(
    201,
    "rust.ThreadAllocationStatistics",
    "Thread Allocation Statistics",
    &["Rust Application", "Statistics"],
    false,
    false,
    &[
        ValueDescriptor::new("allocated", "Allocated", FieldType::Long).with_content(ContentType::DataAmount).with_description("Approximate number of bytes allocated since thread start"),
        ValueDescriptor::new("thread", "Thread", FieldType::Thread),
    ],
);

pub static HEAP_SUMMARY: EventType = EventType::new(
    202,
    "rust.HeapSummary",
    "Heap Summary",
    &["Rust Application", "Statistics"],
    false,
    false,
    &[
        ValueDescriptor::new("heapUsed", "Heap Used", FieldType::Long).with_content(ContentType::DataAmount).with_description("Bytes allocated by live objects"),
        ValueDescriptor::new("allocated", "Allocated", FieldType::Long).with_content(ContentType::DataAmount).with_description("Bytes allocated since the process started"),
    ],
);

pub static FILE_READ: EventType = EventType::new(
    203,
    "rust.FileRead",
    "File Read",
    &["Rust Application"],
    true,
    true,
    &[
        ValueDescriptor::new("path", "Path", FieldType::String).with_description("Full path of the file, or N/A if a file descriptor was used to create the stream, for example System.in"),
        ValueDescriptor::new("bytesRead", "Bytes Read", FieldType::Long).with_content(ContentType::DataAmount).with_description("Number of bytes read from the file (possibly 0)"),
        ValueDescriptor::new("endOfFile", "End of File", FieldType::Boolean).with_description("If end of file was reached"),
    ],
)
.with_description("Reading data from a file");

pub static FILE_WRITE: EventType = EventType::new(
    204,
    "rust.FileWrite",
    "File Write",
    &["Rust Application"],
    true,
    true,
    &[
        ValueDescriptor::new("path", "Path", FieldType::String).with_description("Full path of the file, or N/A if a file descriptor was used to create the stream, for example System.out and System.err"),
        ValueDescriptor::new("bytesWritten", "Bytes Written", FieldType::Long).with_content(ContentType::DataAmount).with_description("Number of bytes written to the file"),
    ],
)
.with_description("Writing data to a file");

struct CpuSample {
    at: Instant,
    process_user: Duration,
    process_system: Duration,
    machine_busy: u64,
    machine_total: u64,
}

static LAST_CPU: Mutex<Option<CpuSample>> = Mutex::new(None);

pub fn register() {
    for event_type in [&CPU_LOAD, &THREAD_ALLOCATION_STATISTICS, &HEAP_SUMMARY, &FILE_READ, &FILE_WRITE] {
        recording::register(event_type);
    }
    recording::add_periodic_event(&CPU_LOAD, Duration::from_secs(1), emit_cpu_load);
    recording::add_periodic_event(&THREAD_ALLOCATION_STATISTICS, Duration::from_secs(5), emit_thread_allocation_statistics);
    recording::add_periodic_event(&HEAP_SUMMARY, Duration::from_secs(1), emit_heap_summary);
}

fn emit_cpu_load() {
    let Some(sample) = cpu_sample() else { return };
    let mut last = LAST_CPU.lock().unwrap();
    if let Some(previous) = last.as_ref() {
        let cpus = std::thread::available_parallelism().map_or(1, |n| n.get()) as f64;
        let wall = sample.at.duration_since(previous.at).as_secs_f64() * cpus;
        if wall > 0.0 {
            let user = (sample.process_user.saturating_sub(previous.process_user).as_secs_f64() / wall).clamp(0.0, 1.0);
            let system = (sample.process_system.saturating_sub(previous.process_system).as_secs_f64() / wall).clamp(0.0, 1.0);
            let total = sample.machine_total.saturating_sub(previous.machine_total);
            let machine = if total == 0 { 0.0 } else { (sample.machine_busy.saturating_sub(previous.machine_busy) as f64 / total as f64).clamp(user + system, 1.0) };
            CPU_LOAD.commit_now(&[Value::Float(user as f32), Value::Float(system as f32), Value::Float(machine as f32)]);
        }
    }
    *last = Some(sample);
}

fn emit_thread_allocation_statistics() {
    let now = super::ticks();
    for thread in allocation::live_threads() {
        let key = super::thread::register_other(thread.os_thread_id, &super::thread::os_thread_name(thread.os_thread_id));
        THREAD_ALLOCATION_STATISTICS.commit(now, now, &[Value::Long(thread.allocated as i64), Value::Thread(key)]);
    }
}

fn emit_heap_summary() {
    HEAP_SUMMARY.commit_now(&[Value::Long(allocation::heap_used() as i64), Value::Long(allocation::total_allocated() as i64)]);
}

#[cfg(windows)]
fn cpu_sample() -> Option<CpuSample> {
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes, GetSystemTimes};
    let zero = || FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
    let value = |time: FILETIME| (time.dwHighDateTime as u64) << 32 | time.dwLowDateTime as u64;
    let (mut creation, mut exit, mut kernel, mut user) = (zero(), zero(), zero(), zero());
    if unsafe { GetProcessTimes(GetCurrentProcess(), &mut creation, &mut exit, &mut kernel, &mut user) } == 0 {
        return None;
    }
    let (mut idle, mut machine_kernel, mut machine_user) = (zero(), zero(), zero());
    if unsafe { GetSystemTimes(&mut idle, &mut machine_kernel, &mut machine_user) } == 0 {
        return None;
    }
    let machine_total = value(machine_kernel) + value(machine_user);
    Some(CpuSample {
        at: Instant::now(),
        process_user: Duration::from_nanos(value(user) * 100),
        process_system: Duration::from_nanos(value(kernel) * 100),
        machine_busy: machine_total.saturating_sub(value(idle)),
        machine_total,
    })
}

#[cfg(target_os = "linux")]
fn cpu_sample() -> Option<CpuSample> {
    let ticks_per_second = unsafe { libc::sysconf(libc::_SC_CLK_TCK) }.max(1) as u64;
    let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
    let fields: Vec<&str> = stat.rsplit_once(')')?.1.split_whitespace().collect();
    let user: u64 = fields.get(11)?.parse().ok()?;
    let system: u64 = fields.get(12)?.parse().ok()?;
    let machine = std::fs::read_to_string("/proc/stat").ok()?;
    let times: Vec<u64> = machine.lines().next()?.split_whitespace().skip(1).filter_map(|value| value.parse().ok()).collect();
    let total: u64 = times.iter().take(8).sum();
    let idle = times.get(3).copied().unwrap_or(0) + times.get(4).copied().unwrap_or(0);
    let to_duration = |ticks: u64| Duration::from_nanos(ticks * 1_000_000_000 / ticks_per_second);
    Some(CpuSample { at: Instant::now(), process_user: to_duration(user), process_system: to_duration(system), machine_busy: total.saturating_sub(idle), machine_total: total })
}

#[cfg(not(any(windows, target_os = "linux")))]
fn cpu_sample() -> Option<CpuSample> {
    None
}
