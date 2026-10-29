use wafer_core::random::RandomSource;
use wafer_protocol::nbt::Nbt;

use super::gossip_type::GossipType;
use crate::util::codec::{self, DecodeResult, Record};
use crate::util::java_collections::JavaHashCode;
use crate::world::entity::entity_save::decode_uuid;
use crate::world::level::storage::uuid_ints;

pub const DISCARD_THRESHOLD: i32 = 2;

#[derive(Clone, Debug, Default)]
struct EntityGossips {
    entries: Vec<(GossipType, i32)>,
}

impl EntityGossips {
    fn get(&self, ty: GossipType) -> i32 {
        self.entries.iter().find(|(t, _)| *t == ty).map_or(0, |(_, v)| *v)
    }

    fn put(&mut self, ty: GossipType, value: i32) {
        match self.entries.iter_mut().find(|(t, _)| *t == ty) {
            Some((_, v)) => *v = value,
            None => self.entries.push((ty, value)),
        }
    }

    fn merge(&mut self, ty: GossipType, value: i32, merge: impl FnOnce(i32, i32) -> i32) {
        match self.entries.iter_mut().find(|(t, _)| *t == ty) {
            Some((_, v)) => *v = merge(*v, value),
            None => self.entries.push((ty, value)),
        }
    }

    fn weighted_value(&self, types: impl Fn(GossipType) -> bool) -> i32 {
        self.entries.iter().filter(|(ty, _)| types(*ty)).map(|(ty, value)| value * ty.weight()).sum()
    }

    fn decay(&mut self) {
        self.entries.retain_mut(|(ty, value)| {
            let new_value = *value - ty.decay_per_day();
            if new_value < DISCARD_THRESHOLD {
                false
            } else {
                *value = new_value;
                true
            }
        });
    }

    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn make_sure_value_isnt_too_low_or_too_high(&mut self, ty: GossipType) {
        let value = self.get(ty);
        if value > ty.max() {
            self.put(ty, ty.max());
        }
        if value < DISCARD_THRESHOLD {
            self.remove(ty);
        }
    }

    fn remove(&mut self, ty: GossipType) {
        self.entries.retain(|(t, _)| *t != ty);
    }
}

#[derive(Clone, Copy, Debug)]
struct GossipEntry {
    target: u128,
    ty: GossipType,
    value: i32,
}

impl GossipEntry {
    fn weighted_value(&self) -> i32 {
        self.value * self.ty.weight()
    }
}

#[derive(Clone, Debug)]
pub struct GossipContainer {
    gossips: Vec<(u128, EntityGossips)>,
    capacity: usize,
}

impl Default for GossipContainer {
    fn default() -> GossipContainer {
        GossipContainer { gossips: Vec::new(), capacity: 16 }
    }
}

impl GossipContainer {
    fn bucket(&self, target: u128) -> usize {
        let h = target.java_hash_code();
        (h ^ ((h as u32) >> 16) as i32) as u32 as usize & (self.capacity - 1)
    }

    fn ordered(&self) -> Vec<usize> {
        let mut order: Vec<usize> = (0..self.gossips.len()).collect();
        order.sort_by_key(|&i| self.bucket(self.gossips[i].0));
        order
    }

    fn get(&self, target: u128) -> Option<&EntityGossips> {
        self.gossips.iter().find(|(uuid, _)| *uuid == target).map(|(_, gossips)| gossips)
    }

    fn get_or_create(&mut self, target: u128) -> &mut EntityGossips {
        let index = match self.gossips.iter().position(|(uuid, _)| *uuid == target) {
            Some(index) => index,
            None => {
                self.gossips.push((target, EntityGossips::default()));
                if self.gossips.len() > self.capacity * 3 / 4 {
                    self.capacity *= 2;
                }
                self.gossips.len() - 1
            }
        };
        &mut self.gossips[index].1
    }

    fn remove_target(&mut self, target: u128) {
        self.gossips.retain(|(uuid, _)| *uuid != target);
    }

    pub fn decay(&mut self) {
        for (_, gossips) in &mut self.gossips {
            gossips.decay();
        }
        self.gossips.retain(|(_, gossips)| !gossips.is_empty());
    }

    fn unpack(&self) -> Vec<GossipEntry> {
        self.ordered()
            .into_iter()
            .flat_map(|i| {
                let (target, gossips) = &self.gossips[i];
                gossips.entries.iter().map(move |&(ty, value)| GossipEntry { target: *target, ty, value })
            })
            .collect()
    }

