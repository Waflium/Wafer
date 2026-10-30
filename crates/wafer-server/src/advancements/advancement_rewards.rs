use wafer_core::random::RandomSource;
use wafer_data::registry;
use wafer_protocol::nbt::Nbt;

use crate::advancements::triggers::criterion_trigger::CriterionTrigger;
use crate::core::holder::Holder;
use crate::server::level::{ServerLevel, SoundSource};
use crate::util::codec::{self, DataResult, DecodeResult, Record};
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::crafting::recipe_holder::RecipeHolder;
use crate::world::item::crafting::recipe_manager::RecipeManager;
use crate::world::level::storage::loot::loot_params::LootParams;
use crate::world::level::storage::loot::loot_table::LootTable;

#[derive(Default)]
pub struct AdvancementRewards {
    pub experience: i32,
    pub loot: Vec<Holder<LootTable>>,
    pub recipes: Vec<String>,
    pub function: Option<String>,
}

impl AdvancementRewards {
    pub fn decode(tag: &Nbt) -> DataResult<AdvancementRewards> {
        AdvancementRewards::decode_partial(tag).into_result()
    }

    pub fn decode_partial(tag: &Nbt) -> DecodeResult<AdvancementRewards> {
        codec::as_codec(|| {
            if let Err(error) = codec::map(tag) {
                return DecodeResult::error(error);
            }
            let mut record = Record::default();
            let experience = record.with_default(tag, "experience", 0, |experience| DecodeResult::from(codec::int(experience)));
            let loot = record.with_default(tag, "loot", Vec::new(), |loot| DecodeResult::from(LootTable::decode_list(loot)));
            let recipes = record.with_default(tag, "recipes", Vec::new(), |recipes| codec::list_partial(recipes, |recipe| DecodeResult::from(codec::identifier(recipe))));
            let function = record.optional(tag, "function", |function| DecodeResult::from(codec::identifier(function)));
            record.build(|| Some(AdvancementRewards { experience, loot, recipes, function }))
        })
    }

    pub fn grant(&self, level: &mut ServerLevel, player: EntityId, trigger: CriterionTrigger) {
        level.give_experience_points(player, self.experience);
        let mut changes = false;
        for loot_table in &self.loot {
            let origin = level.player(player).entity.position();
            let mut params = LootParams::new(level);
            params.this_entity = Some(player);
            params.origin = Some(origin);
            for mut item_stack in loot_table.value().get_random_items(params) {
                let p = level.player_mut(player);
                let infinite_materials = p.abilities.instabuild;
                if p.inventory.add_stack(&mut item_stack, infinite_materials).0 {
                    let p = level.player_mut(player);
                    let (x, y, z) = (p.entity.x(), p.entity.y(), p.entity.z());
                    let pitch = ((p.entity.random.next_float() - p.entity.random.next_float()) * 0.7 + 1.0) * 2.0;
                    let sound = wafer_data::registry_id(registry::SOUND_EVENT, "minecraft:entity.item.pickup");
                    level.play_sound_at(None, x, y, z, sound, SoundSource::Players, 0.2, pitch);
                    changes = true;
                } else if !item_stack.is_empty() {
                    level.swing(player, crate::world::InteractionHand::MainHand, crate::world::item::component::swing_animation::SwingAnimation::DEFAULT, true);
                    let mut drop = level.create_item_stack_to_drop(player, item_stack, false, false);
                    drop.set_no_pickup_delay();
                    drop.target = Some(level.player(player).profile.uuid);
                    level.add_fresh_entity(LevelEntity::Item(drop));
                }
            }
        }
        if changes {
            level.broadcast_container_menu_changes(player);
        }
        if !self.recipes.is_empty() {
            let recipe_manager = RecipeManager::get();
            let recipes: Vec<&'static RecipeHolder> = self.recipes.iter().filter_map(|id| recipe_manager.by_key(id)).collect();
            level.award_recipes(player, &recipes);
        }
        if let Some(function) = &self.function {
            level.pending_function_executions.push((function.clone(), player, trigger));
        }
    }
}
