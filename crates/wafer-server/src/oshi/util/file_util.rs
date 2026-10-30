use std::collections::HashMap;

pub fn read_all_lines(content: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut rest = content;
    while !rest.is_empty() {
        match rest.find(['\n', '\r']) {
            Some(index) => {
                lines.push(rest[..index].to_string());
                let skip = if rest[index..].starts_with("\r\n") { 2 } else { 1 };
                rest = &rest[index + skip..];
            }
            None => {
                lines.push(rest.to_string());
                break;
            }
        }
    }
    lines
}

#[cfg(target_os = "linux")]
mod linux {
    use std::path::Path;

    use super::read_all_lines;

    pub fn read_file(filename: &str) -> Vec<String> {
        read_file_reporting(filename, true)
    }

    pub fn read_file_reporting(filename: &str, report_error: bool) -> Vec<String> {
        match std::fs::read(Path::new(filename)) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(content) => read_all_lines(&content),
                Err(e) => {
                    if report_error {
                        error!("Error reading file {filename}. Input length = {}", e.utf8_error().valid_up_to().max(1));
                    }
                    Vec::new()
                }
            },
            Err(_) => {
                if report_error {
                    warn!("File not found or not readable: {filename}");
                }
                Vec::new()
            }
        }
    }

    pub fn read_lines(filename: &str, count: usize, report_error: bool) -> Vec<String> {
        let mut lines = read_file_reporting(filename, report_error);
        lines.truncate(count);
        lines
    }

    pub fn get_int_from_file(filename: &str) -> i32 {
        match read_lines(filename, 1, false).first() {
            Some(line) => crate::oshi::util::parse_util::parse_int_or_default(line, 0),
            None => 0,
        }
    }

    pub fn get_string_from_file(filename: &str) -> String {
        read_lines(filename, 1, false).into_iter().next().unwrap_or_default()
    }
}

#[cfg(target_os = "linux")]
pub use linux::*;

pub fn parse_properties(content: &str) -> HashMap<String, String> {
    let mut properties = HashMap::new();
    for line in read_all_lines(content) {
        let line = line.trim_start_matches([' ', '\t', '\u{c}']);
        if line.is_empty() || line.starts_with('#') || line.starts_with('!') {
            continue;
        }
        let separator = line.find(['=', ':', ' ', '\t', '\u{c}']).unwrap_or(line.len());
        let key = &line[..separator];
        let mut value = &line[separator..];
        value = value.trim_start_matches([' ', '\t', '\u{c}']);
        if let Some(rest) = value.strip_prefix(['=', ':']) {
            value = rest.trim_start_matches([' ', '\t', '\u{c}']);
        }
        properties.insert(key.to_string(), value.to_string());
    }
    properties
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn properties_parse_like_java() {
        let properties = parse_properties("# comment\n\nhw_impl.0x41=ARM\n6.186 = Raptor Lake\nkey:value\n");
        assert_eq!(properties.get("hw_impl.0x41").map(String::as_str), Some("ARM"));
        assert_eq!(properties.get("6.186").map(String::as_str), Some("Raptor Lake"));
        assert_eq!(properties.get("key").map(String::as_str), Some("value"));
        assert_eq!(properties.len(), 3);
    }

    #[test]
    fn lines_split_like_files_readalllines() {
        assert_eq!(read_all_lines("a\nb\r\nc\rd"), vec!["a", "b", "c", "d"]);
        assert_eq!(read_all_lines("a\n\nb\n"), vec!["a", "", "b"]);
        assert!(read_all_lines("").is_empty());
    }
}
