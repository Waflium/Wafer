use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::time::Duration;

use crate::rfr::chunk::{BOOLEAN, BYTE, CHAR, CHECKPOINT_TYPE_ID, DOUBLE, FLOAT, HEADER_SIZE, INT, LONG, METADATA_TYPE_ID, SHORT, STRING};

#[derive(Clone, Debug, PartialEq)]
pub struct RecordedThread {
    pub os_name: Option<String>,
    pub java_name: Option<String>,
}

impl RecordedThread {
    pub fn get_java_name(&self) -> Option<&str> {
        self.java_name.as_deref()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum RecordedValue {
    Boolean(bool),
    Long(i64),
    Float(f32),
    Double(f64),
    String(Option<String>),
    Thread(Option<RecordedThread>),
    Object(Vec<(String, RecordedValue)>),
    Reference(i64),
}

#[derive(Clone, Debug)]
pub struct RecordedEvent {
    name: String,
    start_nanos: i64,
    duration_nanos: i64,
    fields: Vec<(String, RecordedValue, Option<String>)>,
}

impl RecordedEvent {
    pub fn get_event_type_name(&self) -> &str {
        &self.name
    }

    pub fn get_start_time(&self) -> i64 {
        self.start_nanos
    }

    pub fn get_end_time(&self) -> i64 {
        self.start_nanos + self.duration_nanos
    }

    pub fn get_duration(&self) -> Duration {
        Duration::from_nanos(self.duration_nanos.max(0) as u64)
    }

    fn field(&self, name: &str) -> Option<&(String, RecordedValue, Option<String>)> {
        self.fields.iter().find(|(field, _, _)| field == name)
    }

    pub fn get_long(&self, name: &str) -> i64 {
        match self.field(name) {
            Some((_, RecordedValue::Long(value), _)) => *value,
            _ => 0,
        }
    }

    pub fn get_int(&self, name: &str) -> i32 {
        self.get_long(name) as i32
    }

    pub fn get_float(&self, name: &str) -> f32 {
        match self.field(name) {
            Some((_, RecordedValue::Float(value), _)) => *value,
            Some((_, RecordedValue::Double(value), _)) => *value as f32,
            _ => 0.0,
        }
    }

    pub fn get_boolean(&self, name: &str) -> bool {
        matches!(self.field(name), Some((_, RecordedValue::Boolean(true), _)))
    }

    pub fn get_string(&self, name: &str) -> Option<String> {
        match self.field(name) {
            Some((_, RecordedValue::String(value), _)) => value.clone(),
            _ => None,
        }
    }

    pub fn get_thread(&self, name: &str) -> Option<RecordedThread> {
        match self.field(name) {
            Some((_, RecordedValue::Thread(value), _)) => value.clone(),
            _ => None,
        }
    }

    pub fn get_duration_field(&self, name: &str, ticks_per_second: i64) -> Duration {
        match self.field(name) {
            Some((_, RecordedValue::Long(value), unit)) if unit.as_deref() == Some("TICKS") => Duration::from_nanos((*value as i128 * 1_000_000_000 / ticks_per_second as i128).max(0) as u64),
            Some((_, RecordedValue::Long(value), _)) => Duration::from_nanos((*value).max(0) as u64),
            _ => Duration::ZERO,
        }
    }
}

struct FieldDescriptor {
    name: String,
    type_id: i64,
    constant_pool: bool,
    array: bool,
    timespan: Option<String>,
}

struct TypeDescriptor {
    name: String,
    fields: Vec<FieldDescriptor>,
    event: bool,
}

struct Element {
    name: String,
    attributes: Vec<(String, String)>,
    children: Vec<Element>,
}

impl Element {
    fn attribute(&self, key: &str) -> Option<&str> {
        self.attributes.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }
}

struct Input<'a> {
    bytes: &'a [u8],
    position: usize,
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_string())
}

impl<'a> Input<'a> {
    fn byte(&mut self) -> io::Result<u8> {
        let byte = *self.bytes.get(self.position).ok_or_else(|| invalid("Unexpected end of recording"))?;
        self.position += 1;
        Ok(byte)
    }

    fn long(&mut self) -> io::Result<i64> {
        let mut value = 0u64;
        for i in 0..8 {
            let byte = self.byte()?;
            value |= ((byte & 0x7F) as u64) << (7 * i);
            if byte & 0x80 == 0 {
                return Ok(value as i64);
            }
        }
        Ok((value | (self.byte()? as u64) << 56) as i64)
    }

