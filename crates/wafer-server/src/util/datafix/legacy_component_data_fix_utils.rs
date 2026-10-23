use crate::datafixers::dynamic::Dynamic;

const EMPTY_CONTENTS: &str = "{\"text\":\"\"}";

pub fn create_empty_component() -> Dynamic {
    Dynamic::new(wafer_protocol::nbt::Nbt::String(EMPTY_CONTENTS.to_string()))
}
