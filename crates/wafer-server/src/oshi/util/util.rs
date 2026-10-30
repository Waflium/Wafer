use regex_lite::Regex;

pub fn wildcard_match(text: &str, pattern: &str) -> bool {
    match pattern.strip_prefix('^') {
        Some(rest) => !wildcard_match(text, rest),
        None => {
            let expression = format!("^(?:{})$", pattern.replace('?', ".?").replace('*', ".*?"));
            Regex::new(&expression).is_ok_and(|regex| regex.is_match(text))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wildcards_follow_oshi() {
        assert!(wildcard_match("java#1", "java*"));
        assert!(!wildcard_match("_total", "^*_total"));
        assert!(wildcard_match("javaw", "^*_total"));
        assert!(wildcard_match("idle", "_total|idle"));
        assert!(!wildcard_match("system", "_total|idle"));
    }
}
