use wafer_core::java_string::JavaStr;
use crate::java_string::{from_utf16, utf16_units};
use crate::lazily_parsed_number::{double_value, exact_nbt, parse_long};
use crate::fault::{Fault, Frame};
use crate::{DecodeError, Reader, Writer};

const WRITE_UTF: [Frame; 3] = [
    Frame::Fixed("java.base/java.io.DataOutputStream.writeUTF(DataOutputStream.java:370)"),
    Frame::Fixed("java.base/java.io.DataOutputStream.writeUTF(DataOutputStream.java:338)"),
    Frame::Fixed("io.netty.buffer.ByteBufOutputStream.writeUTF(ByteBufOutputStream.java:160)"),
];
const STRING_TAG_WRITE: Frame = Frame::Fixed("net.minecraft.nbt.StringTag.write(StringTag.java:76)");
const NAMED_TAG_KEY: Frame = Frame::Fixed("net.minecraft.nbt.CompoundTag.writeNamedTag(CompoundTag.java:504)");
const NAMED_TAG_VALUE: [Frame; 1] = [Frame::Fixed("net.minecraft.nbt.CompoundTag.writeNamedTag(CompoundTag.java:506)")];
const COMPOUND_TAG_WRITE: [Frame; 1] = [Frame::Fixed("net.minecraft.nbt.CompoundTag.write(CompoundTag.java:208)")];
const LIST_TAG_WRITE: [Frame; 1] = [Frame::Fixed("net.minecraft.nbt.ListTag.write(ListTag.java:193)")];
const WRITE_ANY_TAG: Frame = Frame::Fixed("net.minecraft.nbt.NbtIo.writeAnyTag(NbtIo.java:156)");
const WRITE_NBT_CAUSE: Frame = Frame::Fixed("net.minecraft.network.FriendlyByteBuf.writeNbt(FriendlyByteBuf.java:459)");
const WRITE_NBT_WRAP: Frame = Frame::Fixed("net.minecraft.network.FriendlyByteBuf.writeNbt(FriendlyByteBuf.java:461)");

#[derive(Clone, Debug)]
pub enum Nbt {
    End,
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<i8>),
    String(String),
    List(Vec<Nbt>),
    Compound(TagMap),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
    LazilyParsedNumber(String),
}

#[derive(Clone, Default)]
pub struct TagMap {
    entries: Vec<(String, Nbt)>,
    capacity: u32,
}

impl TagMap {
    pub const fn new() -> TagMap {
        TagMap { entries: Vec::new(), capacity: 0 }
    }

    pub fn into_vec(self) -> Vec<(String, Nbt)> {
        self.entries
    }

    pub fn table_capacity(&self) -> usize {
        let mut capacity = if self.capacity == 0 { 16 } else { self.capacity as usize };
        while self.entries.len() > capacity * 3 / 4 {
            capacity *= 2;
        }
        capacity
    }

    pub fn iteration_order(&self) -> Vec<usize> {
        let capacity = self.table_capacity();
        let mut order: Vec<usize> = (0..self.entries.len()).collect();
        order.sort_by_key(|&i| java_bucket(&self.entries[i].0, capacity));
        order
    }

    pub fn iteration(&self) -> impl Iterator<Item = &(String, Nbt)> {
        self.iteration_order().into_iter().map(|i| &self.entries[i])
    }

    pub fn into_iteration(self) -> Vec<(String, Nbt)> {
        let order = self.iteration_order();
        let mut slots: Vec<Option<(String, Nbt)>> = self.entries.into_iter().map(Some).collect();
        order.into_iter().map(|i| slots[i].take().expect("each entry is visited once")).collect()
    }

    pub fn copy(&self) -> TagMap {
        TagMap { entries: self.iteration().map(|(key, value)| (key.clone(), value.copy())).collect(), capacity: 0 }
    }

    pub fn merge(&mut self, other: &TagMap) {
        for (key, other_tag) in other.iteration() {
            match (self.entries.iter_mut().find(|(k, _)| k == key), other_tag) {
                (Some((_, Nbt::Compound(self_compound))), Nbt::Compound(other_compound)) => self_compound.merge(other_compound),
                _ => self.put(key, other_tag.copy()),
            }
        }
    }

