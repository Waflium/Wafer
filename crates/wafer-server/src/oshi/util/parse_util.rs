use std::sync::OnceLock;

use regex_lite::Regex;

fn hertz_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"(\d+(.\d+)?) ?([kKMGT]?Hz).*").unwrap())
}

#[cfg(windows)]
fn vendor_product_id_serial() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"^.*(?:VID|VEN)_([0-9A-Fa-f]{4})&(?:PID|DEV)_([0-9A-Fa-f]{4})(.*)\\(.*)$").unwrap())
}

fn whitespaces() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"\s+").unwrap())
}

pub fn java_trim(s: &str) -> &str {
    s.trim_matches(|c: char| c <= ' ')
}

fn java_split<'a>(pattern: &Regex, s: &'a str, limit: usize) -> Vec<&'a str> {
    let mut parts = Vec::new();
    let mut index = 0;
    for found in pattern.find_iter(s) {
        if limit > 0 && parts.len() + 1 >= limit {
            break;
        }
        if found.end() == 0 {
            continue;
        }
        parts.push(&s[index..found.start()]);
        index = found.end();
    }
    if index == 0 && parts.is_empty() {
        return vec![s];
    }
    parts.push(&s[index..]);
    if limit == 0 {
        while parts.len() > 1 && parts.last().is_some_and(|last| last.is_empty()) {
            parts.pop();
        }
        if parts.len() == 1 && parts[0].is_empty() && !s.is_empty() {
            parts.pop();
        }
    }
    parts
}

pub fn split_whitespaces(s: &str) -> Vec<&str> {
    java_split(whitespaces(), s, 0)
}

fn multiplier(unit: &str) -> i64 {
    match unit {
        "Hz" => 1,
        "kHz" => 1_000,
        "MHz" => 1_000_000,
        "GHz" => 1_000_000_000,
        "THz" => 1_000_000_000_000,
        "PHz" => 1_000_000_000_000_000,
        _ => -1,
    }
}

pub fn parse_hertz(hertz: &str) -> i64 {
    if let Some(captures) = hertz_pattern().captures(java_trim(hertz)) {
        let value = captures[1].parse::<f64>().unwrap_or(f64::NAN) * multiplier(&captures[3]) as f64;
        if value >= 0.0 {
            return value as i64;
        }
    }
    -1
}

#[cfg(windows)]
pub fn parse_device_id_to_vendor_product_serial(device_id: &str) -> Option<(String, String, String)> {
    let captures = vendor_product_id_serial().captures(device_id)?;
    let vendor_id = format!("0x{}", captures[1].to_ascii_lowercase());
    let product_id = format!("0x{}", captures[2].to_ascii_lowercase());
    let serial = captures[4].to_string();
    let third = if captures[3].is_empty() && !serial.contains('&') { serial } else { String::new() };
    Some((vendor_id, product_id, third))
}

#[cfg(unix)]
mod unix {
    use super::*;

    fn bytes_pattern() -> &'static Regex {
        static PATTERN: OnceLock<Regex> = OnceLock::new();
        PATTERN.get_or_init(|| Regex::new(r"(\d+) ?([kKMGT]?B?).*").unwrap())
    }

    pub fn parse_int_or_default(s: &str, default: i32) -> i32 {
        s.parse().unwrap_or(default)
    }

    pub fn parse_long_or_default(s: &str, default: i64) -> i64 {
        s.parse().unwrap_or(default)
    }

    pub fn get_first_int_value(line: &str) -> i32 {
        get_nth_int_value(line, 1)
    }

    pub fn get_nth_int_value(line: &str, n: usize) -> i32 {
        let trimmed = line.trim_start_matches(|c: char| !c.is_ascii_digit());
        let tokens: Vec<&str> = trimmed.split(|c: char| !c.is_ascii_digit()).collect();
        if tokens.len() >= n { parse_int_or_default(tokens[n - 1], 0) } else { 0 }
    }

    pub fn parse_decimal_memory_size_to_binary(size: &str) -> i64 {
        let mut mem: Vec<String> = split_whitespaces(size).iter().map(ToString::to_string).collect();
        if mem.len() < 2
            && let Some(captures) = bytes_pattern().captures(java_trim(size))
        {
            mem = vec![captures[1].to_string(), captures[2].to_string()];
        }
        let mut capacity = parse_long_or_default(mem.first().map_or("", String::as_str), 0);
        if mem.len() == 2 && mem[1].chars().count() > 1 {
            match mem[1].chars().next() {
                Some('G') => capacity <<= 30,
                Some('K' | 'k') => capacity <<= 10,
                Some('M') => capacity <<= 20,
                Some('T') => capacity <<= 40,
                _ => {}
            }
        }
        capacity
    }
}

#[cfg(unix)]
pub use unix::*;

#[cfg(target_os = "linux")]
mod linux {
    use super::*;

