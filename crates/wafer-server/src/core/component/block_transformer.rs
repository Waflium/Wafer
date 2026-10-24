use std::sync::LazyLock;

use wafer_core::{BlockPos, Direction};
use wafer_data::properties::{CHEST_TYPE, ChestType};
use wafer_data::EquipmentSlot;
use wafer_protocol::nbt::Nbt;
use serde_json::Value;

use crate::server::level::{ServerLevel, SoundSource};
use crate::sounds::sound_event::SoundEvent;
use crate::world::entity::EntityId;
use crate::world::item::component as dc;
use crate::world::item::component::json_to_nbt;
use crate::world::item::component::types::{self, Pair, def, opt, record, req};
use crate::world::level::levelgen::feature::stateproviders::block_state_provider;
use crate::world::item::context::BlockPlaceContext;
use crate::world::item::item_utils::has_infinite_materials;
use crate::world::level::BlockGetter;
use crate::world::level::block::chest_block::get_connected_direction;
use crate::world::level::levelgen::feature::stateproviders::block_state_provider::BlockStateProviderHolder;
use crate::world::level::update_flags::UPDATE_ALL_IMMEDIATE;
use crate::world::{InteractionHand, InteractionResult};

fn block_transform_data_codec() -> Pair {
    record(vec![
        req("block_state_provider", block_state_provider::holder_codec()),
        opt("sound", types::sound_event()),
        def("particle", types::enumeration(&["none", "scrape", "wax_on", "wax_off"]), types::s("none")),
        def("disallowed_faces", types::list(types::enumeration(&["down", "up", "north", "south", "west", "east"])), types::empty_list()),
        opt("loot", types::identifier()),
        def("drop_strategy", types::enumeration(&["clicked_face", "from_middle"]), types::s("from_middle")),
        def("update_from_neighbors", types::bool(), types::b(true)),
        def("transform_type", types::enumeration(&["single_block", "copper_chest"]), types::s("single_block")),
        def("consume_on_use", types::bool(), types::b(true)),
        def("item_damage_per_use", types::var_int(types::non_negative_int()), types::i(0)),
    ])
}

