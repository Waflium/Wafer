use super::{parse_lenient, parse_strict, to_stable_string};
use crate::util::datafix::legacy_component_data_fix_utils::{extract_translation_string, rewrite_from_lenient};

fn hex(value: &str) -> String {
    value.encode_utf16().map(|unit| format!("{unit:04x}")).collect()
}

fn unhex(value: &str) -> Vec<u16> {
    (0..value.len()).step_by(4).map(|i| u16::from_str_radix(&value[i..i + 4], 16).unwrap()).collect()
}

fn has_lone_surrogate(value: &str) -> bool {
    char::decode_utf16(unhex(value)).any(|c| c.is_err())
}

fn check(expected: &str) -> usize {
    let mut failures = 0;
    for line in expected.lines() {
        let fields: Vec<&str> = line.split('\t').collect();
        let flagged = [fields[1], fields[2], fields[4]].map(|field| &field[1..]);
        if flagged.iter().chain([&fields[3]]).any(|field| has_lone_surrogate(field)) {
            continue;
        }
        let input = String::from_utf16(&unhex(fields[0])).unwrap();
        let lenient = parse_lenient(&input).map(|element| format!("1{}", hex(&to_stable_string(&element)))).unwrap_or_else(|_| "0".to_string());
        let strict = parse_strict(&input).map(|element| format!("1{}", hex(&to_stable_string(&element)))).unwrap_or_else(|_| "0".to_string());
        let rewritten = hex(&rewrite_from_lenient(&input));
        let translation = extract_translation_string(&input).map(|key| format!("1{}", hex(&key))).unwrap_or_else(|| "0".to_string());
        let actual = [fields[0].to_string(), lenient, strict, rewritten, translation].join("\t");
        if actual != line {
            failures += 1;
            if failures <= 20 {
                eprintln!("input {input:?}\n  expected {line}\n  actual   {actual}");
            }
        }
    }
    failures
}

#[test]
fn gson_matches_the_vanilla_cases() {
    assert_eq!(check(include_str!("oracle_cases.tsv")), 0);
}

#[test]
#[ignore]
fn gson_matches_the_vanilla_oracle() {
    let path = std::env::var("GSON_ORACLE").expect("GSON_ORACLE");
    assert_eq!(check(&std::fs::read_to_string(path).unwrap()), 0);
}

fn result(value: Result<String, String>) -> String {
    match value {
        Ok(text) => format!("1{}", hex(&text)),
        Err(text) => format!("0{}", hex(text.split("\nCaused by: ").next().unwrap())),
    }
}

fn check_errors(expected: &str) -> usize {
    let mut failures = 0;
    for line in expected.lines() {
        let fields: Vec<&str> = line.split('\t').collect();
        if fields[1..].iter().any(|field| has_lone_surrogate(&field[1..])) {
            continue;
        }
        let input = String::from_utf16(&unhex(fields[0])).unwrap();
        let strict = result(parse_strict(&input).map(|element| to_stable_string(&element)));
        let lenient = result(parse_lenient(&input).map(|element| to_stable_string(&element)));
        let object = result(super::parse_object(&input).map(|element| super::to_json_string(&element)));
        let actual = [fields[0].to_string(), strict, lenient, object].join("\t");
        if actual != line {
            failures += 1;
            if failures <= 10 {
                let decode = |field: &str| String::from_utf16_lossy(&unhex(&field[1..]));
                let actual_fields: Vec<String> = actual.split('\t').map(str::to_string).collect();
                for i in 1..4 {
                    if actual_fields[i] != fields[i] {
                        eprintln!("input {input:?} field {i}\n  expected {}\n  actual   {}", decode(fields[i]), decode(&actual_fields[i]));
                    }
                }
            }
        }
    }
    failures
}

#[test]
#[ignore]
fn gson_errors_match_the_vanilla_oracle() {
    let path = std::env::var("GSON_ERROR_ORACLE").expect("GSON_ERROR_ORACLE");
    assert_eq!(check_errors(&std::fs::read_to_string(path).unwrap()), 0);
}

#[test]
fn gson_errors_match_the_vanilla_cases() {
    assert_eq!(check_errors(include_str!("error_cases.tsv")), 0);
}