    pub fn shallow_copy(self) -> TagMap {
        let size = self.entries.len();
        let entries = self.into_iteration();
        TagMap { entries, capacity: if size == 0 { 0 } else { table_size_for_copy(size) as u32 } }
    }

    pub fn remove_key(&mut self, key: &str) -> Option<Nbt> {
        self.capacity = self.table_capacity() as u32;
        let index = self.entries.iter().position(|(k, _)| k == key)?;
        Some(self.entries.remove(index).1)
    }

    pub fn put(&mut self, key: &str, value: Nbt) {
        match self.entries.iter_mut().find(|(k, _)| k == key) {
            Some((_, slot)) => *slot = value,
            None => {
                self.entries.push((key.to_string(), value));
                self.capacity = self.table_capacity() as u32;
            }
        }
    }
}

fn table_size_for_copy(size: usize) -> usize {
    ((size as f64 / 0.75f64).ceil() as usize).next_power_of_two()
}

impl std::fmt::Debug for TagMap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.entries.fmt(f)
    }
}

impl std::ops::Deref for TagMap {
    type Target = Vec<(String, Nbt)>;

    fn deref(&self) -> &Vec<(String, Nbt)> {
        &self.entries
    }
}

impl std::ops::DerefMut for TagMap {
    fn deref_mut(&mut self) -> &mut Vec<(String, Nbt)> {
        self.capacity = self.table_capacity() as u32;
        &mut self.entries
    }
}

impl From<Vec<(String, Nbt)>> for TagMap {
    fn from(entries: Vec<(String, Nbt)>) -> TagMap {
        TagMap { entries, capacity: 0 }
    }
}

impl FromIterator<(String, Nbt)> for TagMap {
    fn from_iter<I: IntoIterator<Item = (String, Nbt)>>(iter: I) -> TagMap {
        TagMap { entries: iter.into_iter().collect(), capacity: 0 }
    }
}

impl IntoIterator for TagMap {
    type Item = (String, Nbt);
    type IntoIter = std::vec::IntoIter<(String, Nbt)>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}

impl<'a> IntoIterator for &'a TagMap {
    type Item = &'a (String, Nbt);
    type IntoIter = std::slice::Iter<'a, (String, Nbt)>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter()
    }
}

impl<'a> IntoIterator for &'a mut TagMap {
    type Item = &'a mut (String, Nbt);
    type IntoIter = std::slice::IterMut<'a, (String, Nbt)>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter_mut()
    }
}

impl Extend<(String, Nbt)> for TagMap {
    fn extend<I: IntoIterator<Item = (String, Nbt)>>(&mut self, iter: I) {
        self.entries.extend(iter);
    }
}

pub fn float_tag_value(v: f32) -> f32 {
    if v == 0.0 { 0.0 } else { v }
}

pub fn double_tag_value(v: f64) -> f64 {
    if v == 0.0 { 0.0 } else { v }
}

impl PartialEq for Nbt {
    fn eq(&self, other: &Nbt) -> bool {
        match (self, other) {
            (Nbt::End, Nbt::End) => true,
            (Nbt::Byte(a), Nbt::Byte(b)) => a == b,
            (Nbt::Short(a), Nbt::Short(b)) => a == b,
            (Nbt::Int(a), Nbt::Int(b)) => a == b,
            (Nbt::Long(a), Nbt::Long(b)) => a == b,
            (Nbt::Float(a), Nbt::Float(b)) => float_tag_value(*a).to_bits() == float_tag_value(*b).to_bits(),
            (Nbt::Double(a), Nbt::Double(b)) => double_tag_value(*a).to_bits() == double_tag_value(*b).to_bits(),
            (Nbt::ByteArray(a), Nbt::ByteArray(b)) => a == b,
            (Nbt::String(a), Nbt::String(b)) => a == b,
            (Nbt::List(a), Nbt::List(b)) => a == b,
            (Nbt::Compound(a), Nbt::Compound(b)) => a.len() == b.len() && a.iter().all(|(k, v)| b.iter().any(|(k2, v2)| k == k2 && v == v2)),
            (Nbt::IntArray(a), Nbt::IntArray(b)) => a == b,
            (Nbt::LongArray(a), Nbt::LongArray(b)) => a == b,
            (Nbt::LazilyParsedNumber(a), Nbt::LazilyParsedNumber(b)) => a == b,
            _ => false,
        }
    }
}

