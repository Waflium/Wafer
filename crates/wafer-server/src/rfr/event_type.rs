use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FieldType {
    Boolean,
    Int,
    Long,
    Float,
    String,
    Thread,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ContentType {
    None,
    DataAmount,
    Percentage,
    Timespan,
}

pub struct ValueDescriptor {
    pub name: &'static str,
    pub label: &'static str,
    pub description: Option<&'static str>,
    pub field_type: FieldType,
    pub content_type: ContentType,
}

impl ValueDescriptor {
    pub const fn new(name: &'static str, label: &'static str, field_type: FieldType) -> ValueDescriptor {
        ValueDescriptor { name, label, description: None, field_type, content_type: ContentType::None }
    }

    pub const fn with_content(self, content_type: ContentType) -> ValueDescriptor {
        ValueDescriptor { content_type, ..self }
    }

    pub const fn with_description(self, description: &'static str) -> ValueDescriptor {
        ValueDescriptor { description: Some(description), ..self }
    }
}

pub struct EventType {
    pub id: i64,
    pub name: &'static str,
    pub label: &'static str,
    pub description: Option<&'static str>,
    pub category: &'static [&'static str],
    pub has_duration: bool,
    pub has_thread: bool,
    pub fields: &'static [ValueDescriptor],
    enabled: AtomicBool,
}

#[derive(Clone, Copy, Debug)]
pub enum Value<'a> {
    Boolean(bool),
    Int(i32),
    Long(i64),
    Float(f32),
    String(&'a str),
    NullString,
    Thread(u64),
}

impl EventType {
    pub const fn new(
        id: i64,
        name: &'static str,
        label: &'static str,
        category: &'static [&'static str],
        has_duration: bool,
        has_thread: bool,
        fields: &'static [ValueDescriptor],
    ) -> EventType {
        EventType { id, name, label, description: None, category, has_duration, has_thread, fields, enabled: AtomicBool::new(false) }
    }

    pub const fn with_description(self, description: &'static str) -> EventType {
        EventType { description: Some(description), ..self }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub(super) fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    pub fn commit(&self, start_ticks: i64, end_ticks: i64, values: &[Value]) {
        if self.is_enabled() {
            super::recording::append(self, start_ticks, end_ticks, values);
        }
    }

    pub fn commit_now(&self, values: &[Value]) {
        let now = super::ticks();
        self.commit(now, now, values);
    }
}
