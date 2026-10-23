use std::cell::RefCell;

use wafer_protocol::nbt::Nbt;

use crate::util::codec::{self, DecodeResult};
use crate::util::problem_reporter::{PathElement, Problem, ProblemReporter};

thread_local! {
    static PROBLEM_REPORTER: RefCell<Option<ProblemReporter>> = const { RefCell::new(None) };
}

pub struct ScopedCollector {
    collector: ProblemReporter,
    previous: Option<ProblemReporter>,
    logger: &'static str,
    late_root: Option<PathElement>,
    log_thread: Option<&'static str>,
}

impl ScopedCollector {
    pub fn new(root: PathElement, logger: &'static str) -> ScopedCollector {
        let collector = ProblemReporter::collector(root);
        let previous = PROBLEM_REPORTER.with(|current| current.replace(Some(collector.clone())));
        ScopedCollector { collector, previous, logger, late_root: None, log_thread: None }
    }

    pub fn logging_as(mut self, thread: &'static str) -> ScopedCollector {
        self.log_thread = Some(thread);
        self
    }

    pub fn new_empty(logger: &'static str) -> ScopedCollector {
        ScopedCollector::new(PathElement::Empty, logger)
    }

    pub fn set_root(&mut self, root: PathElement) {
        self.late_root = Some(root);
    }
}

impl Drop for ScopedCollector {
    fn drop(&mut self) {
        PROBLEM_REPORTER.with(|current| *current.borrow_mut() = self.previous.take());
        if !self.collector.is_empty() {
            let report = match self.late_root.take() {
                Some(root) => self.collector.get_tree_report_rooted(root),
                None => self.collector.get_tree_report(),
            };
            match self.log_thread {
                Some(thread) => warn!(thread: thread, "[{}] Serialization errors:\n{}", self.logger, report),
                None => warn!("[{}] Serialization errors:\n{}", self.logger, report),
            }
        }
    }
}

pub struct ChildScope {
    previous: Option<ProblemReporter>,
}

impl Drop for ChildScope {
    fn drop(&mut self) {
        PROBLEM_REPORTER.with(|current| *current.borrow_mut() = self.previous.take());
    }
}

pub fn enter_child(element: PathElement) -> ChildScope {
    let previous = PROBLEM_REPORTER.with(|current| {
        let mut current = current.borrow_mut();
        let child = current.as_ref().map(|reporter| reporter.for_child(element));
        std::mem::replace(&mut *current, child)
    });
    ChildScope { previous }
}

pub fn enter_indexed_field(name: &str, index: usize) -> ChildScope {
    enter_child(PathElement::IndexedField(name.to_string(), index))
}

pub fn report(problem: impl FnOnce() -> Problem) {
    PROBLEM_REPORTER.with(|current| {
        if let Some(reporter) = current.borrow().as_ref() {
            reporter.report(problem());
        }
    });
}

pub fn type_name(tag: &Nbt) -> &'static str {
    match tag {
        Nbt::Byte(_) => "BYTE",
        Nbt::Short(_) => "SHORT",
        Nbt::Int(_) => "INT",
        Nbt::Long(_) => "LONG",
        Nbt::Float(_) => "FLOAT",
        Nbt::Double(_) => "DOUBLE",
        Nbt::ByteArray(_) => "BYTE[]",
        Nbt::String(_) => "STRING",
        Nbt::List(_) => "LIST",
        Nbt::Compound(_) => "COMPOUND",
        Nbt::IntArray(_) => "INT[]",
        Nbt::LongArray(_) => "LONG[]",
    }
}

pub fn report_decode_from_field_failed(name: &str, tag: &Nbt, message: &str) {
    report(|| Problem::new(format!("Failed to decode value '{}' from field '{name}': {message}", crate::nbt::to_string(tag))));
}

pub fn report_decode_from_list_failed(name: &str, index: usize, tag: &Nbt, message: &str) {
    report(|| Problem::new(format!("Failed to decode value '{}' from field '{name}' at index {index}': {message}", crate::nbt::to_string(tag))));
}

pub fn read<T>(input: &Nbt, name: &str, decode: impl FnOnce(&Nbt) -> DecodeResult<T>) -> Option<T> {
    let tag = input.get(name)?;
    let result = codec::with_nbt_ops(|| decode(tag));
    if let Some(message) = &result.error {
        report_decode_from_field_failed(name, tag, message);
    }
    result.value
}

