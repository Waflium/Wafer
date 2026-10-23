pub fn make_key(version: i32) -> i32 {
    make_key_with_sub_version(version, 0)
}

pub fn make_key_with_sub_version(version: i32, sub_version: i32) -> i32 {
    version * 10 + sub_version
}

pub fn get_version(key: i32) -> i32 {
    key / 10
}

pub fn get_sub_version(key: i32) -> i32 {
    key % 10
}