impl Nbt {
    pub fn copy(&self) -> Nbt {
        match self {
            Nbt::List(items) => Nbt::List(items.iter().map(Nbt::copy).collect()),
            Nbt::Compound(fields) => Nbt::Compound(fields.copy()),
            other => other.clone(),
        }
    }

    fn tag_id(&self) -> u8 {
        match self {
            Nbt::End => 0,
            Nbt::Byte(_) => 1,
            Nbt::Short(_) => 2,
            Nbt::Int(_) => 3,
            Nbt::Long(_) => 4,
            Nbt::Float(_) => 5,
            Nbt::Double(_) => 6,
            Nbt::ByteArray(_) => 7,
            Nbt::String(_) => 8,
            Nbt::List(_) => 9,
            Nbt::Compound(_) => 10,
            Nbt::IntArray(_) => 11,
            Nbt::LongArray(_) => 12,
            Nbt::LazilyParsedNumber(text) => exact_nbt(text).tag_id(),
        }
    }

    pub fn write_named(&self, name: &str, w: &mut Writer) {
        w.u8(self.tag_id());
        write_string(w, name, None);
        self.write_payload(w);
    }

    pub fn read_named(r: &mut Reader) -> Result<(String, Nbt), DecodeError> {
        let id = r.u8()?;
        let name = read_string(r)?;
        Ok((name, read_payload(r, id, 0)?))
    }

    pub fn read(r: &mut Reader) -> Result<Nbt, DecodeError> {
        let id = r.u8()?;
        read_payload(r, id, 0)
    }