pub fn read_or_error<T>(input: &Nbt, name: &str, decode: impl FnOnce(&Nbt) -> Result<T, String>) -> Option<T> {
    let tag = input.get(name)?;
    match codec::with_nbt_ops(|| decode(tag)) {
        Ok(value) => Some(value),
        Err(message) => {
            report_decode_from_field_failed(name, tag, &message);
            None
        }
    }
}

fn get_optional_typed_tag<'a>(input: &'a Nbt, name: &str, expected: &'static str, matches: impl FnOnce(&Nbt) -> bool) -> Option<&'a Nbt> {
    let tag = input.get(name)?;
    if matches(tag) {
        Some(tag)
    } else {
        report(|| Problem::new(format!("Expected field '{name}' to contain value of type {expected}, but got {}", type_name(tag))));
        None
    }
}

pub fn get_numeric<'a>(input: &'a Nbt, name: &str) -> Option<&'a Nbt> {
    let tag = input.get(name)?;
    if matches!(tag, Nbt::Byte(_) | Nbt::Short(_) | Nbt::Int(_) | Nbt::Long(_) | Nbt::Float(_) | Nbt::Double(_)) {
        Some(tag)
    } else {
        report(|| Problem::new(format!("Expected field '{name}' to contain number, but got {}", type_name(tag))));
        None
    }
}

pub fn get_f64(input: &Nbt, name: &str) -> Option<f64> {
    get_numeric(input, name).and_then(Nbt::as_f64)
}

pub fn get_i64(input: &Nbt, name: &str) -> Option<i64> {
    get_numeric(input, name).and_then(Nbt::as_i64)
}

pub fn get_boolean(input: &Nbt, name: &str) -> Option<bool> {
    get_numeric(input, name).map(|tag| match *tag {
        Nbt::Byte(value) => value != 0,
        Nbt::Short(value) => value as i8 != 0,
        Nbt::Int(value) => value as i8 != 0,
        Nbt::Long(value) => value as i8 != 0,
        Nbt::Float(value) => wafer_core::mth::floor_f(value) as i8 != 0,
        Nbt::Double(value) => wafer_core::mth::floor(value) as i8 != 0,
        _ => false,
    })
}

pub fn get_string<'a>(input: &'a Nbt, name: &str) -> Option<&'a str> {
    get_optional_typed_tag(input, name, "STRING", |tag| matches!(tag, Nbt::String(_))).and_then(Nbt::as_str)
}

pub fn get_int_array<'a>(input: &'a Nbt, name: &str) -> Option<&'a [i32]> {
    match get_optional_typed_tag(input, name, "INT[]", |tag| matches!(tag, Nbt::IntArray(_)))? {
        Nbt::IntArray(values) => Some(values),
        _ => None,
    }
}

pub fn children_list<'a>(input: &'a Nbt, name: &str) -> Option<&'a [Nbt]> {
    match get_optional_typed_tag(input, name, "LIST", |tag| matches!(tag, Nbt::List(_)))? {
        Nbt::List(list) => Some(list),
        _ => None,
    }
}

pub fn for_each_child(input: &Nbt, name: &str, mut action: impl FnMut(&Nbt)) {
    let Some(list) = children_list(input, name) else { return };
    for (index, value) in list.iter().enumerate() {
        match value {
            Nbt::Compound(fields) if fields.is_empty() => action(value),
            Nbt::Compound(_) => {
                let _scope = enter_indexed_field(name, index);
                action(value);
            }
            _ => report(|| {
                Problem::new(format!("Expected list '{name}' to contain at index {index} value of type COMPOUND, but got {}", type_name(value)))
            }),
        }
    }
}

pub fn list_or_empty<T>(input: &Nbt, name: &str, mut decode: impl FnMut(&Nbt) -> DecodeResult<T>) -> Vec<T> {
    let Some(list) = children_list(input, name) else { return Vec::new() };
    let mut values = Vec::with_capacity(list.len());
    for (index, tag) in list.iter().enumerate() {
        let result = codec::with_nbt_ops(|| decode(tag));
        if let Some(message) = &result.error {
            report_decode_from_list_failed(name, index, tag, message);
        }
        values.extend(result.value);
    }
    values
}

pub fn read_byte(input: &Nbt, name: &str) -> Option<i8> {
    read_or_error(input, name, |tag| codec::number(tag).map(|number| crate::world::item::component::codec::as_i64(number) as i8))
}

#[cfg(test)]
mod tests;
