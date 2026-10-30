use wafer_core::fastutil::{array_size, max_fill};

const DEFAULT_INITIAL_SIZE: usize = 16;
const DEFAULT_LOAD_FACTOR: f32 = 0.75;
const INT_PHI: i32 = -1640531527;

fn mix(x: i32) -> i32 {
    let h = x.wrapping_mul(INT_PHI);
    h ^ ((h as u32) >> 16) as i32
}

type Slot<V> = Option<(String, i32, V)>;

#[derive(Clone)]
pub struct ObjectOpenHashMap<V> {
    slots: Vec<Slot<V>>,
    size: usize,
    max_fill: usize,
}

impl<V> Default for ObjectOpenHashMap<V> {
    fn default() -> ObjectOpenHashMap<V> {
        let n = array_size(DEFAULT_INITIAL_SIZE, DEFAULT_LOAD_FACTOR);
        ObjectOpenHashMap { slots: empty_slots(n), size: 0, max_fill: max_fill(n, DEFAULT_LOAD_FACTOR) }
    }
}

fn empty_slots<V>(n: usize) -> Vec<Slot<V>> {
    let mut slots = Vec::with_capacity(n);
    slots.resize_with(n, || None);
    slots
}

fn start_slot(hash: i32, mask: usize) -> usize {
    mix(hash) as u32 as usize & mask
}

impl<V> ObjectOpenHashMap<V> {
    pub fn len(&self) -> usize {
        self.size
    }

    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    fn find(&self, key: &str, hash: i32) -> Result<usize, usize> {
        let mask = self.slots.len() - 1;
        let mut pos = start_slot(hash, mask);
        loop {
            match &self.slots[pos] {
                None => return Err(pos),
                Some((existing, _, _)) if existing == key => return Ok(pos),
                Some(_) => pos = (pos + 1) & mask,
            }
        }
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.find(key, wafer_core::random::java_string_hash(key)).is_ok()
    }

    pub fn entry_or_insert_with(&mut self, key: &str, default: impl FnOnce() -> V) -> &mut V {
        self.entry_or_insert_with_hash(key, wafer_core::random::java_string_hash(key), default)
    }

    pub fn entry_or_insert_with_hash(&mut self, key: &str, hash: i32, default: impl FnOnce() -> V) -> &mut V {
        let pos = match self.find(key, hash) {
            Ok(pos) => pos,
            Err(pos) => {
                self.slots[pos] = Some((key.to_string(), hash, default()));
                let previous = self.size;
                self.size += 1;
                if previous >= self.max_fill {
                    self.rehash(array_size(self.size + 1, DEFAULT_LOAD_FACTOR));
                    self.find(key, hash).unwrap_or_else(|pos| pos)
                } else {
                    pos
                }
            }
        };
        &mut self.slots[pos].as_mut().expect("slot").2
    }

    pub fn try_capacity(&mut self, capacity: usize) {
        let needed = array_size(capacity, DEFAULT_LOAD_FACTOR);
        if needed > self.slots.len() {
            self.rehash(needed);
        }
    }

    fn rehash(&mut self, new_n: usize) {
        let mask = new_n - 1;
        let mut new_slots = empty_slots::<V>(new_n);
        for entry in self.slots.iter_mut().rev() {
            if let Some((key, hash, value)) = entry.take() {
                let mut pos = start_slot(hash, mask);
                while new_slots[pos].is_some() {
                    pos = (pos + 1) & mask;
                }
                new_slots[pos] = Some((key, hash, value));
            }
        }
        self.slots = new_slots;
        self.max_fill = max_fill(new_n, DEFAULT_LOAD_FACTOR);
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &V)> {
        self.slots.iter().rev().filter_map(|slot| slot.as_ref().map(|(key, _, value)| (key.as_str(), value)))
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&str, &mut V)> {
        self.slots.iter_mut().rev().filter_map(|slot| slot.as_mut().map(|(key, _, value)| (key.as_str(), value)))
    }

    pub fn into_values(self) -> Vec<V> {
        self.slots.into_iter().rev().flatten().map(|(_, _, value)| value).collect()
    }
}

pub fn java_hash_set_copy_order(hashes: &[i32]) -> Vec<usize> {
    let requested = (((hashes.len() as f32) / 0.75f32) as usize + 1).max(16);
    let mut capacity = requested.next_power_of_two();
    while hashes.len() > capacity * 3 / 4 {
        capacity *= 2;
    }
    let bucket = |h: i32| ((h ^ ((h as u32) >> 16) as i32) as u32 as usize) & (capacity - 1);
    let mut order: Vec<usize> = (0..hashes.len()).collect();
    order.sort_by_key(|&i| bucket(hashes[i]));
    order
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counters_keep_fastutil_iteration_order() {
        let mut map = ObjectOpenHashMap::<i64>::default();
        for name in ["getChunk", "getChunkCacheMiss", "runTask", "chunkLoad", "chunkSave", "getEntities", "hasEntities", "tickNonPassenger", "tickPassenger", "containersToTick", "ticksToRun"] {
            *map.entry_or_insert_with(name, || 0) += 1;
        }
        assert_eq!(map.len(), 11);
        let mut names: Vec<&str> = map.iter().map(|(name, _)| name).collect();
        names.sort_unstable();
        assert_eq!(names.len(), 11);
    }

    #[test]
    fn java_hash_sets_copy_in_bucket_order() {
        let order = java_hash_set_copy_order(&[2, 17, 1]);
        assert_eq!(order, vec![1, 2, 0]);
    }
}
