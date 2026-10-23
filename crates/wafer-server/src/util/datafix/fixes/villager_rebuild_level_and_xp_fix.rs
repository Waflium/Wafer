use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{Typed, find_field, named_choice, type_finder};
use crate::datafixers::types::Type;
use crate::util::datafix::fixes::references::ENTITY;

const LEVEL_XP_THRESHOLDS: [i32; 5] = [0, 10, 50, 100, 150];

pub fn get_min_xp_per_level(level: i32) -> i32 {
    LEVEL_XP_THRESHOLDS[(level - 1).clamp(0, LEVEL_XP_THRESHOLDS.len() as i32 - 1) as usize]
}

pub struct VillagerRebuildLevelAndXpFix {
    base: DataFixBase,
}

impl VillagerRebuildLevelAndXpFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> VillagerRebuildLevelAndXpFix {
        VillagerRebuildLevelAndXpFix { base: DataFixBase::new(output_schema, changes_type) }
    }
}

impl DataFix for VillagerRebuildLevelAndXpFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let villager_type = self.get_input_schema().get_choice_type(ENTITY, "minecraft:villager");
        let entity_f = named_choice("minecraft:villager", villager_type.clone());
        let offers_f = find_field(&villager_type, "Offers");
        let offers_type = offers_f.ty();
        let recipe_list_f = find_field(&offers_type, "Recipes");
        let recipe_f = match &*recipe_list_f.ty() {
            Type::List(element) => type_finder(element.clone()),
            other => panic!("Expecting recipes to be a list, got {other:?}"),
        };
        self.fix_type_everywhere_typed("Villager level and xp rebuild", self.get_input_schema().get_type(ENTITY), move |input| {
            input.update_typed(&entity_f, Some(&villager_type), |villager| {
                let remainder = villager.get_remainder();
                let mut level = remainder.get("VillagerData").get("level").as_int(0);
                let mut modified_villager = villager.clone();
                if level == 0 || level == 1 {
                    let offer_count = villager
                        .get_optional_typed(&offers_f)
                        .and_then(|offers| offers.get_optional_typed(&recipe_list_f))
                        .map(|recipe_list| recipe_list.get_all_typed(&recipe_f).len())
                        .unwrap_or(0);
                    level = (offer_count as i32 / 2).clamp(1, 5);
                    if level > 1 {
                        modified_villager = add_level(modified_villager, level);
                    }
                }
                if remainder.get("Xp").as_number().into_result().is_err() {
                    modified_villager = add_xp_from_level(modified_villager, level);
                }
                modified_villager
            })
        })
    }
}

fn add_level(villager: Typed, level: i32) -> Typed {
    villager.update_remainder(|remainder| {
        remainder.update("VillagerData", |villager_data| {
            let level = villager_data.create_int(level);
            villager_data.set("level", level)
        })
    })
}

fn add_xp_from_level(villager: Typed, level: i32) -> Typed {
    let xp = get_min_xp_per_level(level);
    villager.update_remainder(|remainder| {
        let xp = remainder.create_int(xp);
        remainder.set("Xp", xp)
    })
}
