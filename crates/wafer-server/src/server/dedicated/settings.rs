use std::collections::HashMap;
use std::io;
use std::path::Path;

use chrono::{Local, Offset};
use wafer_core::java_string::{from_utf16, utf16_units};

use crate::util::java_integer::{parse_int, parse_long};

#[cfg(windows)]
const LINE_SEPARATOR: &str = "\r\n";
#[cfg(not(windows))]
const LINE_SEPARATOR: &str = "\n";

#[derive(Clone, Default, Debug)]
pub struct Properties {
    map: HashMap<String, String>,
}

impl Properties {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.map.get(key).map(String::as_str)
    }

    pub fn put(&mut self, key: &str, value: String) {
        self.map.insert(key.to_string(), value);
    }

    pub fn remove(&mut self, key: &str) -> Option<String> {
        self.map.remove(key)
    }

    pub fn load(text: &str) -> Result<Properties, MalformedEscape> {
        let mut properties = Properties::default();
        let mut reader = LineReader { chars: text.chars().collect(), position: 0 };
        while let Some(line) = reader.read_line() {
            let (key, value) = split_logical_line(&line);
            let key = unescape(&key).map_err(|load_convert_line| MalformedEscape { load_convert_line, load0_line: 457 })?;
            let value = unescape(&value).map_err(|load_convert_line| MalformedEscape { load_convert_line, load0_line: 458 })?;
            properties.map.insert(key, value);
        }
        Ok(properties)
    }

    pub fn store(&self, comments: &str, escape_unicode: bool) -> String {
        let mut out = String::new();
        out.push('#');
        out.push_str(comments);
        out.push_str(LINE_SEPARATOR);
        out.push('#');
        out.push_str(&java_date_string());
        out.push_str(LINE_SEPARATOR);
        let mut keys: Vec<&String> = self.map.keys().collect();
        keys.sort_by(|a, b| utf16_units(a).cmp(utf16_units(b)));
        for key in keys {
            out.push_str(&save_convert(key, true, escape_unicode));
            out.push('=');
            out.push_str(&save_convert(&self.map[key], false, escape_unicode));
            out.push_str(LINE_SEPARATOR);
        }
        out
    }
}

struct LineReader {
    chars: Vec<char>,
    position: usize,
}

impl LineReader {
    fn read_line(&mut self) -> Option<String> {
        let mut line = String::new();
        let mut skip_white_space = true;
        let mut is_comment_line = false;
        let mut is_new_line = true;
        let mut appended_line_begin = false;
        let mut preceding_backslash = false;
        let mut skip_lf = false;
        while let Some(&c) = self.chars.get(self.position) {
            self.position += 1;
            if skip_lf {
                skip_lf = false;
                if c == '\n' {
                    continue;
                }
            }
            if skip_white_space {
                if matches!(c, ' ' | '\t' | '\x0c') || (!appended_line_begin && (c == '\r' || c == '\n')) {
                    continue;
                }
                skip_white_space = false;
                appended_line_begin = false;
            }
            if is_new_line {
                is_new_line = false;
                if c == '#' || c == '!' {
                    is_comment_line = true;
                    continue;
                }
            }
            if c != '\n' && c != '\r' {
                if !is_comment_line {
                    line.push(c);
                }
                preceding_backslash = c == '\\' && !preceding_backslash;
                continue;
            }
            if is_comment_line || line.is_empty() {
                is_comment_line = false;
                is_new_line = true;
                skip_white_space = true;
                preceding_backslash = false;
                line.clear();
                skip_lf = c == '\r';
                continue;
            }
            if preceding_backslash {
                line.pop();
                skip_white_space = true;
                appended_line_begin = true;
                preceding_backslash = false;
                skip_lf = c == '\r';
                continue;
            }
            if c == '\r' && self.chars.get(self.position) == Some(&'\n') {
                self.position += 1;
            }
            return Some(line);
        }
        if line.is_empty() || is_comment_line {
            return None;
        }
        if preceding_backslash {
            line.pop();
        }
        Some(line)
    }
}

