use super::event_type::{ContentType, EventType, FieldType, Value};
use super::thread::ThreadInfo;

pub const HEADER_SIZE: usize = 68;
pub const METADATA_TYPE_ID: i64 = 0;
pub const CHECKPOINT_TYPE_ID: i64 = 1;
pub const FLAG_COMPRESSED_INTS: i32 = 1;
pub const FLAG_CHUNK_FINAL: i32 = 2;

pub const BOOLEAN: i64 = 4;
pub const CHAR: i64 = 5;
pub const FLOAT: i64 = 6;
pub const DOUBLE: i64 = 7;
pub const BYTE: i64 = 8;
pub const SHORT: i64 = 9;
pub const INT: i64 = 10;
pub const LONG: i64 = 11;
pub const STRING: i64 = 20;
pub const THREAD: i64 = 21;
pub const ANNOTATION_LABEL: i64 = 40;
pub const ANNOTATION_DESCRIPTION: i64 = 41;
pub const ANNOTATION_CATEGORY: i64 = 42;
pub const ANNOTATION_TIMESTAMP: i64 = 43;
pub const ANNOTATION_TIMESPAN: i64 = 44;
pub const ANNOTATION_DATA_AMOUNT: i64 = 45;
pub const ANNOTATION_PERCENTAGE: i64 = 46;
pub const ANNOTATION_CONTENT_TYPE: i64 = 47;

