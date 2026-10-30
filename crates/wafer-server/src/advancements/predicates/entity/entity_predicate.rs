use wafer_core::Vec3;
use wafer_protocol::nbt::Nbt;

use super::entity_sub_predicate::EntitySubPredicate;
use super::{
    cube_mob_predicate, distance_to_player_predicate, entity_effects_predicate, entity_equipment_predicate, entity_exact_data_components_predicate,
    entity_flags_predicate, entity_location_predicate, entity_nbt_predicate, entity_partial_components_predicate, entity_slots_predicate, entity_tag_predicate,
    entity_type_predicate, fishing_hook_predicate, lightning_bolt_predicate, movement_affected_by_predicate, movement_predicate, passenger_predicate,
    periodic_entity_tick_predicate, player_predicate, raider_predicate, sheep_predicate, stepping_on_predicate, targeted_entity_predicate, team_predicate,
    vehicle_predicate,
};
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::item::component::codec::Wrap;
use crate::world::item::component::types::{self, Pair};
use crate::world::entity::EntityId;
use crate::world::level::storage::loot::loot_context::{LootContext, LootContextBuilder};
use crate::world::level::storage::loot::loot_params::LootParams;

fn sub_predicate_codec(id: &str) -> Option<Pair> {
    Some(match id {
        "minecraft:entity_type" => entity_type_predicate::codec(),
        "minecraft:location" => entity_location_predicate::codec(),
        "minecraft:stepping_on" => stepping_on_predicate::codec(),
        "minecraft:movement_affected_by" => movement_affected_by_predicate::codec(),
        "minecraft:distance" => distance_to_player_predicate::codec(),
        "minecraft:movement" => movement_predicate::codec(),
        "minecraft:effects" => entity_effects_predicate::codec(),
        "minecraft:nbt" => entity_nbt_predicate::codec(),
        "minecraft:flags" => entity_flags_predicate::codec(),
        "minecraft:equipment" => entity_equipment_predicate::codec(),
        "minecraft:periodic_tick" => periodic_entity_tick_predicate::codec(),
        "minecraft:vehicle" => vehicle_predicate::codec(),
        "minecraft:passenger" => passenger_predicate::codec(),
        "minecraft:targeted_entity" => targeted_entity_predicate::codec(),
        "minecraft:team" => team_predicate::codec(),
        "minecraft:slots" => entity_slots_predicate::codec(),
        "minecraft:components" => entity_exact_data_components_predicate::codec(),
        "minecraft:predicates" => entity_partial_components_predicate::codec(),
        "minecraft:entity_tags" => entity_tag_predicate::codec(),
        "minecraft:type_specific/lightning" => lightning_bolt_predicate::codec(),
        "minecraft:type_specific/fishing_hook" => fishing_hook_predicate::codec(),
        "minecraft:type_specific/player" => player_predicate::codec(),
        "minecraft:type_specific/cube_mob" => cube_mob_predicate::codec(),
        "minecraft:type_specific/raider" => raider_predicate::codec(),
        "minecraft:type_specific/sheep" => sheep_predicate::codec(),
        _ => return None,
    })
}

fn decode_parts(tag: Nbt) -> Result<Nbt, String> {
    let entries = codec::map(&tag)?;
    let mut parts = Vec::with_capacity(entries.len());
    for (key, value) in entries {
        let id = crate::resources::identifier::read(key)?;
        let sub_codec = sub_predicate_codec(&id)
            .ok_or_else(|| format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:entity_sub_predicate_type]: {id}"))?;
        parts.push((id, sub_codec.0.decode(value)?));
    }
    Ok(Nbt::Compound(parts.into()))
}

pub fn codec() -> Pair {
    types::transform(decode_parts)
}

#[derive(Debug)]
pub struct EntityPredicate {
    combined_part: Vec<EntitySubPredicate>,
}

impl EntityPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<EntityPredicate> {
        let mut parts: Vec<EntitySubPredicate> = codec::wrapped(Wrap::Xmap, || {
            codec::dispatched_map(tag, |key| EntitySubPredicate::codec(&crate::resources::identifier::read(key)?), |decode, value| decode(value))
        })?
        .into_iter()
        .map(|(_, part)| part)
        .collect();
        if parts.len() > 1 {
            parts.sort_by_key(EntitySubPredicate::predicate_type_order);
        }
        Ok(EntityPredicate { combined_part: parts })
    }

    pub fn matches(&self, level: &mut ServerLevel, position: Option<Vec3>, entity: Option<EntityId>) -> bool {
        let Some(entity) = entity else { return false };
        self.combined_part.iter().all(|part| part.matches(level, entity, position))
    }

    pub fn matches_player(&self, level: &mut ServerLevel, player: EntityId, entity: Option<EntityId>) -> bool {
        let position = level.entity(player).position();
        self.matches(level, Some(position), entity)
    }

    pub fn create_context(level: &mut ServerLevel, player: EntityId, entity: EntityId) -> LootContext<'_> {
        let origin = level.entity(player).position();
        let mut params = LootParams::new(level);
        params.this_entity = Some(entity);
        params.origin = Some(origin);
        LootContextBuilder::new(params).create(None)
    }
}