fn split_logical_line(line: &str) -> (String, String) {
    let chars: Vec<char> = line.chars().collect();
    let mut key_len = 0;
    let mut value_start = chars.len();
    let mut has_sep = false;
    let mut preceding_backslash = false;
    while key_len < chars.len() {
        let c = chars[key_len];
        if (c == '=' || c == ':') && !preceding_backslash {
            value_start = key_len + 1;
            has_sep = true;
            break;
        } else if matches!(c, ' ' | '\t' | '\x0c') && !preceding_backslash {
            value_start = key_len + 1;
            break;
        }
        preceding_backslash = c == '\\' && !preceding_backslash;
        key_len += 1;
    }
    while value_start < chars.len() {
        let c = chars[value_start];
        if !matches!(c, ' ' | '\t' | '\x0c') {
            if !has_sep && (c == '=' || c == ':') {
                has_sep = true;
            } else {
                break;
            }
        }
        value_start += 1;
    }
    (chars[..key_len].iter().collect(), chars[value_start.min(chars.len())..].iter().collect())
}

pub struct MalformedEscape {
    load_convert_line: u32,
    load0_line: u32,
}

impl MalformedEscape {
    pub fn print_stack_trace(&self, load_from_file_line: u32) -> String {
        format!(
            "java.lang.IllegalArgumentException: Malformed \\uxxxx encoding.\n\
             \tat java.base/java.util.Properties.loadConvert(Properties.java:{})\n\
             \tat java.base/java.util.Properties.load0(Properties.java:{})\n\
             \tat java.base/java.util.Properties.load(Properties.java:384)\n\
             \tat net.minecraft.server.dedicated.Settings.loadFromFile(Settings.java:{load_from_file_line})\n\
             \tat net.minecraft.server.dedicated.DedicatedServerProperties.fromFile(DedicatedServerProperties.java:156)\n\
             \tat net.minecraft.server.dedicated.DedicatedServerSettings.<init>(DedicatedServerSettings.java:12)\n\
             \tat net.minecraft.server.Main.main(Main.java:117)\n\
             \tat {}\n\
             \tat {}",
            self.load_convert_line,
            self.load0_line,
            crate::server::main::BUNDLER_MAIN_TAIL[0],
            crate::server::main::BUNDLER_MAIN_TAIL[1],
        )
    }
}

fn unescape(input: &str) -> Result<String, u32> {
    let units: Vec<u16> = utf16_units(input).collect();
    let mut out: Vec<u16> = Vec::with_capacity(units.len());
    let mut i = 0;
    while i < units.len() {
        let c = units[i];
        i += 1;
        if c != b'\\' as u16 {
            out.push(c);
            continue;
        }
        let Some(&next) = units.get(i) else { break };
        i += 1;
        match next {
            0x75 => {
                if i + 4 > units.len() {
                    return Err(655);
                }
                let mut value = 0u16;
                for &digit in &units[i..i + 4] {
                    let digit = char::from_u32(digit as u32).and_then(|c| c.to_digit(16)).filter(|_| digit < 0x80).ok_or(664u32)?;
                    value = (value << 4) + digit as u16;
                }
                i += 4;
                out.push(value);
            }
            0x74 => out.push(b'\t' as u16),
            0x72 => out.push(b'\r' as u16),
            0x6E => out.push(b'\n' as u16),
            0x66 => out.push(0x0c),
            other => out.push(other),
        }
    }
    Ok(from_utf16(&out))
}

