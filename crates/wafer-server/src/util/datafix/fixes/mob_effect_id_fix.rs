use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dsl::{ConstType, TypeReference};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{OpticFinder, Typed, field_finder, find_field, named_choice};
use crate::datafixers::types::{Type, const_type, named};
use crate::util::datafix::fixes::references::{BLOCK_ENTITY, ENTITY, ITEM_NAME, ITEM_STACK, PLAYER};

const MOB_EFFECT_INSTANCE_CARRIER_ITEMS: [&str; 4] = ["minecraft:potion", "minecraft:splash_potion", "minecraft:lingering_potion", "minecraft:tipped_arrow"];

fn id_map(id: i32) -> Option<&'static str> {
    Some(match id {
        1 => "minecraft:speed",
        2 => "minecraft:slowness",
        3 => "minecraft:haste",
        4 => "minecraft:mining_fatigue",
        5 => "minecraft:strength",
        6 => "minecraft:instant_health",
        7 => "minecraft:instant_damage",
        8 => "minecraft:jump_boost",
        9 => "minecraft:nausea",
        10 => "minecraft:regeneration",
        11 => "minecraft:resistance",
        12 => "minecraft:fire_resistance",
        13 => "minecraft:water_breathing",
        14 => "minecraft:invisibility",
        15 => "minecraft:blindness",
        16 => "minecraft:night_vision",
        17 => "minecraft:hunger",
        18 => "minecraft:weakness",
        19 => "minecraft:poison",
        20 => "minecraft:wither",
        21 => "minecraft:health_boost",
        22 => "minecraft:absorption",
        23 => "minecraft:saturation",
        24 => "minecraft:glowing",
        25 => "minecraft:levitation",
        26 => "minecraft:luck",
        27 => "minecraft:unluck",
        28 => "minecraft:slow_falling",
        29 => "minecraft:conduit_power",
        30 => "minecraft:dolphins_grace",
        31 => "minecraft:bad_omen",
        32 => "minecraft:hero_of_the_village",
        33 => "minecraft:darkness",
        _ => return None,
    })
}

pub struct MobEffectIdFix {
    base: DataFixBase,
}

impl MobEffectIdFix {
    pub fn new(output_schema: &Arc<Schema>) -> MobEffectIdFix {
        MobEffectIdFix { base: DataFixBase::new(output_schema, false) }
    }

    fn named_choice_updater(&self, type_reference: TypeReference, name: &str) -> NamedChoiceUpdater {
        let old_type = self.get_input_schema().get_choice_type(type_reference, name);
        let new_type = self.get_output_schema().get_choice_type(type_reference, name);
        NamedChoiceUpdater { finder: named_choice(name, old_type), new_type }
    }

    fn block_entity_fixer(&self) -> TypeRewriteRule {
        let block_entity_type = self.get_input_schema().get_type(BLOCK_ENTITY);
        let beacon = self.named_choice_updater(BLOCK_ENTITY, "minecraft:beacon");
        self.fix_type_everywhere_typed("BlockEntityMobEffectIdFix", block_entity_type, move |input| {
            beacon.update(input, |mut tag| {
                tag = update_mob_effect_id_field_in_place(tag, "Primary", "primary_effect");
                update_mob_effect_id_field_in_place(tag, "Secondary", "secondary_effect")
            })
        })
    }

    fn entity_fixer(&self) -> TypeRewriteRule {
        let entity_type = self.get_input_schema().get_type(ENTITY);
        let mooshroom = self.named_choice_updater(ENTITY, "minecraft:mooshroom");
        let arrow = self.named_choice_updater(ENTITY, "minecraft:arrow");
        let area_effect_cloud = self.named_choice_updater(ENTITY, "minecraft:area_effect_cloud");
        self.fix_type_everywhere_typed("EntityMobEffectIdFix", entity_type, move |mut input| {
            input = mooshroom.update(input, fix_mooshroom_tag);
            input = arrow.update(input, fix_arrow_tag);
            input = area_effect_cloud.update(input, fix_area_effect_cloud_tag);
            input.update_remainder(update_living_entity_tag)
        })
    }

    fn player_fixer(&self) -> TypeRewriteRule {
        let player_type = self.get_input_schema().get_type(PLAYER);
        self.fix_type_everywhere_typed("PlayerMobEffectIdFix", player_type, |input| input.update_remainder(update_living_entity_tag))
    }

