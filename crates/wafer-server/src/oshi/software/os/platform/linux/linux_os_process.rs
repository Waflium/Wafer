use super::linux_operating_system::statics;
use crate::oshi::software::os::OSProcess;
use crate::oshi::util::file_util::get_string_from_file;
use crate::oshi::util::parse_util::{parse_long_or_default, split_whitespaces};

struct StatFields {
    kernel_time: i64,
    user_time: i64,
    start_time: i64,
    virtual_size: i64,
    resident_set_size: i64,
}

fn parse_stat(stat: &str) -> Option<StatFields> {
    let name_end = stat.rfind(')')?;
    let fields: Vec<&str> = split_whitespaces(stat[name_end + 1..].trim()).into_iter().collect();
    let field = |index: usize| parse_long_or_default(fields.get(index).copied().unwrap_or(""), 0);
    Some(StatFields { user_time: field(11), kernel_time: field(12), start_time: field(19), virtual_size: field(20), resident_set_size: field(21) })
}

pub fn create(pid: i32) -> Option<OSProcess> {
    let stat = get_string_from_file(&format!("/proc/{pid}/stat"));
    if stat.is_empty() {
        return None;
    }
    let fields = parse_stat(&stat)?;
    let statics = statics();
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_millis() as i64);
    let mut start_time = (statics.boot_time * statics.user_hz + fields.start_time) * 1000 / statics.user_hz;
    if start_time >= now {
        start_time = now - 1;
    }
    Some(OSProcess {
        virtual_size: fields.virtual_size,
        resident_set_size: fields.resident_set_size * statics.page_size,
        kernel_time: fields.kernel_time * 1000 / statics.user_hz,
        user_time: fields.user_time * 1000 / statics.user_hz,
        up_time: now - start_time,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stat_lines_parse_after_the_command_name() {
        let stat = "1234 (my (proc)) S 1 1234 1234 0 -1 4194560 100 0 0 0 150 30 0 0 20 0 8 0 98765 123456789 4321 18446744073709551615 1 1 0 0 0 0 0 0 0 0 0 0 17 3 0 0 0 0 0";
        let fields = parse_stat(stat).unwrap();
        assert_eq!((fields.user_time, fields.kernel_time, fields.start_time, fields.virtual_size, fields.resident_set_size), (150, 30, 98765, 123456789, 4321));
    }

    #[test]
    fn the_current_process_can_be_read() {
        let process = create(std::process::id() as i32).unwrap();
        assert!(process.virtual_size > 0 && process.up_time > 0);
    }
}