fn save_convert(input: &str, escape_space: bool, escape_unicode: bool) -> String {
    let mut out = String::with_capacity(input.len() * 2);
    for (x, c) in input.chars().enumerate() {
        if c as u32 > 61 && (c as u32) < 127 {
            if c == '\\' {
                out.push_str("\\\\");
            } else {
                out.push(c);
            }
            continue;
        }
        match c {
            ' ' => {
                if x == 0 || escape_space {
                    out.push('\\');
                }
                out.push(' ');
            }
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\x0c' => out.push_str("\\f"),
            '=' | ':' | '#' | '!' => {
                out.push('\\');
                out.push(c);
            }
            _ if escape_unicode && ((c as u32) < 0x20 || (c as u32) > 0x7e) => {
                for unit in utf16_units(c.encode_utf8(&mut [0; 4])) {
                    out.push_str(&format!("\\u{unit:04X}"));
                }
            }
            _ => out.push(c),
        }
    }
    out
}

pub fn java_date_string() -> String {
    let now = Local::now();
    format!("{} {} {}", now.format("%a %b %d %H:%M:%S"), java_time_zone_abbreviation(&now), now.format("%Y"))
}

fn java_time_zone_abbreviation(now: &chrono::DateTime<Local>) -> String {
    let named = iana_time_zone::get_timezone().ok().and_then(|name| name.parse::<chrono_tz::Tz>().ok()).map(|tz| now.with_timezone(&tz).format("%Z").to_string());
    match named {
        Some(abbreviation) if !abbreviation.starts_with(['+', '-']) => abbreviation,
        _ => {
            let seconds = now.offset().fix().local_minus_utc();
            if seconds == 0 {
                "GMT".into()
            } else {
                let sign = if seconds < 0 { '-' } else { '+' };
                let minutes = seconds.unsigned_abs() / 60;
                format!("GMT{sign}{:02}:{:02}", minutes / 60, minutes % 60)
            }
        }
    }
}

pub fn java_io_exception(error: &io::Error, path: &Path) -> String {
    let path = path.display();
    match error.kind() {
        io::ErrorKind::NotFound => format!("java.nio.file.NoSuchFileException: {path}"),
        io::ErrorKind::PermissionDenied => format!("java.nio.file.AccessDeniedException: {path}"),
        _ => format!("java.io.IOException: {error}"),
    }
}

pub fn load_from_file(path: &Path) -> Result<Properties, String> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) => {
            error!("Failed to load properties from file: {}\n{}", path.display(), java_io_exception(&e, path));
            return Ok(Properties::default());
        }
    };
    match std::str::from_utf8(&bytes) {
        Ok(text) => Properties::load(&wafer_core::java_string::from_code_points(text)).map_err(|e| e.print_stack_trace(68)),
        Err(_) => {
            info!("Failed to load properties as UTF-8 from file {}, trying ISO_8859_1", path.display());
            Properties::load(&bytes.iter().map(|&b| b as char).collect::<String>()).map_err(|e| e.print_stack_trace(75))
        }
    }
}

pub fn store_to_file(properties: &Properties, path: &Path) {
    let content = properties.store("Minecraft server properties", false);
    let (bytes, malformed) = buffered_writer_bytes(&content);
    if std::fs::write(path, bytes).is_err() || malformed {
        error!("Failed to store properties to file: {}", path.display());
    }
}

fn buffered_writer_bytes(content: &str) -> (Vec<u8>, bool) {
    if !wafer_core::java_string::contains_lone_surrogate(content) {
        return (content.as_bytes().to_vec(), false);
    }
    let mut writer = crate::util::buffered_writer::BufferedWriter::default();
    let stored = writer.write(content).and_then(|()| writer.flush());
    let closed = writer.close();
    (writer.into_bytes(), stored.is_err() || closed.is_err())
}

pub struct Settings {
    pub properties: Properties,
}

impl Settings {
    fn raw(&self, key: &str) -> Option<&str> {
        self.properties.get(key)
    }

    pub fn get<V>(&mut self, key: &str, deserializer: impl Fn(&str) -> Option<V>, serializer: impl Fn(&V) -> String, default: V) -> V {
        let result = self.raw(key).and_then(&deserializer).unwrap_or(default);
        self.properties.put(key, serializer(&result));
        result
    }

