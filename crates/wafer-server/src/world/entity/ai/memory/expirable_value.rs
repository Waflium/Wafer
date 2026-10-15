use super::memory_value::MemoryValue;

#[derive(Clone, Debug)]
pub struct ExpirableValue {
    pub value: MemoryValue,
    pub time_to_live: Option<i64>,
}

impl ExpirableValue {
    pub fn of(value: MemoryValue) -> ExpirableValue {
        ExpirableValue { value, time_to_live: None }
    }

    pub fn of_expiring(value: MemoryValue, ticks_until_expiry: i64) -> ExpirableValue {
        ExpirableValue { value, time_to_live: Some(ticks_until_expiry) }
    }
}
