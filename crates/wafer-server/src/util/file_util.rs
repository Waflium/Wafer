use wafer_core::java_string::JavaStr;
use std::path::Path;

fn contains_allowed_characters_only(segment: &str) -> bool {
    !segment.is_empty() && segment.bytes().all(|c| matches!(c, b'-' | b'.' | b'_' | b'a'..=b'z' | b'0'..=b'9'))
}

pub fn is_valid_path_segment(segment: &str) -> bool {
    segment != ".." && segment != "." && contains_allowed_characters_only(segment)
}

pub fn is_path_part_portable(name: &str) -> bool {
    if name.ends_with('.') {
        return false;
    }
    let base = name.split('.').next().unwrap_or_default().to_ascii_uppercase();
    let numbered = |prefix: &str| base.len() == 4 && base.starts_with(prefix) && matches!(base.as_bytes()[3], b'1'..=b'9');
    !(matches!(base.as_str(), "COM" | "CLOCK$" | "CON" | "PRN" | "AUX" | "NUL") || numbered("COM") || numbered("LPT"))
}

pub fn decompose_path(path: &str) -> Option<Vec<&str>> {
    let segments: Vec<&str> = path.split('/').collect();
    segments.iter().all(|segment| is_valid_path_segment(segment)).then_some(segments)
}

const MAX_FILE_NAME: usize = 255;
const ILLEGAL_FILE_CHARACTERS: [char; 15] = ['/', '\n', '\r', '\t', '\0', '\u{c}', '`', '?', '*', '\\', '<', '>', '|', '"', ':'];

pub fn sanitize_name(base_name: &str) -> String {
    let mut base_name = base_name.to_string();
    for replacer in ILLEGAL_FILE_CHARACTERS {
        base_name = base_name.replace(replacer, "_");
    }
    base_name.replace(['.', '/', '"'], "_")
}

pub fn find_available_name(base_dir: &Path, base_name: &str, suffix: &str) -> std::io::Result<String> {
    let mut base_name = sanitize_name(base_name);
    if !is_path_part_portable(&base_name) {
        base_name = format!("_{base_name}_");
    }
    let mut count = 0;
    if base_name.java_utf16().count() > MAX_FILE_NAME - suffix.java_utf16().count() {
        base_name = base_name.chars().take(MAX_FILE_NAME - suffix.java_utf16().count()).collect();
    }
    loop {
        let mut name_to_test = base_name.clone();
        if count != 0 {
            let count_suffix = format!(" ({count})");
            let length = MAX_FILE_NAME - count_suffix.len();
            if name_to_test.java_utf16().count() > length {
                name_to_test = name_to_test.chars().take(length).collect();
            }
            name_to_test.push_str(&count_suffix);
        }
        name_to_test.push_str(suffix);
        let full_path = base_dir.join(&name_to_test);
        match std::fs::create_dir(&full_path) {
            Ok(()) => {
                crate::util::java_nio::delete_if_exists(&full_path)?;
                return Ok(name_to_test);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => count += 1,
            Err(e) => return Err(crate::util::java_nio::for_path(e, &full_path)),
        }
    }
}

pub fn create_directories_safe(dir: &Path) -> std::io::Result<()> {
    match dir.exists() {
        true => std::fs::create_dir_all(dir.canonicalize()?),
        false => std::fs::create_dir_all(dir),
    }
}