    pub fn get(&self, key: &str) -> Option<&Nbt> {
        match self {
            Nbt::Compound(fields) => fields.iter().find(|(k, v)| k == key && !matches!(v, Nbt::End)).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Nbt::Byte(v) => Some(*v as i64),
            Nbt::Short(v) => Some(*v as i64),
            Nbt::Int(v) => Some(*v as i64),
            Nbt::Long(v) => Some(*v),
            Nbt::LazilyParsedNumber(text) => parse_long(text),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Nbt::Float(v) => Some(*v as f64),
            Nbt::Double(v) => Some(*v),
            Nbt::LazilyParsedNumber(text) => Some(double_value(text)),
            _ => self.as_i64().map(|v| v as f64),
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Nbt::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_list(&self) -> &[Nbt] {
        match self {
            Nbt::List(items) => items,
            _ => &[],
        }
    }

    pub fn write(&self, w: &mut Writer) {
        let faults = w.fault_count();
        w.u8(self.tag_id());
        self.write_payload(w);
        if w.fault_count() != faults {
            w.wrap_faults(faults, |mut root| {
                root.push_frames(&[WRITE_ANY_TAG, WRITE_NBT_CAUSE]);
                let description = format!("io.netty.handler.codec.EncoderException: {}", root.description);
                Fault::new(description, &[WRITE_NBT_WRAP]).with_cause(root)
            });
        }
    }

    fn write_payload(&self, w: &mut Writer) {
        match self {
            Nbt::End => {}
            Nbt::Byte(v) => w.u8(*v as u8),
            Nbt::Short(v) => w.bytes(&v.to_be_bytes()),
            Nbt::Int(v) => w.bytes(&v.to_be_bytes()),
            Nbt::Long(v) => w.i64(*v),
            Nbt::Float(v) => w.bytes(&float_tag_value(*v).to_be_bytes()),
            Nbt::Double(v) => w.bytes(&double_tag_value(*v).to_be_bytes()),
            Nbt::ByteArray(v) => {
                w.bytes(&(v.len() as i32).to_be_bytes());
                v.iter().for_each(|b| w.u8(*b as u8));
            }
            Nbt::String(s) => write_string(w, s, Some(STRING_TAG_WRITE)),
            Nbt::List(items) => w.layer(&LIST_TAG_WRITE, |w| {
                let element = list_element_type(items);
                w.u8(element);
                w.bytes(&(items.len() as i32).to_be_bytes());
                for item in items {
                    if element == 10 && !matches!(item, Nbt::Compound(f) if !is_wrapper(f)) {
                        w.layer(&COMPOUND_TAG_WRITE, |w| {
                            w.u8(item.tag_id());
                            write_string(w, "", Some(NAMED_TAG_KEY));
                            w.layer(&NAMED_TAG_VALUE, |w| item.write_payload(w));
                            w.u8(0);
                        });
                    } else {
                        item.write_payload(w);
                    }
                }
            }),
            Nbt::Compound(fields) => w.layer(&COMPOUND_TAG_WRITE, |w| {
                for i in fields.iteration_order() {
                    let (name, value) = &fields[i];
                    w.u8(value.tag_id());
                    if matches!(value, Nbt::End) {
                        continue;
                    }
                    write_string(w, name, Some(NAMED_TAG_KEY));
                    w.layer(&NAMED_TAG_VALUE, |w| value.write_payload(w));
                }
                w.u8(0);
            }),
            Nbt::IntArray(v) => {
                w.bytes(&(v.len() as i32).to_be_bytes());
                v.iter().for_each(|i| w.bytes(&i.to_be_bytes()));
            }
            Nbt::LongArray(v) => {
                w.bytes(&(v.len() as i32).to_be_bytes());
                v.iter().for_each(|l| w.i64(*l));
            }
            Nbt::LazilyParsedNumber(text) => exact_nbt(text).write_payload(w),
        }
    }
}

fn write_string(w: &mut Writer, s: &str, origin: Option<Frame>) {
    let mut bytes = Vec::with_capacity(s.len());
    for unit in utf16_units(s) {
        match unit {
            0x0001..=0x007F => bytes.push(unit as u8),
            0x0000 | 0x0080..=0x07FF => {
                bytes.push(0xC0 | (unit >> 6) as u8);
                bytes.push(0x80 | (unit & 0x3F) as u8);
            }
            _ => {
                bytes.push(0xE0 | (unit >> 12) as u8);
                bytes.push(0x80 | ((unit >> 6) & 0x3F) as u8);
                bytes.push(0x80 | (unit & 0x3F) as u8);
            }
        }
    }
    if bytes.len() > u16::MAX as usize {
        let units: Vec<u16> = utf16_units(s).collect();
        let display = |units: &[u16]| char::decode_utf16(units.iter().copied()).map(|c| c.unwrap_or('?')).collect::<String>();
        let message = format!(
            "java.io.UTFDataFormatException: encoded string ({}...{}) too long: {} bytes",
            display(&units[..8]),
            display(&units[units.len() - 8..]),
            bytes.len()
        );
        let mut fault = Fault::new(message, &WRITE_UTF);
        fault.push_frames(origin.as_slice());
        w.fail_with(fault);
        w.u16(0);
        return;
    }
    w.u16(bytes.len() as u16);
    w.bytes(&bytes);
}

fn read_string(r: &mut Reader) -> Result<String, DecodeError> {
    let len = r.u16()? as usize;
    let mut units = Vec::with_capacity(len);
    let mut bytes = Vec::with_capacity(len);
    for _ in 0..len {
        bytes.push(r.u8()?);
    }
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i] as u16;
        let (unit, size) = if b & 0x80 == 0 {
            (b, 1)
        } else if b & 0xE0 == 0xC0 && i + 1 < bytes.len() {
            ((b & 0x1F) << 6 | (bytes[i + 1] as u16 & 0x3F), 2)
        } else if b & 0xF0 == 0xE0 && i + 2 < bytes.len() {
            ((b & 0x0F) << 12 | (bytes[i + 1] as u16 & 0x3F) << 6 | (bytes[i + 2] as u16 & 0x3F), 3)
        } else {
            return Err(DecodeError::InvalidUtf8);
        };
        units.push(unit);
        i += size;
    }
    Ok(from_utf16(&units))
}