    fn lspci_machine_readable() -> &'static Regex {
        static PATTERN: OnceLock<Regex> = OnceLock::new();
        PATTERN.get_or_init(|| Regex::new(r"^(.+)\s\[(.*?)\]$").unwrap())
    }

    fn lspci_memory_size() -> &'static Regex {
        static PATTERN: OnceLock<Regex> = OnceLock::new();
        PATTERN.get_or_init(|| Regex::new(r"^.+\s\[size=(\d+)([kKMGT])\]$").unwrap())
    }

    fn whitespaces_colon_whitespace() -> &'static Regex {
        static PATTERN: OnceLock<Regex> = OnceLock::new();
        PATTERN.get_or_init(|| Regex::new(r"\s+:\s").unwrap())
    }

    pub fn parse_last_string(s: &str) -> &str {
        split_whitespaces(s).last().copied().unwrap_or(s)
    }

    pub fn parse_last_int(s: &str, default: i32) -> i32 {
        let last = parse_last_string(s);
        if last.to_ascii_lowercase().starts_with("0x") { i32::from_str_radix(&last[2..], 16).unwrap_or(default) } else { last.parse().unwrap_or(default) }
    }

    pub fn split_whitespaces_limit(s: &str, limit: usize) -> Vec<&str> {
        java_split(whitespaces(), s, limit)
    }

    pub fn split_whitespaces_colon_whitespace(s: &str) -> Vec<&str> {
        java_split(whitespaces_colon_whitespace(), s, 0)
    }

    pub fn parse_speed(speed: &str) -> i64 {
        if speed.contains("T/s") { parse_hertz(&speed.replace("T/s", "Hz")) } else { parse_hertz(speed) }
    }

    pub fn parse_lshw_resource_string(resources: &str) -> i64 {
        let mut bytes = 0i64;
        for resource in split_whitespaces(resources) {
            if let Some(memory) = resource.strip_prefix("memory:") {
                let range: Vec<&str> = memory.split('-').collect();
                if range.len() == 2
                    && let (Ok(start), Ok(end)) = (i64::from_str_radix(range[0], 16), i64::from_str_radix(range[1], 16))
                {
                    bytes += end - start + 1;
                }
            }
        }
        bytes
    }

    pub fn parse_lspci_machine_readable(line: &str) -> Option<(String, String)> {
        let captures = lspci_machine_readable().captures(line)?;
        Some((captures[1].to_string(), captures[2].to_string()))
    }

    pub fn parse_lspci_memory_size(line: &str) -> i64 {
        match lspci_memory_size().captures(line) {
            Some(captures) => parse_decimal_memory_size_to_binary(&format!("{} {}B", &captures[1], &captures[2])),
            None => 0,
        }
    }
}

#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hertz_values_parse_like_oshi() {
        assert_eq!(parse_hertz("3.2 GHz"), 3_200_000_000);
        assert_eq!(parse_hertz("2400MHz"), 2_400_000_000);
        assert_eq!(parse_hertz("2400 MHz (0.4 ns)"), 2_400_000_000);
        assert_eq!(parse_hertz("nonsense"), -1);
        assert_eq!(parse_hertz("5 KHz"), -1);
    }

    #[test]
    fn splitting_follows_java() {
        assert_eq!(split_whitespaces("a  b\tc"), vec!["a", "b", "c"]);
        assert_eq!(split_whitespaces(" a b "), vec!["", "a", "b"]);
        assert_eq!(split_whitespaces(""), vec![""]);
        assert!(split_whitespaces("  ").is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn device_ids_parse() {
        assert_eq!(
            parse_device_id_to_vendor_product_serial("PCI\\VEN_10DE&DEV_2702&SUBSYS_00000000&REV_A1\\4&1234"),
            Some(("0x10de".to_string(), "0x2702".to_string(), String::new()))
        );
        assert_eq!(parse_device_id_to_vendor_product_serial("USB\\nothing"), None);
    }

    #[cfg(unix)]
    #[test]
    fn memory_sizes_parse_like_oshi() {
        assert_eq!(parse_decimal_memory_size_to_binary("16 GB"), 16 << 30);
        assert_eq!(parse_decimal_memory_size_to_binary("8192 kB"), 8192 << 10);
        assert_eq!(parse_decimal_memory_size_to_binary("512M"), 512);
        assert_eq!(parse_decimal_memory_size_to_binary("512 MB"), 512 << 20);
        assert_eq!(parse_decimal_memory_size_to_binary("No Module Installed"), 0);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn last_integers_parse_with_defaults() {
        assert_eq!(parse_last_int("processor : 12", 0), 12);
        assert_eq!(parse_last_int("cpu part : 0xd0b", 0), 0xd0b);
        assert_eq!(parse_last_int("nothing", 7), 7);
    }

    #[cfg(unix)]
    #[test]
    fn integers_parse_with_defaults() {
        assert_eq!(get_first_int_value("cpu12"), 12);
        assert_eq!(get_first_int_value("/sys/devices/system/cpu/cpu3"), 3);
        assert_eq!(get_first_int_value("none"), 0);
        assert_eq!(get_nth_int_value("15.1.2", 2), 1);
        assert_eq!(parse_int_or_default("x", 5), 5);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_command_output_parses() {
        assert_eq!(parse_speed("4800 MT/s"), 4_800_000_000);
        assert_eq!(parse_speed("4800 T/s"), 4800);
        assert_eq!(split_whitespaces_limit("MemTotal:   1024 kB", 2), vec!["MemTotal:", "1024 kB"]);
        assert_eq!(split_whitespaces_colon_whitespace("model name\t: AMD Ryzen 9"), vec!["model name", "AMD Ryzen 9"]);
        assert_eq!(split_whitespaces_colon_whitespace("flags\t\t: fpu vme"), vec!["flags", "fpu vme"]);
        assert_eq!(parse_lspci_memory_size("\tMemory at 6000000000 (64-bit, prefetchable) [size=16G]"), 16 << 30);
        assert_eq!(parse_lspci_machine_readable("NVIDIA Corporation [10de]"), Some(("NVIDIA Corporation".to_string(), "10de".to_string())));
        assert_eq!(parse_lshw_resource_string("irq:130 memory:fb000000-fbffffff memory:c0000000-cfffffff"), (0xfbffffff - 0xfb000000 + 1) + (0xcfffffff - 0xc0000000 + 1));
    }
}
