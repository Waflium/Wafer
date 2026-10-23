use wafer_protocol::nbt::Nbt;

use crate::commands::arguments::nbt_path_argument::NbtPath;
use crate::util::codec::{self, DecodeResult, Record};
use crate::world::level::storage::loot::loot_context::LootContext;

pub struct StoredNumberAccess {
    storage: String,
    path: NbtPath,
}

impl StoredNumberAccess {
    pub fn decode_partial(tag: &Nbt) -> DecodeResult<StoredNumberAccess> {
        let mut record = Record::default();
        let storage = record.required(tag, "storage", codec::identifier);
        let path = record.required(tag, "path", NbtPath::decode);
        record.build(|| Some(StoredNumberAccess { storage: storage?, path: path? }))
    }

    pub fn get_numeric_tag(&self, context: &mut LootContext) -> Option<Nbt> {
        let value = Nbt::Compound(context.level().command_storage.borrow_mut().get(&self.storage));
        let selected_tags = self.path.get(&value).ok()?;
        match selected_tags.as_slice() {
            [tag @ (Nbt::Byte(_) | Nbt::Short(_) | Nbt::Int(_) | Nbt::Long(_) | Nbt::Float(_) | Nbt::Double(_))] => Some(tag.clone()),
            _ => None,
        }
    }
}
