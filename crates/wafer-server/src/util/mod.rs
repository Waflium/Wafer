pub mod argb;
pub mod basic_file_attributes;
pub mod block_util;
pub mod brightness;
pub mod codec;
pub mod datafix;
pub mod compilable_string;
pub mod dependency_sorter;
pub mod ease;
pub mod easing_type;
pub mod gson;
pub mod either;
pub mod extra_codecs;
pub mod directory_lock;
pub mod file_util;
pub mod filefix;
pub mod fraction;
pub mod hash_ops;
pub mod inclusive_range;
pub mod java_collections;
pub mod java_gzip;
pub mod java_heap;
pub mod java_inflater;
pub mod java_integer;
pub mod java_regex;
pub mod java_simple_date_format;
pub mod java_nio;
pub mod java_throwable;
pub mod java_uuid;
pub mod keyframe_track;
pub mod lifecycle;
pub mod parsing;
pub mod png_info;
pub mod random;
pub mod range_codecs;
pub mod signature_validator;
pub mod spawn_util;
pub mod special_dates;
pub mod problem_reporter;
pub mod string_util;
pub mod tick_throttler;
pub mod top_k_selector;
pub mod time_util;
pub mod tri_state;
pub mod uri;
pub mod valueproviders;
pub mod worldupdate;

use wafer_core::random::RandomSource;

pub type FastIndexMap<K, V> = indexmap::IndexMap<K, V, std::hash::BuildHasherDefault<wafer_core::hash_map::FastHasher>>;
pub type FastIndexSet<K> = indexmap::IndexSet<K, std::hash::BuildHasherDefault<wafer_core::hash_map::FastHasher>>;

pub fn parse_and_validate_untrusted_uri(uri: &str) -> Result<uri::URI, uri::URISyntaxException> {
    let parsed = uri::URI::new(uri)?;
    match parsed.scheme() {
        None => Err(uri::URISyntaxException::new(uri, format!("Missing protocol in URI: {uri}"), -1)),
        Some(scheme) if !matches!(scheme.to_ascii_lowercase().as_str(), "http" | "https") => {
            Err(uri::URISyntaxException::new(uri, format!("Unsupported protocol in URI: {uri}"), -1))
        }
        Some(_) => Ok(parsed),
    }
}

pub fn get_filename_formatted_date_time() -> String {
    chrono::Local::now().format("%Y-%m-%d_%H.%M.%S").to_string()
}

thread_local! {
    static CATCHING_EXCEPTION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub fn is_catching_exception() -> bool {
    CATCHING_EXCEPTION.with(std::cell::Cell::get)
}

pub fn catch_exception<T>(function: impl FnOnce() -> T) -> Result<T, String> {
    let outer = CATCHING_EXCEPTION.with(|catching| catching.replace(true));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(function));
    CATCHING_EXCEPTION.with(|catching| catching.set(outer));
    result.map_err(|payload| crate::crash_report::exception_of(&payload))
}

pub fn write_and_read_typed_or_throw(
    typed: &crate::datafixers::typed::Typed,
    new_type: &std::sync::Arc<crate::datafixers::types::Type>,
    function: impl FnOnce(crate::datafixers::dynamic::Dynamic) -> crate::datafixers::dynamic::Dynamic,
) -> crate::datafixers::typed::Typed {
    let dynamic = typed.write().into_result().unwrap_or_else(|message| panic!("{message}"));
    read_typed_or_throw(new_type, function(dynamic), true)
}

pub fn read_typed_or_throw(
    ty: &std::sync::Arc<crate::datafixers::types::Type>,
    dynamic: crate::datafixers::dynamic::Dynamic,
    accept_partial: bool,
) -> crate::datafixers::typed::Typed {
    let result = crate::datafixers::typed::Typed::read_typed(ty, dynamic);
    match (result.value, result.error) {
        (Some((typed, _)), None) => typed,
        (Some((typed, _)), Some(_)) if accept_partial => typed,
        (_, error) => panic!("Reading type: {}", error.unwrap_or_default()),
    }
}

