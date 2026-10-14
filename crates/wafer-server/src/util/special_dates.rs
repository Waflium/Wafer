use chrono::Datelike;

pub fn day_now() -> (u32, u32) {
    let now = chrono::Local::now();
    (now.month(), now.day())
}

pub fn is_halloween() -> bool {
    day_now() == (10, 31)
}
