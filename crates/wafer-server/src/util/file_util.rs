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

fn substring(s: &str, end: usize) -> String {
    wafer_core::java_string::from_utf16(&s.java_utf16().take(end).collect::<Vec<_>>())
}

pub fn find_available_name(base_dir: &Path, base_name: &str, suffix: &str) -> std::io::Result<String> {
    let mut base_name = sanitize_name(base_name);
    if !is_path_part_portable(&base_name) {
        base_name = format!("_{base_name}_");
    }
    let mut count = 0;
    if base_name.java_utf16().count() > MAX_FILE_NAME - suffix.java_utf16().count() {
        base_name = substring(&base_name, MAX_FILE_NAME - suffix.java_utf16().count());
    }
    loop {
        let mut name_to_test = base_name.clone();
        if count != 0 {
            let count_suffix = format!(" ({count})");
            let length = MAX_FILE_NAME - count_suffix.len();
            if name_to_test.java_utf16().count() > length {
                name_to_test = substring(&name_to_test, length);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_names_are_cut_in_utf16_units() {
        let name = "\u{1F600}".repeat(200);
        assert_eq!(substring(&name, 255).java_utf16().collect::<Vec<_>>(), name.java_utf16().take(255).collect::<Vec<_>>());
    }

    #[test]
    fn copy_counters_are_never_stripped() {
        let dir = std::env::temp_dir().join(format!("wafer-find-available-name-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(find_available_name(&dir, "<name>world (<count>3)", ".zip").unwrap(), "_name_world (_count_3).zip");
        assert_eq!(find_available_name(&dir, "world (3)", ".zip").unwrap(), "world (3).zip");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