fn read_len(r: &mut Reader) -> Result<usize, DecodeError> {
    usize::try_from(r.i32()?).map_err(|_| DecodeError::UnexpectedEof)
}

fn read_payload(r: &mut Reader, id: u8, depth: u32) -> Result<Nbt, DecodeError> {
    if depth > 512 {
        return Err(DecodeError::UnexpectedEof);
    }
    Ok(match id {
        1 => Nbt::Byte(r.u8()? as i8),
        2 => Nbt::Short(r.i16()?),
        3 => Nbt::Int(r.i32()?),
        4 => Nbt::Long(r.i64()?),
        5 => Nbt::Float(r.f32()?),
        6 => Nbt::Double(r.f64()?),
        7 => Nbt::ByteArray((0..read_len(r)?).map(|_| r.u8().map(|b| b as i8)).collect::<Result<_, _>>()?),
        8 => Nbt::String(read_string(r)?),
        9 => {
            let element = r.u8()?;
            let len = read_len(r)?;
            Nbt::List((0..len).map(|_| read_payload(r, element, depth + 1).map(try_unwrap)).collect::<Result<_, _>>()?)
        }
        10 => {
            let mut fields = TagMap::new();
            loop {
                let field = r.u8()?;
                if field == 0 {
                    break;
                }
                let name = read_string(r)?;
                fields.push((name, read_payload(r, field, depth + 1)?));
            }
            Nbt::Compound(fields)
        }
        11 => Nbt::IntArray((0..read_len(r)?).map(|_| r.i32()).collect::<Result<_, _>>()?),
        12 => Nbt::LongArray((0..read_len(r)?).map(|_| r.i64()).collect::<Result<_, _>>()?),
        _ => return Err(DecodeError::UnexpectedEof),
    })
}

fn list_element_type(items: &[Nbt]) -> u8 {
    let mut homogeneous = 0;
    for item in items {
        let id = item.tag_id();
        if homogeneous == 0 {
            homogeneous = id;
        } else if homogeneous != id {
            return 10;
        }
    }
    homogeneous
}

fn is_wrapper(fields: &[(String, Nbt)]) -> bool {
    fields.len() == 1 && fields[0].0.is_empty()
}

fn try_unwrap(tag: Nbt) -> Nbt {
    match tag {
        Nbt::Compound(mut fields) if is_wrapper(&fields) => fields.pop().unwrap().1,
        other => other,
    }
}

pub fn text(s: impl Into<String>) -> Nbt {
    Nbt::String(s.into())
}

fn java_string_hash(s: &str) -> i32 {
    s.java_utf16().fold(0i32, |h, c| h.wrapping_mul(31).wrapping_add(c as i32))
}

fn java_bucket(key: &str, capacity: usize) -> usize {
    let h = java_string_hash(key);
    ((h ^ ((h as u32) >> 16) as i32) as u32 as usize) & (capacity - 1)
}