    fn select_gossips_for_transfer(&self, random: &mut impl RandomSource, max_count: i32) -> Vec<GossipEntry> {
        let entries = self.unpack();
        if entries.is_empty() {
            return Vec::new();
        }
        let mut ranges = vec![0; entries.len()];
        let mut ranges_end = 0;
        for (i, gossip) in entries.iter().enumerate() {
            ranges_end += gossip.weighted_value().abs();
            ranges[i] = ranges_end - 1;
        }
        let mut results: Vec<usize> = Vec::new();
        for _ in 0..max_count {
            let choice = random.next_int_bounded(ranges_end);
            let selected_index = match ranges.binary_search(&choice) {
                Ok(index) => index,
                Err(index) => index,
            };
            if !results.contains(&selected_index) {
                results.push(selected_index);
            }
        }
        results.into_iter().map(|i| entries[i]).collect()
    }

    pub fn transfer_from(&mut self, source: &GossipContainer, random: &mut impl RandomSource, max_count: i32) -> i32 {
        let new_gossips = source.select_gossips_for_transfer(random, max_count);
        for new_gossip in &new_gossips {
            let decayed_value = new_gossip.value - new_gossip.ty.decay_per_transfer();
            if decayed_value >= DISCARD_THRESHOLD {
                self.get_or_create(new_gossip.target).merge(new_gossip.ty, decayed_value, i32::max);
            }
        }
        new_gossips.len() as i32
    }

    pub fn get_reputation(&self, entity: u128, types: impl Fn(GossipType) -> bool) -> i32 {
        self.get(entity).map_or(0, |entry| entry.weighted_value(types))
    }

    pub fn get_count_for_type(&self, ty: GossipType, value_test: impl Fn(f64) -> bool) -> i64 {
        self.gossips.iter().filter(|(_, e)| value_test((e.get(ty) * ty.weight()) as f64)).count() as i64
    }

    pub fn add(&mut self, target: u128, ty: GossipType, amount_to_add: i32) {
        let entity_gossips = self.get_or_create(target);
        entity_gossips.merge(ty, amount_to_add, |o, n| {
            let sum = o + n;
            if sum > ty.max() { ty.max().max(o) } else { sum }
        });
        entity_gossips.make_sure_value_isnt_too_low_or_too_high(ty);
        if entity_gossips.is_empty() {
            self.remove_target(target);
        }
    }

    pub fn remove_amount(&mut self, target: u128, ty: GossipType, amount_to_remove: i32) {
        self.add(target, ty, -amount_to_remove);
    }

    pub fn remove(&mut self, target: u128, ty: GossipType) {
        if let Some((_, entity_gossips)) = self.gossips.iter_mut().find(|(uuid, _)| *uuid == target) {
            entity_gossips.remove(ty);
            if entity_gossips.is_empty() {
                self.remove_target(target);
            }
        }
    }

    pub fn remove_type(&mut self, ty: GossipType) {
        for (_, gossips) in &mut self.gossips {
            gossips.remove(ty);
        }
        self.gossips.retain(|(_, gossips)| !gossips.is_empty());
    }

    pub fn clear(&mut self) {
        self.gossips.clear();
    }

    pub fn put_all(&mut self, container: &GossipContainer) {
        for i in container.ordered() {
            let (target, gossips) = &container.gossips[i];
            let own = self.get_or_create(*target);
            for &(ty, value) in &gossips.entries {
                own.put(ty, value);
            }
        }
    }

    pub fn copy(&self) -> GossipContainer {
        let mut container = GossipContainer::default();
        container.put_all(self);
        container
    }

    pub fn to_nbt(&self) -> Nbt {
        Nbt::List(
            self.unpack()
                .into_iter()
                .map(|entry| {
                    Nbt::Compound(vec![
                        ("Target".into(), uuid_ints(entry.target)),
                        ("Type".into(), Nbt::String(entry.ty.id().into())),
                        ("Value".into(), Nbt::Int(entry.value)),
                    ])
                })
                .collect(),
        )
    }

    pub fn decode(tag: &Nbt) -> DecodeResult<GossipContainer> {
        codec::list_partial(tag, GossipContainer::decode_entry).map(|entries| {
            let mut container = GossipContainer::default();
            for (target, ty, value) in entries {
                container.get_or_create(target).put(ty, value);
            }
            container
        })
    }

    fn decode_entry(tag: &Nbt) -> DecodeResult<(u128, GossipType, i32)> {
        if let Err(error) = codec::map(tag) {
            return DecodeResult::error(error);
        }
        let mut record = Record::default();
        let target = record.required(tag, "Target", decode_uuid);
        let ty = record.required(tag, "Type", |ty| codec::enum_by_name(ty, GossipType::by_id));
        let value = record.required(tag, "Value", |value| {
            let value = codec::int(value)?;
            if value < 1 { Err(format!("Value must be positive: {value}")) } else { Ok(value) }
        });
        record.build(|| Some((target?, ty?, value?)))
    }
}
