use wafer_core::random::java_string_hash;

fn mix(x: i32) -> i32 {
    let h = x.wrapping_mul(0x9E37_79B9u32 as i32);
    h ^ ((h as u32) >> 16) as i32
}

fn array_size(expected: usize) -> usize {
    ((expected as f64 / 0.75f32 as f64).ceil() as usize).next_power_of_two().max(2)
}

struct OpenHashMap<'a> {
    hashes: &'a [i32],
    slots: Vec<Option<usize>>,
    mask: usize,
}

impl<'a> OpenHashMap<'a> {
    fn new(expected: usize, hashes: &'a [i32]) -> OpenHashMap<'a> {
        let n = array_size(expected);
        OpenHashMap { hashes, slots: vec![None; n], mask: n - 1 }
    }

    fn put(&mut self, item: usize) {
        let mut pos = mix(self.hashes[item]) as u32 as usize & self.mask;
        while self.slots[pos].is_some() {
            pos = (pos + 1) & self.mask;
        }
        self.slots[pos] = Some(item);
    }

    fn descending(&self) -> impl Iterator<Item = usize> + '_ {
        self.slots.iter().rev().flatten().copied()
    }

    fn ascending(&self) -> impl Iterator<Item = usize> + '_ {
        self.slots.iter().flatten().copied()
    }
}

fn java_hash_map_order(hashes: &[i32]) -> Vec<usize> {
    let mut capacity = 16;
    for len in 1..=hashes.len() {
        if len > capacity * 3 / 4 {
            capacity *= 2;
        }
    }
    let mut order: Vec<usize> = (0..hashes.len()).collect();
    order.sort_by_key(|&item| {
        let h = hashes[item];
        (h ^ ((h as u32) >> 16) as i32) as u32 as usize & (capacity - 1)
    });
    order
}

pub fn point_order(keys: &[&str]) -> Vec<usize> {
    let hashes: Vec<i32> = keys.iter().map(|key| java_string_hash(key)).collect();
    let mut templates = OpenHashMap::new(keys.len(), &hashes);
    for item in java_hash_map_order(&hashes) {
        templates.put(item);
    }
    let mut types = OpenHashMap::new(keys.len(), &hashes);
    for item in templates.descending() {
        types.put(item);
    }
    let mut choice_types = OpenHashMap::new(keys.len(), &hashes);
    for item in types.descending() {
        choice_types.put(item);
    }
    choice_types.ascending().collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::point_order;
    use crate::datafixers::dsl::TypeReference;
    use crate::util::datafix::data_fixers::get_data_fixer;

    #[test]
    #[ignore]
    fn point_order_matches_vanilla() {
        let expected = std::fs::read_to_string(std::env::var("WAFER_CHOICE_ORDER").expect("WAFER_CHOICE_ORDER points at the ChoiceOrderAll output")).unwrap();
        let fixer = get_data_fixer();
        let mut mismatches = 0;
        let mut count = 0;
        let mut by_key: HashMap<(i32, String), Vec<String>> = HashMap::new();
        for line in expected.lines() {
            let mut fields = line.splitn(3, ' ');
            let key: i32 = fields.next().unwrap().parse().unwrap();
            let name = fields.next().unwrap().to_string();
            let keys: Vec<String> = fields.next().unwrap().split(',').map(str::to_string).collect();
            by_key.insert((key, name), keys);
        }
        for ((key, name), vanilla) in &by_key {
            let name: &'static str = Box::leak(name.clone().into_boxed_str());
            let schema = fixer.get_schema(*key);
            if schema.version_key != *key {
                continue;
            }
            let choice_type = schema.find_choice_type(TypeReference(name));
            let choice = choice_type.choices();
            let keys: Vec<&str> = choice.types.iter().map(|(key, _)| key.as_str()).collect();
            let order: Vec<&str> = point_order(&keys).into_iter().map(|item| keys[item]).collect();
            count += 1;
            if order != *vanilla {
                mismatches += 1;
                let position = order.iter().zip(vanilla).position(|(a, b)| a != b).unwrap_or(0);
                eprintln!("{key} {name}: {} keys, first difference at {position}: wafer {:?} vanilla {:?}", keys.len(), &order[position..(position + 4).min(order.len())], &vanilla[position..(position + 4).min(vanilla.len())]);
            }
        }
        eprintln!("{count} choice types, {mismatches} mismatches");
        assert_eq!(mismatches, 0);
    }
}
