use wafer_core::RandomSource;
use serde_json::Value;

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