pub fn java_hash_map_order<S: AsRef<str>>(keys: &[S]) -> Vec<usize> {
    let mut capacity = 16usize;
    while keys.len() > capacity * 3 / 4 {
        capacity *= 2;
    }
    let mut order: Vec<usize> = (0..keys.len()).collect();
    order.sort_by_key(|&i| java_bucket(keys[i].as_ref(), capacity));
    order
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_insert_in_iteration_order_with_the_default_capacity() {
        let fields: TagMap = ["id", "Owner", "SkullOwner"].into_iter().map(|key| (key.to_string(), Nbt::Byte(0))).collect();
        let copy = fields.shallow_copy().copy();
        assert_eq!(copy.iter().map(|(key, _)| key.as_str()).collect::<Vec<_>>(), ["Owner", "SkullOwner", "id"]);
        assert_eq!(copy.table_capacity(), 16);
    }

    fn grown(keep: &[&str]) -> TagMap {
        let mut fields = TagMap::new();
        for i in 0..13 {
            fields.put(&format!("grow{i}"), Nbt::Int(i));
        }
        for key in keep {
            fields.put(key, Nbt::Int(key.len() as i32));
        }
        for i in 0..13 {
            fields.remove_key(&format!("grow{i}"));
        }
        fields
    }

    fn keys(fields: &TagMap) -> Vec<&str> {
        fields.iteration().map(|(key, _)| key.as_str()).collect()
    }

    #[test]
    fn copy_resets_the_capacity_that_removals_kept() {
        let source = grown(&["ax", "h", "i"]);
        assert_eq!(keys(&source), ["h", "i", "ax"]);
        assert_eq!(keys(&source.clone()), ["h", "i", "ax"]);
        assert_eq!(keys(&source.copy()), ["ax", "h", "i"]);
        assert_eq!(keys(&source.copy().copy()), ["ax", "h", "i"]);
    }

    #[test]
    fn copy_resets_the_capacity_of_nested_compounds_and_lists() {
        let nested: TagMap = vec![
            ("inner".to_string(), Nbt::Compound(grown(&["ax", "h", "i"]))),
            ("list".to_string(), Nbt::List(vec![Nbt::Compound(grown(&["ax", "h", "i"]))])),
        ]
        .into();
        let copy = nested.copy();
        let Some((_, Nbt::Compound(inner))) = copy.iter().find(|(key, _)| key == "inner") else { panic!("inner") };
        let Some((_, Nbt::List(list))) = copy.iter().find(|(key, _)| key == "list") else { panic!("list") };
        let Nbt::Compound(listed) = &list[0] else { panic!("listed") };
        assert_eq!(keys(inner), ["ax", "h", "i"]);
        assert_eq!(keys(listed), ["ax", "h", "i"]);
        let Some((_, Nbt::Compound(original))) = nested.iter().find(|(key, _)| key == "inner") else { panic!("inner") };
        assert_eq!(keys(original), ["h", "i", "ax"]);
    }

    #[test]
    fn merge_inserts_in_the_iteration_order_of_the_source() {
        let other = grown(&["ax", "g"]);
        assert_eq!(keys(&other), ["g", "ax"]);
        let mut target = TagMap::new();
        target.merge(&other);
        assert_eq!(keys(&target), ["g", "ax"]);
        assert_eq!(keys(&other.copy()), ["g", "ax"]);
    }

    #[test]
    fn merge_copies_the_merged_values() {
        let other: TagMap = vec![("inner".to_string(), Nbt::Compound(grown(&["ax", "h", "i"])))].into();
        let mut target = TagMap::new();
        target.merge(&other);
        let Some((_, Nbt::Compound(inner))) = target.iter().find(|(key, _)| key == "inner") else { panic!("inner") };
        assert_eq!(keys(inner), ["ax", "h", "i"]);
        let mut existing: TagMap = vec![("inner".to_string(), Nbt::Compound(TagMap::new()))].into();
        existing.merge(&other);
        let Some((_, Nbt::Compound(inner))) = existing.iter().find(|(key, _)| key == "inner") else { panic!("inner") };
        assert_eq!(keys(inner), ["ax", "h", "i"]);
    }

    #[test]
    fn vec_removals_keep_the_grown_table_capacity() {
        let mut fields: TagMap = (0..13).map(|i| (format!("grow{i}"), Nbt::Int(i))).chain(["ax", "h", "i"].map(|key| (key.to_string(), Nbt::Int(0)))).collect();
        fields.retain(|(key, _)| !key.starts_with("grow"));
        assert_eq!(keys(&fields), ["h", "i", "ax"]);
        assert_eq!(keys(&fields.copy()), ["ax", "h", "i"]);
    }

    #[test]
    fn removed_keys_keep_the_grown_table_capacity() {
        let mut fields = TagMap::new();
        for i in 0..13 {
            fields.put(&format!("k{i}"), Nbt::Byte(0));
        }
        assert!(fields.remove_key("k0").is_some());
        assert!(fields.remove_key("k0").is_none());
        for i in 1..5 {
            fields.remove_key(&format!("k{i}"));
        }
        assert_eq!(fields.len(), 8);
        assert_eq!(fields.table_capacity(), 32);
    }

    #[test]
    fn table_capacity_grows_with_the_entry_count() {
        let mut fields = TagMap::new();
        assert_eq!(fields.table_capacity(), 16);
        for i in 0..13 {
            fields.put(&format!("k{i}"), Nbt::Byte(0));
        }
        assert_eq!(fields.table_capacity(), 32);
    }

    #[test]
    fn compound_layout() {
        let mut w = Writer::new();
        Nbt::Compound(vec![("a".into(), Nbt::Byte(1))].into()).write(&mut w);
        assert_eq!(w.as_slice(), &[10, 1, 0, 1, b'a', 1, 0]);
    }

    #[test]
    fn named_round_trip() {
        let value = Nbt::Compound(vec![
            ("name".into(), Nbt::String(" é😀".into())),
            ("list".into(), Nbt::List(vec![Nbt::Int(1), Nbt::Int(2)])),
            ("longs".into(), Nbt::LongArray(vec![-1, 7])),
            ("nested".into(), Nbt::Compound(vec![("f".into(), Nbt::Float(0.5))].into())),
        ].into());
        let mut w = Writer::new();
        value.write_named("root", &mut w);
        let (name, read) = Nbt::read_named(&mut Reader::new(w.as_slice())).unwrap();
        assert_eq!((name.as_str(), read), ("root", value));
    }

    #[test]
    fn mixed_lists_wrap_on_the_wire() {
        let wrapper = Nbt::Compound(vec![(String::new(), Nbt::Int(1))].into());
        for list in [Nbt::List(vec![wrapper.clone(), Nbt::Int(2)]), Nbt::List(vec![wrapper]), Nbt::List(vec![Nbt::String("a".into()), Nbt::Byte(1)])] {
            let mut w = Writer::new();
            list.write(&mut w);
            assert_eq!(w.as_slice()[1], 10);
            assert_eq!(Nbt::read(&mut Reader::new(w.as_slice())).unwrap(), list);
        }
    }

    #[test]
    fn modified_utf8() {
        let mut w = Writer::new();
        Nbt::String("\0😀".into()).write(&mut w);
        assert_eq!(w.as_slice(), &[8, 0, 8, 0xC0, 0x80, 0xED, 0xA0, 0xBD, 0xED, 0xB8, 0x80]);
    }

    #[test]
    fn strings_over_65535_bytes_fail_like_write_utf() {
        let mut w = Writer::new();
        Nbt::String("a".repeat(65535)).write(&mut w);
        assert!(w.faults().is_empty());
        let mut w = Writer::new();
        Nbt::String("a".repeat(70000)).write(&mut w);
        let message = "java.io.UTFDataFormatException: encoded string (aaaaaaaa...aaaaaaaa) too long: 70000 bytes";
        assert_eq!(w.faults()[0].description, format!("io.netty.handler.codec.EncoderException: {message}"));
        assert_eq!(w.faults()[0].root().description, message);
        assert_eq!(w.as_slice(), &[8, 0, 0]);
        let mut w = Writer::new();
        Nbt::String("\u{20AC}".repeat(21846)).write(&mut w);
        assert_eq!(w.faults().len(), 1);
        assert!(w.faults()[0].root().description.ends_with("(\u{20AC}\u{20AC}\u{20AC}\u{20AC}\u{20AC}\u{20AC}\u{20AC}\u{20AC}...\u{20AC}\u{20AC}\u{20AC}\u{20AC}\u{20AC}\u{20AC}\u{20AC}\u{20AC}) too long: 65538 bytes"));
    }

    fn fault_frames(tag: &Nbt) -> (Vec<&'static str>, Vec<&'static str>) {
        let mut w = Writer::new();
        w.layer(&[Frame::composite(2, 1)], |w| w.layer(crate::stream_codec::TAG_CODEC, |w| tag.write(w)));
        let fault = &w.faults()[0];
        (fault.frames.clone(), fault.root().frames.clone())
    }

    const COMMON: [&str; 3] = [
        "net.minecraft.network.codec.ByteBufCodecs$18.encode(ByteBufCodecs.java:309)",
        "net.minecraft.network.codec.ByteBufCodecs$18.encode(ByteBufCodecs.java:294)",
        "net.minecraft.network.codec.StreamCodec$8.encode(StreamCodec.java:162)",
    ];

    #[test]
    fn faults_in_a_root_string_carry_the_write_nbt_frames() {
        let (outer, root) = fault_frames(&Nbt::String("a".repeat(70000)));
        assert_eq!(outer[0], "net.minecraft.network.FriendlyByteBuf.writeNbt(FriendlyByteBuf.java:461)");
        assert_eq!(outer[1..], COMMON[..3]);
        assert_eq!(
            root,
            [
                "java.base/java.io.DataOutputStream.writeUTF(DataOutputStream.java:370)",
                "java.base/java.io.DataOutputStream.writeUTF(DataOutputStream.java:338)",
                "io.netty.buffer.ByteBufOutputStream.writeUTF(ByteBufOutputStream.java:160)",
                "net.minecraft.nbt.StringTag.write(StringTag.java:76)",
                "net.minecraft.nbt.NbtIo.writeAnyTag(NbtIo.java:156)",
                "net.minecraft.network.FriendlyByteBuf.writeNbt(FriendlyByteBuf.java:459)",
                COMMON[0],
                COMMON[1],
                COMMON[2],
            ]
        );
    }

    #[test]
    fn faults_in_nested_tags_carry_compound_and_list_frames() {
        let compound = |key: &str, value: Nbt| Nbt::Compound(vec![(key.into(), value)].into());
        let long = || Nbt::String("a".repeat(70000));
        let nested = compound("a", Nbt::List(vec![compound("b", long())]));
        let (_, root) = fault_frames(&nested);
        assert_eq!(
            root[3..12],
            [
                "net.minecraft.nbt.StringTag.write(StringTag.java:76)",
                "net.minecraft.nbt.CompoundTag.writeNamedTag(CompoundTag.java:506)",
                "net.minecraft.nbt.CompoundTag.write(CompoundTag.java:208)",
                "net.minecraft.nbt.ListTag.write(ListTag.java:193)",
                "net.minecraft.nbt.CompoundTag.writeNamedTag(CompoundTag.java:506)",
                "net.minecraft.nbt.CompoundTag.write(CompoundTag.java:208)",
                "net.minecraft.nbt.NbtIo.writeAnyTag(NbtIo.java:156)",
                "net.minecraft.network.FriendlyByteBuf.writeNbt(FriendlyByteBuf.java:459)",
                COMMON[0],
            ]
        );
        let (_, root) = fault_frames(&compound(&"k".repeat(70000), Nbt::Byte(1)));
        assert_eq!(
            root[3..6],
            [
                "net.minecraft.nbt.CompoundTag.writeNamedTag(CompoundTag.java:504)",
                "net.minecraft.nbt.CompoundTag.write(CompoundTag.java:208)",
                "net.minecraft.nbt.NbtIo.writeAnyTag(NbtIo.java:156)",
            ]
        );
        let mixed = Nbt::List(vec![long(), Nbt::Byte(1)]);
        let (_, root) = fault_frames(&mixed);
        assert_eq!(root[3..7].len(), 4);
        assert_eq!(root[4], "net.minecraft.nbt.CompoundTag.writeNamedTag(CompoundTag.java:506)");
        assert_eq!(root[5], "net.minecraft.nbt.CompoundTag.write(CompoundTag.java:208)");
        assert_eq!(root[6], "net.minecraft.nbt.ListTag.write(ListTag.java:193)");
    }
}
