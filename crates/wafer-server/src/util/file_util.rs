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

pub fn create_directories_safe(dir: &Path) -> std::io::Result<()> {
    match dir.exists() {
        true => std::fs::create_dir_all(dir.canonicalize()?),
        false => std::fs::create_dir_all(dir),
    }
}