pub fn get_millis() -> i64 {
    static ORIGIN: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    ORIGIN.get_or_init(std::time::Instant::now).elapsed().as_millis() as i64
}

struct Operation<'a> {
    description: String,
    run: Box<dyn Fn() -> bool + 'a>,
}

fn create_renamer<'a>(from: &'a std::path::Path, to: &'a std::path::Path, options: &'a [java_nio::CopyOption]) -> Operation<'a> {
    Operation {
        description: format!("rename {} to {}", from.display(), to.display()),
        run: Box::new(move || match java_nio::move_file(from, to, options) {
            Ok(()) => true,
            Err(e) => {
                error!("Failed to rename\n{e}");
                false
            }
        }),
    }
}

fn create_deleter(target: &std::path::Path) -> Operation<'_> {
    Operation {
        description: format!("delete old {}", target.display()),
        run: Box::new(move || match java_nio::delete_if_exists(target) {
            Ok(_) => true,
            Err(e) => {
                warn!("Failed to delete\n{e}");
                false
            }
        }),
    }
}

fn create_file_deleted_check(target: &std::path::Path) -> Operation<'_> {
    Operation { description: format!("verify that {} is deleted", target.display()), run: Box::new(move || !target.exists()) }
}

fn create_file_created_check(target: &std::path::Path) -> Operation<'_> {
    Operation { description: format!("verify that {} is present", target.display()), run: Box::new(move || java_nio::is_regular_file(target)) }
}

fn execute_in_sequence(operations: &[Operation]) -> bool {
    for operation in operations {
        if !(operation.run)() {
            warn!("Failed to execute {}", operation.description);
            return false;
        }
    }
    true
}

fn run_with_retries(number_of_retries: i32, description: &str, operations: &[Operation]) -> bool {
    for retry in 0..number_of_retries {
        if execute_in_sequence(operations) {
            return true;
        }
        error!("Failed to {description}, retrying {retry}/{number_of_retries}");
    }
    error!("Failed to {description}, aborting, progress might be lost");
    false
}

pub fn safe_move_file(from_path: &std::path::Path, to_path: &std::path::Path, options: &[java_nio::CopyOption]) -> bool {
    run_with_retries(
        10,
        &format!("move from  {} to {}", from_path.display(), to_path.display()),
        &[create_renamer(from_path, to_path, options), create_file_created_check(to_path)],
    )
}

pub fn safe_replace_file(target_path: &std::path::Path, new_path: &std::path::Path, backup_path: &std::path::Path) {
    safe_replace_or_move_file(target_path, new_path, backup_path, false);
}

pub fn safe_replace_or_move_file(target_path: &std::path::Path, new_path: &std::path::Path, backup_path: &std::path::Path, no_rollback: bool) -> bool {
    if target_path.exists()
        && !run_with_retries(
            10,
            &format!("create backup {}", backup_path.display()),
            &[create_deleter(backup_path), create_renamer(target_path, backup_path, &[]), create_file_created_check(backup_path)],
        )
    {
        false
    } else if !run_with_retries(10, &format!("remove old {}", target_path.display()), &[create_deleter(target_path), create_file_deleted_check(target_path)]) {
        false
    } else if !run_with_retries(
        10,
        &format!("replace {} with {}", target_path.display(), new_path.display()),
        &[create_renamer(new_path, target_path, &[]), create_file_created_check(target_path)],
    ) && !no_rollback
    {
        run_with_retries(
            10,
            &format!("restore {} from {}", target_path.display(), backup_path.display()),
            &[create_renamer(backup_path, target_path, &[]), create_file_created_check(target_path)],
        );
        false
    } else {
        true
    }
}

pub fn java_min_f(a: f32, b: f32) -> f32 {
    if a.is_nan() || b.is_nan() {
        f32::NAN
    } else if a == 0.0 && b == 0.0 {
        if a.is_sign_negative() { a } else { b }
    } else if a <= b {
        a
    } else {
        b
    }
}

pub fn java_max_f(a: f32, b: f32) -> f32 {
    if a.is_nan() || b.is_nan() {
        f32::NAN
    } else if a == 0.0 && b == 0.0 {
        if a.is_sign_negative() { b } else { a }
    } else if a >= b {
        a
    } else {
        b
    }
}