    fn raw(&mut self, n: usize) -> io::Result<&'a [u8]> {
        let slice = self.bytes.get(self.position..self.position + n).ok_or_else(|| invalid("Unexpected end of recording"))?;
        self.position += n;
        Ok(slice)
    }

    fn raw_long(&mut self) -> io::Result<i64> {
        Ok(i64::from_be_bytes(self.raw(8)?.try_into().unwrap()))
    }

    fn string(&mut self) -> io::Result<Option<String>> {
        match self.byte()? {
            0 => Ok(None),
            1 => Ok(Some(String::new())),
            3 => {
                let len = self.long()? as usize;
                Ok(Some(String::from_utf8_lossy(self.raw(len)?).into_owned()))
            }
            4 => {
                let len = self.long()? as usize;
                let mut text = String::new();
                for _ in 0..len {
                    text.push(char::from_u32(self.long()? as u32).unwrap_or('\u{FFFD}'));
                }
                Ok(Some(text))
            }
            5 => {
                let len = self.long()? as usize;
                Ok(Some(self.raw(len)?.iter().map(|&b| b as char).collect()))
            }
            other => Err(invalid(&format!("Unknown string encoding {other}"))),
        }
    }
}

fn read_element(input: &mut Input, strings: &[String]) -> io::Result<Element> {
    let string = |index: i64| strings.get(index as usize).cloned().ok_or_else(|| invalid("Bad metadata string index"));
    let name = string(input.long()?)?;
    let mut attributes = Vec::new();
    for _ in 0..input.long()? {
        let key = string(input.long()?)?;
        let value = string(input.long()?)?;
        attributes.push((key, value));
    }
    let mut children = Vec::new();
    for _ in 0..input.long()? {
        children.push(read_element(input, strings)?);
    }
    Ok(Element { name, attributes, children })
}

pub struct RecordingFile {
    events: std::vec::IntoIter<RecordedEvent>,
    pub ticks_per_second: i64,
}

impl RecordingFile {
    pub fn open(path: &Path) -> io::Result<RecordingFile> {
        let bytes = std::fs::read(path)?;
        let mut events = Vec::new();
        let mut ticks_per_second = 1_000_000_000;
        let mut chunk_start = 0;
        while chunk_start < bytes.len() {
            ticks_per_second = read_chunk(&bytes[chunk_start..], &mut events, &mut chunk_start)?;
        }
        Ok(RecordingFile { events: events.into_iter(), ticks_per_second })
    }

    pub fn has_more_events(&self) -> bool {
        self.events.len() > 0
    }

    pub fn read_event(&mut self) -> Option<RecordedEvent> {
        self.events.next()
    }
}

fn read_chunk(bytes: &[u8], events: &mut Vec<RecordedEvent>, chunk_start: &mut usize) -> io::Result<i64> {
    if bytes.len() < HEADER_SIZE || &bytes[0..4] != b"FLR\0" {
        return Err(invalid("Not a Flight Recorder file"));
    }
    let mut header = Input { bytes, position: 8 };
    let chunk_size = header.raw_long()? as usize;
    let constant_pool_offset = header.raw_long()? as usize;
    let metadata_offset = header.raw_long()? as usize;
    let start_nanos = header.raw_long()?;
    let _duration_nanos = header.raw_long()?;
    let start_ticks = header.raw_long()?;
    let ticks_per_second = header.raw_long()?;
    let bytes = bytes.get(..chunk_size).ok_or_else(|| invalid("Truncated chunk"))?;
    *chunk_start += chunk_size;
    let types = read_metadata(&mut Input { bytes, position: metadata_offset })?;
    let pools = read_constant_pools(bytes, constant_pool_offset, &types)?;
    let to_nanos = |ticks: i64| start_nanos + ((ticks - start_ticks) as i128 * 1_000_000_000 / ticks_per_second as i128) as i64;
    let mut input = Input { bytes, position: HEADER_SIZE };
    while input.position < bytes.len() {
        let event_start = input.position;
        let size = input.long()? as usize;
        let type_id = input.long()?;
        if type_id != METADATA_TYPE_ID && type_id != CHECKPOINT_TYPE_ID
            && let Some(descriptor) = types.get(&type_id).filter(|descriptor| descriptor.event)
        {
            let mut start = 0;
            let mut duration = 0;
            let mut fields = Vec::new();
            for field in &descriptor.fields {
                let value = read_value(&mut input, field, &types, &pools)?;
                match (field.name.as_str(), &value) {
                    ("startTime", RecordedValue::Long(ticks)) => start = *ticks,
                    ("duration", RecordedValue::Long(ticks)) => duration = *ticks,
                    _ => fields.push((field.name.clone(), value, field.timespan.clone())),
                }
            }
            let start_nanos = to_nanos(start);
            events.push(RecordedEvent { name: descriptor.name.clone(), start_nanos, duration_nanos: to_nanos(start + duration) - start_nanos, fields });
        }
        input.position = event_start + size.max(1);
    }
    Ok(ticks_per_second)
}

