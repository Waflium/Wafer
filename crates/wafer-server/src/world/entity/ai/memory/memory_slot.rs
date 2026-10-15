use super::memory_value::MemoryValue;

const NEVER_EXPIRE: i64 = i64::MAX;

#[derive(Clone, Debug)]
pub struct MemorySlot {
    value: Option<MemoryValue>,
    time_to_live: i64,
}

impl MemorySlot {
    pub fn create() -> MemorySlot {
        MemorySlot { value: None, time_to_live: NEVER_EXPIRE }
    }

    pub fn tick(&mut self) {
        if self.has_value() && self.can_expire() {
            if self.has_expired() {
                self.clear();
            } else {
                self.time_to_live -= 1;
            }
        }
    }

    pub fn set_with_expiry(&mut self, value: MemoryValue, time_to_live: i64) {
        self.value = Some(value);
        self.time_to_live = time_to_live;
    }

    pub fn set(&mut self, value: MemoryValue) {
        self.set_with_expiry(value, NEVER_EXPIRE);
    }

    pub fn clear(&mut self) {
        self.value = None;
        self.time_to_live = NEVER_EXPIRE;
    }

    pub fn has_value(&self) -> bool {
        self.value.is_some()
    }

    pub fn value(&self) -> Option<&MemoryValue> {
        self.value.as_ref()
    }

    pub fn can_expire(&self) -> bool {
        self.time_to_live != NEVER_EXPIRE
    }

    pub fn has_expired(&self) -> bool {
        self.time_to_live <= 0
    }

    pub fn time_to_live(&self) -> i64 {
        self.time_to_live
    }
}