    fn item_stack_fixer(&self) -> TypeRewriteRule {
        let id_f = field_finder("id", named(ITEM_NAME.type_name(), const_type(ConstType::NamespacedString)));
        let item_stack_type = self.get_input_schema().get_type(ITEM_STACK);
        let tag_f = find_field(&item_stack_type, "tag");
        self.fix_type_everywhere_typed("ItemStackMobEffectIdFix", item_stack_type, move |input| {
            let id = input.get_optional(&id_f).and_then(|id| id.as_str().map(str::to_string));
            if let Some(id) = id {
                if id == "minecraft:suspicious_stew" {
                    return input.update_typed(&tag_f, None, |item_tag| item_tag.update_remainder(fix_suspicious_stew_tag));
                }
                if MOB_EFFECT_INSTANCE_CARRIER_ITEMS.contains(&id.as_str()) {
                    return input.update_typed(&tag_f, None, |item_tag| item_tag.update_remainder(|tag| update_mob_effect_instance_list(tag, "CustomPotionEffects", "custom_potion_effects")));
                }
            }
            input
        })
    }
}

struct NamedChoiceUpdater {
    finder: OpticFinder,
    new_type: Arc<Type>,
}

impl NamedChoiceUpdater {
    fn update(&self, input: Typed, function: impl Fn(Dynamic) -> Dynamic) -> Typed {
        input.update_typed(&self.finder, Some(&self.new_type), |typed_tag| typed_tag.update_remainder(&function))
    }
}

fn get_and_convert_mob_effect_id(obj: &Dynamic, field_name: &str) -> Option<Dynamic> {
    obj.get(field_name).as_number().into_result().ok().and_then(|id| id_map(id.int_value())).map(|id| obj.create_string(id))
}

fn update_mob_effect_id_field(input: &Dynamic, old_field_name: &str, output: Dynamic, new_field_name: &str) -> Dynamic {
    let mapped_id = get_and_convert_mob_effect_id(input, old_field_name);
    output.replace_field(old_field_name, new_field_name, mapped_id)
}

fn update_mob_effect_id_field_in_place(input: Dynamic, old_field_name: &str, new_field_name: &str) -> Dynamic {
    let output = input.clone();
    update_mob_effect_id_field(&input, old_field_name, output, new_field_name)
}

fn update_mob_effect_instance(mut input: Dynamic) -> Dynamic {
    input = update_mob_effect_id_field_in_place(input, "Id", "id");
    input = input.rename_field("Ambient", "ambient");
    input = input.rename_field("Amplifier", "amplifier");
    input = input.rename_field("Duration", "duration");
    input = input.rename_field("ShowParticles", "show_particles");
    input = input.rename_field("ShowIcon", "show_icon");
    let hidden_effect = input.get("HiddenEffect").result().map(update_mob_effect_instance);
    input.replace_field("HiddenEffect", "hidden_effect", hidden_effect)
}

fn update_mob_effect_instance_list(input: Dynamic, old_field: &str, new_field: &str) -> Dynamic {
    let new_value = input.get(old_field).as_stream_opt().into_result().ok().map(|effects| input.create_list(effects.into_iter().map(update_mob_effect_instance)));
    input.replace_field(old_field, new_field, new_value)
}

fn update_suspicious_stew_entry_into(input: &Dynamic, mut output: Dynamic) -> Dynamic {
    output = update_mob_effect_id_field(input, "EffectId", output, "id");
    let duration = input.get("EffectDuration").result();
    output.replace_field("EffectDuration", "duration", duration)
}

fn update_suspicious_stew_entry(input: Dynamic) -> Dynamic {
    let output = input.clone();
    update_suspicious_stew_entry_into(&input, output)
}

fn fix_mooshroom_tag(mut entity_tag: Dynamic) -> Dynamic {
    let initial_entry = entity_tag.empty_map();
    let entry = update_suspicious_stew_entry_into(&entity_tag, initial_entry.clone());
    if entry != initial_entry {
        let stew_effects = entity_tag.create_list([entry]);
        entity_tag = entity_tag.set("stew_effects", stew_effects);
    }
    entity_tag.remove("EffectId").remove("EffectDuration")
}

fn fix_arrow_tag(data: Dynamic) -> Dynamic {
    update_mob_effect_instance_list(data, "CustomPotionEffects", "custom_potion_effects")
}

fn fix_area_effect_cloud_tag(data: Dynamic) -> Dynamic {
    update_mob_effect_instance_list(data, "Effects", "effects")
}

fn update_living_entity_tag(data: Dynamic) -> Dynamic {
    update_mob_effect_instance_list(data, "ActiveEffects", "active_effects")
}

fn fix_suspicious_stew_tag(tag: Dynamic) -> Dynamic {
    let effects_list = tag.get("Effects").as_stream_opt().into_result().ok().map(|list| tag.create_list(list.into_iter().map(update_suspicious_stew_entry)));
    tag.replace_field("Effects", "effects", effects_list)
}

impl DataFix for MobEffectIdFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        TypeRewriteRule::seq(vec![self.block_entity_fixer(), self.entity_fixer(), self.player_fixer(), self.item_stack_fixer()])
    }
}