fn read_metadata(input: &mut Input) -> io::Result<HashMap<i64, TypeDescriptor>> {
    input.long()?;
    if input.long()? != METADATA_TYPE_ID {
        return Err(invalid("Expected metadata event"));
    }
    input.long()?;
    input.long()?;
    input.long()?;
    let mut strings = Vec::new();
    for _ in 0..input.long()? {
        strings.push(input.string()?.unwrap_or_default());
    }
    let root = read_element(input, &strings)?;
    let mut types = HashMap::new();
    for metadata in root.children.iter().filter(|child| child.name == "metadata") {
        for class in metadata.children.iter().filter(|child| child.name == "class") {
            let id: i64 = class.attribute("id").and_then(|id| id.parse().ok()).ok_or_else(|| invalid("Class without id"))?;
            let fields = class
                .children
                .iter()
                .filter(|child| child.name == "field")
                .map(|field| FieldDescriptor {
                    name: field.attribute("name").unwrap_or_default().to_string(),
                    type_id: field.attribute("class").and_then(|id| id.parse().ok()).unwrap_or(0),
                    constant_pool: field.attribute("constantPool") == Some("true"),
                    array: field.attribute("dimension") == Some("1"),
                    timespan: field
                        .children
                        .iter()
                        .find(|annotation| annotation.name == "annotation" && annotation.attribute("class") == Some(&crate::rfr::chunk::ANNOTATION_TIMESPAN.to_string()))
                        .and_then(|annotation| annotation.attribute("value"))
                        .map(str::to_string),
                })
                .collect();
            types.insert(id, TypeDescriptor { name: class.attribute("name").unwrap_or_default().to_string(), fields, event: class.attribute("superType") == Some("jdk.jfr.Event") });
        }
    }
    Ok(types)
}

type Pools = HashMap<i64, HashMap<i64, RecordedValue>>;

fn read_constant_pools(bytes: &[u8], mut offset: usize, types: &HashMap<i64, TypeDescriptor>) -> io::Result<Pools> {
    let mut pools: Pools = HashMap::new();
    loop {
        let mut input = Input { bytes, position: offset };
        input.long()?;
        if input.long()? != CHECKPOINT_TYPE_ID {
            return Err(invalid("Expected checkpoint event"));
        }
        input.long()?;
        input.long()?;
        let delta = input.long()?;
        input.byte()?;
        for _ in 0..input.long()? {
            let type_id = input.long()?;
            let descriptor = types.get(&type_id).ok_or_else(|| invalid("Unknown constant pool type"))?;
            for _ in 0..input.long()? {
                let key = input.long()?;
                let mut fields = Vec::new();
                for field in &descriptor.fields {
                    fields.push((field.name.clone(), read_value(&mut input, field, types, &HashMap::new())?));
                }
                pools.entry(type_id).or_default().insert(key, RecordedValue::Object(fields));
            }
        }
        if delta == 0 {
            return Ok(pools);
        }
        offset = (offset as i64 + delta) as usize;
    }
}

fn read_value(input: &mut Input, field: &FieldDescriptor, types: &HashMap<i64, TypeDescriptor>, pools: &Pools) -> io::Result<RecordedValue> {
    if field.array {
        let mut values = Vec::new();
        for i in 0..input.long()? {
            values.push((i.to_string(), read_single(input, field, types, pools)?));
        }
        return Ok(RecordedValue::Object(values));
    }
    read_single(input, field, types, pools)
}

fn read_single(input: &mut Input, field: &FieldDescriptor, types: &HashMap<i64, TypeDescriptor>, pools: &Pools) -> io::Result<RecordedValue> {
    Ok(match field.type_id {
        BOOLEAN => RecordedValue::Boolean(input.byte()? != 0),
        BYTE => RecordedValue::Long(input.byte()? as i8 as i64),
        CHAR | SHORT | INT | LONG => RecordedValue::Long(input.long()?),
        FLOAT => RecordedValue::Float(f32::from_bits(u32::from_be_bytes(input.raw(4)?.try_into().unwrap()))),
        DOUBLE => RecordedValue::Double(f64::from_bits(u64::from_be_bytes(input.raw(8)?.try_into().unwrap()))),
        STRING => RecordedValue::String(input.string()?),
        type_id if field.constant_pool => {
            let key = input.long()?;
            let value = pools.get(&type_id).and_then(|pool| pool.get(&key)).cloned();
            match (types.get(&type_id).map(|descriptor| descriptor.name.as_str()), value) {
                (Some("java.lang.Thread"), Some(RecordedValue::Object(fields))) => {
                    let string = |name: &str| fields.iter().find(|(field, _)| field == name).and_then(|(_, value)| if let RecordedValue::String(text) = value { text.clone() } else { None });
                    RecordedValue::Thread(Some(RecordedThread { os_name: string("osName"), java_name: string("javaName") }))
                }
                (Some("java.lang.Thread"), _) => RecordedValue::Thread(None),
                (_, Some(value)) => value,
                (_, None) => RecordedValue::Reference(key),
            }
        }
        type_id => {
            let descriptor = types.get(&type_id).ok_or_else(|| invalid("Unknown field type"))?;
            let mut fields = Vec::new();
            for nested in &descriptor.fields {
                fields.push((nested.name.clone(), read_value(input, nested, types, pools)?));
            }
            RecordedValue::Object(fields)
        }
    })
}