pub fn write_long(out: &mut Vec<u8>, value: i64) {
    let mut v = value as u64;
    for _ in 0..8 {
        if v < 0x80 {
            out.push(v as u8);
            return;
        }
        out.push((v as u8 & 0x7F) | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

pub fn write_string(out: &mut Vec<u8>, value: Option<&str>) {
    match value {
        None => out.push(0),
        Some("") => out.push(1),
        Some(text) => {
            out.push(3);
            write_long(out, text.len() as i64);
            out.extend_from_slice(text.as_bytes());
        }
    }
}

fn type_id(field_type: FieldType) -> i64 {
    match field_type {
        FieldType::Boolean => BOOLEAN,
        FieldType::Int => INT,
        FieldType::Long => LONG,
        FieldType::Float => FLOAT,
        FieldType::String => STRING,
        FieldType::Thread => THREAD,
    }
}

fn write_value(out: &mut Vec<u8>, value: &Value) {
    match *value {
        Value::Boolean(v) => out.push(v as u8),
        Value::Int(v) => write_long(out, v as i64),
        Value::Long(v) => write_long(out, v),
        Value::Float(v) => out.extend_from_slice(&v.to_bits().to_be_bytes()),
        Value::String(v) => write_string(out, Some(v)),
        Value::NullString => write_string(out, None),
        Value::Thread(key) => write_long(out, key as i64),
    }
}

fn with_size(body: Vec<u8>) -> Vec<u8> {
    let mut size = body.len() + 1;
    loop {
        let mut prefix = Vec::with_capacity(9);
        write_long(&mut prefix, size as i64);
        if prefix.len() + body.len() == size {
            prefix.extend_from_slice(&body);
            return prefix;
        }
        size = prefix.len() + body.len();
    }
}

pub fn event(event_type: &EventType, start_ticks: i64, end_ticks: i64, thread: u64, values: &[Value]) -> Vec<u8> {
    let mut body = Vec::with_capacity(32);
    write_long(&mut body, event_type.id);
    write_long(&mut body, start_ticks);
    if event_type.has_duration {
        write_long(&mut body, end_ticks - start_ticks);
    }
    if event_type.has_thread {
        write_long(&mut body, thread as i64);
    }
    for value in values {
        write_value(&mut body, value);
    }
    with_size(body)
}

pub fn checkpoint(ticks: i64, threads: &[ThreadInfo]) -> Vec<u8> {
    let mut body = Vec::new();
    write_long(&mut body, CHECKPOINT_TYPE_ID);
    write_long(&mut body, ticks);
    write_long(&mut body, 0);
    write_long(&mut body, 0);
    body.push(1);
    write_long(&mut body, 1);
    write_long(&mut body, THREAD);
    write_long(&mut body, threads.len() as i64);
    for thread in threads {
        write_long(&mut body, thread.key as i64);
        write_string(&mut body, Some(&thread.os_name));
        write_long(&mut body, thread.os_thread_id);
        write_string(&mut body, Some(&thread.java_name));
        write_long(&mut body, thread.java_thread_id);
    }
    with_size(body)
}

struct Element {
    name: &'static str,
    attributes: Vec<(&'static str, String)>,
    children: Vec<Element>,
}

impl Element {
    fn new(name: &'static str) -> Element {
        Element { name, attributes: Vec::new(), children: Vec::new() }
    }

    fn attribute(mut self, key: &'static str, value: impl Into<String>) -> Element {
        self.attributes.push((key, value.into()));
        self
    }

    fn child(mut self, child: Element) -> Element {
        self.children.push(child);
        self
    }

    fn collect_strings(&self, strings: &mut Vec<String>) {
        intern(strings, self.name);
        for (key, value) in &self.attributes {
            intern(strings, key);
            intern(strings, value);
        }
        for child in &self.children {
            child.collect_strings(strings);
        }
    }

    fn write(&self, out: &mut Vec<u8>, strings: &[String]) {
        write_long(out, index(strings, self.name));
        write_long(out, self.attributes.len() as i64);
        for (key, value) in &self.attributes {
            write_long(out, index(strings, key));
            write_long(out, index(strings, value));
        }
        write_long(out, self.children.len() as i64);
        for child in &self.children {
            child.write(out, strings);
        }
    }
}

fn intern(strings: &mut Vec<String>, value: &str) {
    if !strings.iter().any(|s| s == value) {
        strings.push(value.to_string());
    }
}

fn index(strings: &[String], value: &str) -> i64 {
    strings.iter().position(|s| s == value).unwrap() as i64
}

fn class(name: &str, id: i64) -> Element {
    Element::new("class").attribute("name", name).attribute("id", id.to_string())
}

fn annotation(id: i64, value: Option<&str>) -> Element {
    let element = Element::new("annotation").attribute("class", id.to_string());
    match value {
        Some(value) => element.attribute("value", value),
        None => element,
    }
}

fn field(name: &str, type_id: i64, label: &str) -> Element {
    let element = Element::new("field").attribute("name", name).attribute("class", type_id.to_string());
    let element = if type_id == THREAD { element.attribute("constantPool", "true") } else { element };
    element.child(annotation(ANNOTATION_LABEL, Some(label)))
}

fn annotation_class(name: &str, id: i64, has_value: bool, array: bool) -> Element {
    let element = class(name, id).attribute("superType", "java.lang.annotation.Annotation");
    if !has_value {
        return element;
    }
    let value = Element::new("field").attribute("name", "value").attribute("class", STRING.to_string());
    element.child(if array { value.attribute("dimension", "1") } else { value })
}

fn event_class(event_type: &EventType) -> Element {
    let mut element = class(event_type.name, event_type.id).attribute("superType", "jdk.jfr.Event");
    element = element.child(annotation(ANNOTATION_LABEL, Some(event_type.label)));
    if let Some(description) = event_type.description {
        element = element.child(annotation(ANNOTATION_DESCRIPTION, Some(description)));
    }
    let mut category = Element::new("annotation").attribute("class", ANNOTATION_CATEGORY.to_string());
    for (i, name) in event_type.category.iter().enumerate() {
        category.attributes.push((["value-0", "value-1", "value-2", "value-3"][i], name.to_string()));
    }
    element = element.child(category);
    element = element.child(field("startTime", LONG, "Start Time").child(annotation(ANNOTATION_TIMESTAMP, Some("TICKS"))));
    if event_type.has_duration {
        element = element.child(field("duration", LONG, "Duration").child(annotation(ANNOTATION_TIMESPAN, Some("TICKS"))));
    }
    if event_type.has_thread {
        element = element.child(field("eventThread", THREAD, "Event Thread"));
    }
    for descriptor in event_type.fields {
        let mut child = field(descriptor.name, type_id(descriptor.field_type), descriptor.label);
        if let Some(description) = descriptor.description {
            child = child.child(annotation(ANNOTATION_DESCRIPTION, Some(description)));
        }
        child = match descriptor.content_type {
            ContentType::None => child,
            ContentType::DataAmount => child.child(annotation(ANNOTATION_DATA_AMOUNT, Some("BYTES"))),
            ContentType::Percentage => child.child(annotation(ANNOTATION_PERCENTAGE, None)),
            ContentType::Timespan => child.child(annotation(ANNOTATION_TIMESPAN, Some("NANOSECONDS"))),
        };
        element = element.child(child);
    }
    element
}

pub fn metadata(ticks: i64, event_types: &[&EventType], gmt_offset_millis: i64) -> Vec<u8> {
    let mut metadata = Element::new("metadata");
    for (name, id) in [("boolean", BOOLEAN), ("char", CHAR), ("float", FLOAT), ("double", DOUBLE), ("byte", BYTE), ("short", SHORT), ("int", INT), ("long", LONG), ("java.lang.String", STRING)] {
        metadata = metadata.child(class(name, id));
    }
    metadata = metadata.child(
        class("java.lang.Thread", THREAD)
            .child(field("osName", STRING, "OS Thread Name"))
            .child(field("osThreadId", LONG, "OS Thread Id"))
            .child(field("javaName", STRING, "Rust Thread Name"))
            .child(field("javaThreadId", LONG, "Rust Thread Id"))
            .child(annotation(ANNOTATION_LABEL, Some("Thread"))),
    );
    metadata = metadata
        .child(annotation_class("jdk.jfr.Label", ANNOTATION_LABEL, true, false))
        .child(annotation_class("jdk.jfr.Description", ANNOTATION_DESCRIPTION, true, false))
        .child(annotation_class("jdk.jfr.Category", ANNOTATION_CATEGORY, true, true))
        .child(annotation_class("jdk.jfr.ContentType", ANNOTATION_CONTENT_TYPE, false, false))
        .child(annotation_class("jdk.jfr.Timestamp", ANNOTATION_TIMESTAMP, true, false).child(annotation(ANNOTATION_CONTENT_TYPE, None)))
        .child(annotation_class("jdk.jfr.Timespan", ANNOTATION_TIMESPAN, true, false).child(annotation(ANNOTATION_CONTENT_TYPE, None)))
        .child(annotation_class("jdk.jfr.DataAmount", ANNOTATION_DATA_AMOUNT, true, false).child(annotation(ANNOTATION_CONTENT_TYPE, None)))
        .child(annotation_class("jdk.jfr.Percentage", ANNOTATION_PERCENTAGE, false, false).child(annotation(ANNOTATION_CONTENT_TYPE, None)));
    for event_type in event_types {
        metadata = metadata.child(event_class(event_type));
    }
    let region = Element::new("region").attribute("locale", "en_US").attribute("gmtOffset", gmt_offset_millis.to_string());
    let root = Element::new("root").child(metadata).child(region);
    let mut strings = Vec::new();
    root.collect_strings(&mut strings);
    let mut body = Vec::new();
    write_long(&mut body, METADATA_TYPE_ID);
    write_long(&mut body, ticks);
    write_long(&mut body, 0);
    write_long(&mut body, 1);
    write_long(&mut body, strings.len() as i64);
    for string in &strings {
        write_string(&mut body, Some(string));
    }
    root.write(&mut body, &strings);
    with_size(body)
}

pub struct Header {
    pub chunk_size: i64,
    pub constant_pool_offset: i64,
    pub metadata_offset: i64,
    pub start_nanos: i64,
    pub duration_nanos: i64,
    pub start_ticks: i64,
    pub ticks_per_second: i64,
}

impl Header {
    pub fn write(&self) -> [u8; HEADER_SIZE] {
        let mut out = [0u8; HEADER_SIZE];
        out[0..4].copy_from_slice(b"FLR\0");
        out[4..6].copy_from_slice(&2u16.to_be_bytes());
        out[6..8].copy_from_slice(&1u16.to_be_bytes());
        for (i, value) in [self.chunk_size, self.constant_pool_offset, self.metadata_offset, self.start_nanos, self.duration_nanos, self.start_ticks, self.ticks_per_second].iter().enumerate() {
            out[8 + i * 8..16 + i * 8].copy_from_slice(&value.to_be_bytes());
        }
        out[64..68].copy_from_slice(&(FLAG_COMPRESSED_INTS | FLAG_CHUNK_FINAL).to_be_bytes());
        out
    }
}
