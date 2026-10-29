use wafer_core::RandomSource;
use serde_json::Value;
use wafer_protocol::nbt::Nbt;

use crate::world::item::component::codec::{self, Codec, Field, Stream};
use crate::world::item::component::types::{self, Pair, req};

#[derive(Clone, PartialEq, Debug)]
pub struct WeightedList<T> {
    pub items: Vec<(T, i32)>,
    total_weight: i32,
}

impl<T> WeightedList<T> {
    pub fn new(items: Vec<(T, i32)>) -> WeightedList<T> {
        let total_weight = items.iter().map(|(_, w)| *w).sum();
        WeightedList { items, total_weight }
    }

    pub const fn empty() -> WeightedList<T> {
        WeightedList { items: Vec::new(), total_weight: 0 }
    }

    pub fn is_empty(&self) -> bool {
        self.total_weight == 0
    }

    pub fn contains(&self, value: &T) -> bool
    where
        T: PartialEq,
    {
        self.items.iter().any(|(item, _)| item == value)
    }


    pub fn get_random<R: RandomSource + ?Sized>(&self, random: &mut R) -> Option<&T> {
        if self.total_weight == 0 {
            return None;
        }
        let mut selection = random.next_int_bounded(self.total_weight);
        for (item, weight) in &self.items {
            if selection < *weight {
                return Some(item);
            }
            selection -= weight;
        }
        unreachable!("selection within the total weight")
    }

    pub fn get_random_or_throw<R: RandomSource + ?Sized>(&self, random: &mut R) -> &T {
        self.get_random(random).expect("Weighted list has no elements")
    }

    pub fn parse(json: &Value, mut data: impl FnMut(&Value) -> Result<T, String>) -> Result<WeightedList<T>, String> {
        let mut items = Vec::new();
        for entry in json.as_array().ok_or("a weighted list is a list")? {
            let weight = entry.get("weight").and_then(Value::as_i64).ok_or("weighted entry without weight")? as i32;
            items.push((data(entry.get("data").ok_or("weighted entry without data")?)?, weight));
        }
        Ok(WeightedList::new(items))
    }
}

pub const SUM_OF_WEIGHTS_EXCEPTION: &str = "java.lang.IllegalArgumentException: Sum of weights must be <= 2147483647";
const SUM_OF_WEIGHTS_MESSAGE: &str = "Sum of weights must be <= 2147483647";

const SUM_OF_WEIGHTS_FRAMES: &[&str] = &[
    "net.minecraft.util.random.WeightedRandom.getTotalWeight(WeightedRandom.java:21)",
    "net.minecraft.util.random.WeightedList.<init>(WeightedList.java:29)",
    "net.minecraft.util.random.WeightedList.of(WeightedList.java:61)",
    "com.mojang.datafixers.util.Pair.mapFirst(Pair.java:64)",
    "com.mojang.serialization.Decoder$2.lambda$decode$0(Decoder.java:63)",
    "com.mojang.serialization.DataResult$Success.map(DataResult.java:175)",
    "com.mojang.serialization.Decoder$2.decode(Decoder.java:63)",
    "com.mojang.serialization.Codec$2.decode(Codec.java:74)",
    "com.mojang.serialization.Decoder$3.decode(Decoder.java:77)",
    "com.mojang.serialization.Codec$2.decode(Codec.java:74)",
    "com.mojang.serialization.Decoder.parse(Decoder.java:18)",
];

fn total_weight(list: &Nbt) -> i64 {
    list.as_list().iter().filter_map(|entry| entry.get("weight")).map(codec::as_i64).sum()
}

fn limit_total_weight(list: Nbt) -> Result<Nbt, String> {
    if total_weight(&list) > i32::MAX as i64 {
        crate::core::data_registries::throw("java.lang.IllegalArgumentException", SUM_OF_WEIGHTS_MESSAGE.into(), SUM_OF_WEIGHTS_FRAMES);
        Err(SUM_OF_WEIGHTS_EXCEPTION.into())
    } else {
        Ok(list)
    }
}

fn non_zero_total_weight(list: Nbt) -> Result<Nbt, String> {
    if total_weight(&list) == 0 { Err("Weighted list must contain at least one entry with non-zero weight".into()) } else { Ok(list) }
}

pub fn weighted_codec(mut element_fields: Vec<Field>) -> Pair {
    element_fields.push(req("weight", types::var_int(types::non_negative_int())));
    types::record(element_fields)
}

pub fn weighted_data_codec(data: Pair) -> Pair {
    weighted_codec(vec![req("data", data)])
}

pub fn weighted_list_codec(weighted: Pair) -> Pair {
    let stream = Stream::List(Box::new(weighted.1), i32::MAX as usize);
    (Codec::Normalize(Box::new(Codec::list(weighted.0)), limit_total_weight), stream)
}

pub fn non_empty_weighted_list_codec(weighted: Pair) -> Pair {
    let list = weighted_list_codec(weighted);
    (Codec::Normalize(Box::new(list.0), non_zero_total_weight), list.1)
}
