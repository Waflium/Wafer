use regex_lite::{Captures, Regex};

use crate::util::filefix::file_fix_exception::FileFixFailure;
use crate::util::filefix::file_fix_util;
use crate::util::filefix::virtualfilesystem::copy_on_write_fs_path::CopyOnWriteFSPath;
use crate::util::filefix::virtualfilesystem::files;
use crate::util::worldupdate::upgrade_progress::UpgradeProgress;

use super::file_fix_operation::FileFixOperation;

pub struct RegexMove {
    matches_pattern: Regex,
    from_pattern: Regex,
    to_replacement: String,
}

impl RegexMove {
    pub fn new(from_pattern: &str, to_pattern: &str) -> RegexMove {
        RegexMove {
            matches_pattern: Regex::new(&format!("^(?:{from_pattern})$")).unwrap_or_else(|e| panic!("java.util.regex.PatternSyntaxException: {e}")),
            from_pattern: Regex::new(from_pattern).unwrap_or_else(|e| panic!("java.util.regex.PatternSyntaxException: {e}")),
            to_replacement: to_pattern.to_string(),
        }
    }

    pub fn from_pattern(&self) -> &Regex {
        &self.from_pattern
    }

    pub fn to_replacement(&self) -> &str {
        &self.to_replacement
    }
}

fn expand_replacement(replacement: &str, captures: &Captures) -> String {
    let chars: Vec<char> = replacement.chars().collect();
    let group_count = captures.len() - 1;
    let mut result = String::new();
    let mut cursor = 0;
    while cursor < chars.len() {
        let next_char = chars[cursor];
        cursor += 1;
        if next_char == '\\' {
            if cursor == chars.len() {
                panic!("java.lang.IllegalArgumentException: character to be escaped is missing");
            }
            result.push(chars[cursor]);
            cursor += 1;
        } else if next_char == '$' {
            if cursor == chars.len() {
                panic!("java.lang.IllegalArgumentException: Illegal group reference: group index is missing");
            }
            let Some(first_digit) = chars[cursor].to_digit(10).filter(|_| chars[cursor].is_ascii_digit()) else {
                panic!("java.lang.IllegalArgumentException: Illegal group reference");
            };
            cursor += 1;
            let mut ref_num = first_digit as usize;
            if ref_num > group_count {
                panic!("java.lang.IndexOutOfBoundsException: No group {ref_num}");
            }
            while cursor < chars.len() {
                let Some(next_digit) = chars[cursor].to_digit(10).filter(|_| chars[cursor].is_ascii_digit()) else { break };
                let new_ref_num = ref_num * 10 + next_digit as usize;
                if group_count < new_ref_num {
                    break;
                }
                ref_num = new_ref_num;
                cursor += 1;
            }
            if let Some(group) = captures.get(ref_num) {
                result.push_str(group.as_str());
            }
        } else {
            result.push(next_char);
        }
    }
    result
}

impl FileFixOperation for RegexMove {
    fn fix(&self, base_directory: &CopyOnWriteFSPath, _upgrade_progress: &UpgradeProgress) -> Result<(), FileFixFailure> {
        if files::exists(base_directory) && files::is_directory(base_directory) {
            for file in files::list(base_directory)? {
                let file_name = file.get_file_name().map(|name| name.to_string()).unwrap_or_default();
                if self.matches_pattern.is_match(&file_name) {
                    let new_name = self.from_pattern.replace_all(&file_name, |captures: &Captures| expand_replacement(&self.to_replacement, captures)).into_owned();
                    file_fix_util::move_file(base_directory, &file_name, &new_name)?;
                }
            }
        }
        Ok(())
    }
}
