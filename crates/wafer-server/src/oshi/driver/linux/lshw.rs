use crate::oshi::util::executing_command::run_native;
use crate::oshi::util::parse_util::{java_trim, parse_hertz};

pub fn query_cpu_capacity() -> i64 {
    let capacity_marker = "capacity:";
    for check_line in run_native("lshw -class processor") {
        if check_line.contains(capacity_marker) {
            return parse_hertz(java_trim(check_line.split(capacity_marker).nth(1).unwrap_or("")));
        }
    }
    -1
}