pub fn java_round_f(a: f32) -> i32 {
    let int_bits = a.to_bits() as i32;
    let biased_exp = (int_bits & 0x7F80_0000) >> 23;
    let shift = (24 - 2 + 127) - biased_exp;
    if shift & -32 == 0 {
        let mut r = (int_bits & 0x007F_FFFF) | (0x007F_FFFF + 1);
        if int_bits < 0 {
            r = -r;
        }
        ((r >> shift) + 1) >> 1
    } else {
        a as i32
    }
}

pub fn java_format_fixed(value: f64, decimals: i32) -> String {
    if value.is_nan() {
        return "NaN".into();
    }
    if value.is_infinite() {
        return if value > 0.0 { "Infinity" } else { "-Infinity" }.into();
    }
    let negative = value.is_sign_negative();
    let shortest = format!("{:e}", value.abs());
    let (mantissa, exponent) = shortest.split_once('e').expect("exponent form");
    let mut digits: Vec<u8> = mantissa.bytes().filter(|b| b.is_ascii_digit()).map(|b| b - b'0').collect();
    let mut point = exponent.parse::<i32>().unwrap() + 1;
    let keep = point + decimals;
    if keep < 0 {
        digits = vec![0];
        point = 1;
    } else if (keep as usize) < digits.len() {
        let round_up = digits[keep as usize] >= 5;
        digits.truncate(keep as usize);
        if round_up {
            let mut i = digits.len();
            loop {
                if i == 0 {
                    digits.insert(0, 1);
                    point += 1;
                    break;
                }
                i -= 1;
                if digits[i] == 9 {
                    digits[i] = 0;
                } else {
                    digits[i] += 1;
                    break;
                }
            }
        }
    }
    let digit = |i: i32| if i >= 0 && (i as usize) < digits.len() { (b'0' + digits[i as usize]) as char } else { '0' };
    let mut out = String::new();
    if negative {
        out.push('-');
    }
    if point <= 0 {
        out.push('0');
    } else {
        (0..point).for_each(|i| out.push(digit(i)));
    }
    if decimals > 0 {
        out.push('.');
        (point..point + decimals).for_each(|i| out.push(digit(i)));
    }
    out
}

pub fn get_random_safe<T: Copy>(list: &[T], random: &mut (impl RandomSource + ?Sized)) -> Option<T> {
    if list.is_empty() { None } else { Some(list[random.next_int_bounded(list.len() as i32) as usize]) }
}

pub fn shuffle<T>(list: &mut [T], random: &mut impl RandomSource) {
    for i in (2..=list.len()).rev() {
        let swap_to = random.next_int_bounded(i as i32) as usize;
        list.swap(i - 1, swap_to);
    }
}

#[cfg(test)]
mod tests {
    use wafer_core::random::{LegacyRandomSource, RandomSource};

    #[test]
    fn shuffle_matches_vanilla() {
        let mut random = LegacyRandomSource::new(42);
        let mut list: Vec<i32> = (0..10).collect();
        super::shuffle(&mut list, &mut random);
        assert_eq!(list, [4, 6, 2, 1, 7, 9, 8, 5, 3, 0]);
        assert_eq!(random.next_int(), 392236186);
    }

    #[test]
    fn caught_exceptions_return_their_message_and_reset_the_flag() {
        assert!(!super::is_catching_exception());
        assert_eq!(super::catch_exception(|| 5), Ok(5));
        assert_eq!(super::catch_exception(|| -> i32 { panic!("{}", "boom") }), Err("boom".to_string()));
        assert_eq!(super::catch_exception(|| -> i32 { panic!("static") }), Err("static".to_string()));
        assert!(!super::is_catching_exception());
        assert_eq!(super::catch_exception(|| super::catch_exception(|| -> i32 { panic!("inner") }).unwrap_err() + &super::is_catching_exception().to_string()), Ok("innertrue".to_string()));
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Prediction {
    Predicted,
    ServerOnly,
}