pub fn direct_codec() -> Pair {
    types::list_sized(block_transform_data_codec(), 1, 200)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DropStrategy {
    ClickedFace,
    FromMiddle,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TransformParticle {
    None,
    Scrape,
    WaxOn,
    WaxOff,
}

impl TransformParticle {
    fn get_level_event(self) -> i32 {
        match self {
            TransformParticle::None => 0,
            TransformParticle::Scrape => 3005,
            TransformParticle::WaxOn => 3003,
            TransformParticle::WaxOff => 3004,
        }
    }

    fn send(self, level: &mut ServerLevel, player: Option<EntityId>, pos: BlockPos) {
        if self != TransformParticle::None {
            level.level_event(player, self.get_level_event(), pos, 0);
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TransformType {
    SingleBlock,
    CopperChest,
}

pub struct BlockTransformData {
    block_state_provider: BlockStateProviderHolder,
    sound: Option<SoundEvent>,
    particle: TransformParticle,
    disallowed_faces: Vec<Direction>,
    loot: Option<String>,
    drop_strategy: DropStrategy,
    update_from_neighbors: bool,
    transform_type: TransformType,
    consume_on_use: bool,
    item_damage_per_use: i32,
}

impl BlockTransformData {
    fn parse(json: &Value) -> Option<BlockTransformData> {
        let string = |key: &str| json.get(key).and_then(Value::as_str);
        let flag = |key: &str, default: bool| json.get(key).and_then(Value::as_bool).unwrap_or(default);
        Some(BlockTransformData {
            block_state_provider: BlockStateProviderHolder::parse(json.get("block_state_provider")?).ok()?,
            sound: match json.get("sound") {
                Some(sound) => Some(SoundEvent::from_nbt(&json_to_nbt(sound))?),
                None => None,
            },
            particle: match string("particle") {
                Some("scrape") => TransformParticle::Scrape,
                Some("wax_on") => TransformParticle::WaxOn,
                Some("wax_off") => TransformParticle::WaxOff,
                _ => TransformParticle::None,
            },
            disallowed_faces: json
                .get("disallowed_faces")
                .and_then(Value::as_array)
                .map(|faces| faces.iter().filter_map(|face| face.as_str().and_then(Direction::by_name)).collect())
                .unwrap_or_default(),
            loot: string("loot").map(str::to_string),
            drop_strategy: if string("drop_strategy") == Some("clicked_face") { DropStrategy::ClickedFace } else { DropStrategy::FromMiddle },
            update_from_neighbors: flag("update_from_neighbors", true),
            transform_type: if string("transform_type") == Some("copper_chest") { TransformType::CopperChest } else { TransformType::SingleBlock },
            consume_on_use: flag("consume_on_use", true),
            item_damage_per_use: json.get("item_damage_per_use").and_then(Value::as_i64).unwrap_or(0) as i32,
        })
    }
}

pub struct BlockTransformer {
    transforms: Vec<BlockTransformData>,
}

static BLOCK_TRANSFORMERS: LazyLock<Vec<(&'static str, BlockTransformer)>> = LazyLock::new(|| {
    crate::core::data_registries::json_entries("minecraft:block_transformer")
        .iter()
        .filter_map(|&(name, json)| {
            let value: Value = serde_json::from_str(json).ok()?;
            let transforms = value.as_array()?.iter().map(BlockTransformData::parse).collect::<Option<Vec<_>>>()?;
            Some((name, BlockTransformer { transforms }))
        })
        .collect()
});

impl BlockTransformer {
    pub fn get(component: &Nbt) -> Option<&'static BlockTransformer> {
        let name = crate::commands::arguments::parse_identifier(component.as_str()?)?;
        BLOCK_TRANSFORMERS.iter().find(|(n, _)| *n == name).map(|(_, transformer)| transformer)
    }

    fn player_has_blocking_item_use_intent(level: &ServerLevel, player: EntityId, hand: InteractionHand) -> bool {
        hand == InteractionHand::MainHand
            && level.player(player).item_in_hand(InteractionHand::OffHand).is_some_and(|offhand| offhand.has(dc::BLOCKS_ATTACKS))
            && !level.player(player).is_secondary_use_active()
    }

    pub fn transform_block(&self, level: &mut ServerLevel, ctx: &BlockPlaceContext) -> InteractionResult {
        let player = ctx.player.as_ref().map(|p| p.id);
        if let Some(player) = player
            && Self::player_has_blocking_item_use_intent(level, player, ctx.hand)
        {
            return InteractionResult::Pass;
        }
        let pos = ctx.hit.block_pos;
        for transform_data in &self.transforms {
            let clicked_face = ctx.clicked_face();
            if transform_data.disallowed_faces.contains(&clicked_face) {
                continue;
            }
            let mut random = level.random.clone();
            let new_block_state = transform_data.block_state_provider.value().get_optional_state(level, &mut random, pos);
            level.random = random;
            let Some(new_block_state) = new_block_state else { continue };
            let updated_shape = if transform_data.update_from_neighbors { level.update_from_neighbour_shapes(new_block_state, pos) } else { new_block_state };
            let mut item_in_hand = ctx.item.clone();
            if let Some(player) = player {
                crate::advancements::triggers::criteria_triggers::ITEM_USED_ON_BLOCK.trigger(level, player, pos, &item_in_hand);
            }
            let old_block_state = level.block_state(pos);
            if let Some(loot) = &transform_data.loot {
                let block_entity = level.get_block_entity(pos).cloned();
                let drop_strategy = transform_data.drop_strategy;
                crate::world::level::block::drop_from_block_interact_loot_table(
                    level,
                    loot,
                    pos,
                    old_block_state,
                    block_entity.as_ref(),
                    Some(&item_in_hand.clone()),
                    player,
                    |level, stack| match drop_strategy {
                        DropStrategy::ClickedFace => level.pop_resource_from_face(pos, clicked_face, stack),
                        DropStrategy::FromMiddle => level.pop_resource(pos, stack),
                    },
                );
            }
            if let Some(player) = player {
                if item_in_hand.is_stackable() {
                    if !has_infinite_materials(level, player) {
                        item_in_hand.shrink(if transform_data.consume_on_use { 1 } else { 0 });
                        level.player_mut(player).set_item_in_hand(ctx.hand, Some(item_in_hand));
                    }
                } else {
                    let slot = if ctx.hand == InteractionHand::MainHand { EquipmentSlot::Mainhand } else { EquipmentSlot::Offhand };
                    level.hurt_and_break(player, slot, transform_data.item_damage_per_use);
                }
            }
            level.set_block(pos, updated_shape, UPDATE_ALL_IMMEDIATE);
            if let Some(sound) = &transform_data.sound {
                level.play_sound(player, pos, sound.clone(), SoundSource::Blocks, 1.0, 1.0);
            }
            transform_data.particle.send(level, player, pos);
            level.game_event_at_with(crate::world::level::gameevent::game_event::BLOCK_CHANGE, pos, crate::world::level::gameevent::game_event::Context::of_entity_and_state(player, Some(updated_shape)));
            if transform_data.transform_type == TransformType::CopperChest
                && old_block_state.block().is_instance_of("CopperChestBlock")
                && old_block_state.get(CHEST_TYPE) != ChestType::Single
            {
                let neighbor_pos = pos.relative(get_connected_direction(old_block_state));
                let neighbor_state = level.block_state(neighbor_pos);
                level.game_event_at_with(crate::world::level::gameevent::game_event::BLOCK_CHANGE, neighbor_pos, crate::world::level::gameevent::game_event::Context::of_entity_and_state(player, Some(neighbor_state)));
                transform_data.particle.send(level, player, neighbor_pos);
            }
            return InteractionResult::SUCCESS;
        }
        InteractionResult::Pass
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::BlockTransformData;
    use crate::sounds::sound_event::SoundEvent;

    #[test]
    fn sounds_are_registered_or_direct() {
        let data = |sound: serde_json::Value| BlockTransformData::parse(&json!({"block_state_provider": {"type": "minecraft:simple", "state": {"id": "minecraft:stone"}}, "sound": sound}));
        let registered = data(json!("minecraft:item.axe.strip")).unwrap();
        assert!(matches!(registered.sound, Some(SoundEvent::Reference(_))));
        let direct = data(json!({"sound_id": "t:custom_sound", "range": 24.0})).unwrap();
        assert_eq!(direct.sound, Some(SoundEvent::Direct { location: "t:custom_sound".to_string(), fixed_range: Some(24.0) }));
        let without_range = data(json!({"sound_id": "t:custom_sound"})).unwrap();
        assert_eq!(without_range.sound, Some(SoundEvent::Direct { location: "t:custom_sound".to_string(), fixed_range: None }));
        assert!(data(json!("t:nope")).is_none());
    }
}