    pub fn get_string(&mut self, key: &str, default: &str) -> String {
        self.get(key, |v| Some(v.to_string()), String::clone, default.to_string())
    }

    pub fn get_int(&mut self, key: &str, default: i32) -> i32 {
        self.get(key, parse_int, i32::to_string, default)
    }

    pub fn get_long(&mut self, key: &str, default: i64) -> i64 {
        self.get(key, parse_long, i64::to_string, default)
    }

    pub fn get_bool(&mut self, key: &str, default: bool) -> bool {
        self.get(key, |v| Some(v.eq_ignore_ascii_case("true")), bool::to_string, default)
    }

    pub fn get_legacy(&mut self, key: &str) -> Option<String> {
        self.properties.remove(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wafer_core::java_string::JavaStr;

    #[test]
    fn loads_like_java_properties() {
        let p = Properties::load("# c\n! c\n  key1 = value one  \nkey2:v2\nkey3 v3\nlevel-type=minecraft\\:flat\nlong = a\\\n    b\nunicode=\\u0041\\t\n\nempty=\n").ok().unwrap();
        assert_eq!(p.get("key1"), Some("value one  "));
        assert_eq!(p.get("key2"), Some("v2"));
        assert_eq!(p.get("key3"), Some("v3"));
        assert_eq!(p.get("level-type"), Some("minecraft:flat"));
        assert_eq!(p.get("long"), Some("ab"));
        assert_eq!(p.get("unicode"), Some("A\t"));
        assert_eq!(p.get("empty"), Some(""));
        assert_eq!(p.map.len(), 7);
    }

    #[test]
    fn stores_like_java_properties() {
        let mut p = Properties::default();
        p.put("level-type", "minecraft:normal".into());
        p.put("motd", " A Minecraft Server".into());
        p.put("a key", "x=y#z".into());
        let stored = p.store("Minecraft server properties", false);
        let lines: Vec<&str> = stored.split(LINE_SEPARATOR).collect();
        assert_eq!(lines[0], "#Minecraft server properties");
        assert_eq!(&lines[2..5], ["a\\ key=x\\=y\\#z", "level-type=minecraft\\:normal", "motd=\\ A Minecraft Server"]);
    }

    #[test]
    fn surrogate_escapes_load_as_utf16_units() {
        let p = Properties::load("motd=M\\uDBF6\\uDC00N\\uD800O\nemoji=\\uD83D\\uDE00").ok().unwrap();
        let malformed = |text: &str| Properties::load(text).err().map(|e| (e.load_convert_line, e.load0_line));
        assert_eq!(malformed("mo\\u12=x"), Some((655, 457)));
        assert_eq!(malformed("motd=a\\u12G4b"), Some((664, 458)));
        assert_eq!(malformed("motd=a\\u12\u{1F600}"), Some((664, 458)));        assert_eq!(p.get("motd").unwrap().java_utf16().collect::<Vec<_>>(), [0x4D, 0xDBF6, 0xDC00, 0x4E, 0xD800, 0x4F]);
        assert_eq!(p.get("emoji"), Some("\u{1F600}"));
    }

    #[test]
    fn a_lone_surrogate_fails_the_store_after_writing_the_buffered_prefix_twice() {
        let content = from_utf16(&[0x61, 0x0A, 0x4D, 0xDBF6, 0xDC00, 0x4E, 0xD800, 0x4F, 0x0A]);
        let (bytes, malformed) = buffered_writer_bytes(&content);
        assert!(malformed);
        assert_eq!(bytes, b"a\nM\xF4\x8D\xA0\x80Na\nM\xF4\x8D\xA0\x80N");
        let genuine = from_utf16(&[0x4D, 0xDBF6, 0xDC00, 0x0A]);
        assert_eq!(buffered_writer_bytes(&genuine), (b"M\xF4\x8D\xA0\x80\n".to_vec(), false));
    }
}
