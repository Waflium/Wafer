#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MemoryStatus {
    ValuePresent,
    ValueAbsent,
    Registered,
}
