use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{Typed, find_field};
use crate::util::datafix::fixes::references::{ENTITY, ITEM_STACK, PLAYER};

const ID_MAP: [(&str, &str); 40] = [
    ("736565d2-e1a7-403d-a3f8-1aeb3e302542", "minecraft:creative_mode_block_range"),
    ("98491ef6-97b1-4584-ae82-71a8cc85cf73", "minecraft:creative_mode_entity_range"),
    ("91AEAA56-376B-4498-935B-2F7F68070635", "minecraft:effect.speed"),
    ("7107DE5E-7CE8-4030-940E-514C1F160890", "minecraft:effect.slowness"),
    ("AF8B6E3F-3328-4C0A-AA36-5BA2BB9DBEF3", "minecraft:effect.haste"),
    ("55FCED67-E92A-486E-9800-B47F202C4386", "minecraft:effect.mining_fatigue"),
    ("648D7064-6A60-4F59-8ABE-C2C23A6DD7A9", "minecraft:effect.strength"),
    ("C0105BF3-AEF8-46B0-9EBC-92943757CCBE", "minecraft:effect.jump_boost"),
    ("22653B89-116E-49DC-9B6B-9971489B5BE5", "minecraft:effect.weakness"),
    ("5D6F0BA2-1186-46AC-B896-C61C5CEE99CC", "minecraft:effect.health_boost"),
    ("EAE29CF0-701E-4ED6-883A-96F798F3DAB5", "minecraft:effect.absorption"),
    ("03C3C89D-7037-4B42-869F-B146BCB64D2E", "minecraft:effect.luck"),
    ("CC5AF142-2BD2-4215-B636-2605AED11727", "minecraft:effect.unluck"),
    ("6555be74-63b3-41f1-a245-77833b3c2562", "minecraft:evil"),
    ("1eaf83ff-7207-4596-b37a-d7a07b3ec4ce", "minecraft:powder_snow"),
    ("662A6B8D-DA3E-4C1C-8813-96EA6097278D", "minecraft:sprinting"),
    ("020E0DFB-87AE-4653-9556-831010E291A0", "minecraft:attacking"),
    ("766bfa64-11f3-11ea-8d71-362b9e155667", "minecraft:baby"),
    ("7E0292F2-9434-48D5-A29F-9583AF7DF27F", "minecraft:covered"),
    ("9e362924-01de-4ddd-a2b2-d0f7a405a174", "minecraft:suffocating"),
    ("5CD17E52-A79A-43D3-A529-90FDE04B181E", "minecraft:drinking"),
    ("B9766B59-9566-4402-BC1F-2EE2A276D836", "minecraft:baby"),
    ("49455A49-7EC5-45BA-B886-3B90B23A1718", "minecraft:attacking"),
    ("845DB27C-C624-495F-8C9F-6020A9A58B6B", "minecraft:armor.boots"),
    ("D8499B04-0E66-4726-AB29-64469D734E0D", "minecraft:armor.leggings"),
    ("9F3D476D-C118-4544-8365-64846904B48E", "minecraft:armor.chestplate"),
    ("2AD3F246-FEE1-4E67-B886-69FD380BB150", "minecraft:armor.helmet"),
    ("C1C72771-8B8E-BA4A-ACE0-81A93C8928B2", "minecraft:armor.body"),
    ("b572ecd2-ac0c-4071-abde-9594af072a37", "minecraft:enchantment.fire_protection"),
    ("40a9968f-5c66-4e2f-b7f4-2ec2f4b3e450", "minecraft:enchantment.blast_protection"),
    ("07a65791-f64d-4e79-86c7-f83932f007ec", "minecraft:enchantment.respiration"),
    ("60b1b7db-fffd-4ad0-817c-d6c6a93d8a45", "minecraft:enchantment.aqua_affinity"),
    ("11dc269a-4476-46c0-aff3-9e17d7eb6801", "minecraft:enchantment.depth_strider"),
    ("87f46a96-686f-4796-b035-22e16ee9e038", "minecraft:enchantment.soul_speed"),
    ("b9716dbd-50df-4080-850e-70347d24e687", "minecraft:enchantment.soul_speed"),
    ("92437d00-c3a7-4f2e-8f6c-1f21585d5dd0", "minecraft:enchantment.swift_sneak"),
    ("5d3d087b-debe-4037-b53e-d84f3ff51f17", "minecraft:enchantment.sweeping_edge"),
    ("3ceb37c0-db62-46b5-bd02-785457b01d96", "minecraft:enchantment.efficiency"),
    ("CB3F55D3-645C-4F38-A497-9C13A33DB5CF", "minecraft:base_attack_damage"),
    ("FA233E1C-4180-4865-B01B-BCCE9785ACA3", "minecraft:base_attack_speed"),
];

const NAME_MAP: [(&str, &str); 5] = [
    ("Random spawn bonus", "minecraft:random_spawn_bonus"),
    ("Random zombie-spawn bonus", "minecraft:zombie_random_spawn_bonus"),
    ("Leader zombie bonus", "minecraft:leader_zombie_bonus"),
    ("Zombie reinforcement callee charge", "minecraft:reinforcement_callee_charge"),
    ("Zombie reinforcement caller charge", "minecraft:reinforcement_caller_charge"),
];

pub struct AttributeModifierIdFix {
    base: DataFixBase,
}

impl AttributeModifierIdFix {
    pub fn new(output_schema: &Arc<Schema>) -> AttributeModifierIdFix {
        AttributeModifierIdFix { base: DataFixBase::new(output_schema, false) }
    }
}

fn uuid_from_string(name: &str) -> (i64, i64) {
    let parts: Vec<u64> = name.split('-').map(|part| u64::from_str_radix(part, 16).expect("a hexadecimal UUID part")).collect();
    let most_significant_bits = (parts[0] << 32) | (parts[1] << 16) | parts[2];
    let least_significant_bits = (parts[3] << 48) | parts[4];
    (most_significant_bits as i64, least_significant_bits as i64)
}

fn id_from_uuid(uuid: (i64, i64)) -> Option<&'static str> {
    ID_MAP.iter().find(|(name, _)| uuid_from_string(name) == uuid).map(|(_, id)| *id)
}

fn uuid_to_string((most_significant_bits, least_significant_bits): (i64, i64)) -> String {
    let (most, least) = (most_significant_bits as u64, least_significant_bits as u64);
    format!("{:08x}-{:04x}-{:04x}-{:04x}-{:012x}", most >> 32, (most >> 16) & 0xFFFF, most & 0xFFFF, least >> 48, least & 0xFFFF_FFFF_FFFF)
}

pub fn uuid_from_int_array(int_array: &[i32]) -> Option<(i64, i64)> {
    if int_array.len() != 4 {
        return None;
    }
    let most_significant_bits = ((int_array[0] as i64) << 32) | (int_array[1] as i64 & 4294967295);
    let least_significant_bits = ((int_array[2] as i64) << 32) | (int_array[3] as i64 & 4294967295);
    Some((most_significant_bits, least_significant_bits))
}

fn put(converted: &mut Vec<(String, Dynamic)>, id: &str, value: Dynamic) {
    match converted.iter_mut().find(|(key, _)| key == id) {
        Some((_, slot)) => *slot = value,
        None => converted.push((id.to_string(), value)),
    }
}

fn fix_modifiers(modifiers: Vec<Dynamic>) -> Vec<Dynamic> {
    let mut converted: Vec<(String, Dynamic)> = Vec::new();
    for mut modifier in modifiers {
        let uuid = uuid_from_int_array(&modifier.get("uuid").as_int_stream());
        let name = modifier.get("name").as_string_or("");
        let id_from_uuid = uuid.and_then(id_from_uuid);
        let id_from_name = NAME_MAP.iter().find(|(from, _)| *from == name).map(|(_, to)| *to);
        if let Some(id_from_uuid) = id_from_uuid {
            let id = modifier.create_string(id_from_uuid);
            modifier = modifier.set("id", id);
            put(&mut converted, id_from_uuid, modifier.remove("uuid").remove("name"));
        } else if let Some(id_from_name) = id_from_name {
            let pre_existing = converted.iter().find(|(key, _)| key == id_from_name).map(|(_, value)| value.clone());
            match pre_existing {
                None => {
                    let id = modifier.create_string(id_from_name);
                    modifier = modifier.set("id", id);
                    put(&mut converted, id_from_name, modifier.remove("uuid").remove("name"));
                }
                Some(pre_existing) => {
                    let amount = pre_existing.get("amount").as_double(0.0);
                    let added = modifier.get("amount").as_double(0.0);
                    let new_amount = modifier.create_double(amount + added);
                    put(&mut converted, id_from_name, pre_existing.set("amount", new_amount));
                }
            }
        } else {
            let id = format!("minecraft:{}", uuid.map_or_else(|| "unknown".to_string(), uuid_to_string));
            let id_value = modifier.create_string(id.clone());
            modifier = modifier.set("id", id_value);
            put(&mut converted, &id, modifier.remove("uuid").remove("name"));
        }
    }
    converted.into_iter().map(|(_, value)| value).collect()
}

fn convert_modifier_for_entity(modifier: Dynamic) -> Dynamic {
    modifier.rename_field("UUID", "uuid").rename_field("Name", "name").rename_field("Amount", "amount").rename_and_fix_field("Operation", "operation", |operation| {
        operation.create_string(match operation.as_int(0) {
            0 => "add_value",
            1 => "add_multiplied_base",
            2 => "add_multiplied_total",
            _ => "invalid",
        })
    })
}

fn fix_item_stack_components(components: Dynamic) -> Dynamic {
    components.update("minecraft:attribute_modifiers", |attribute_modifiers| {
        attribute_modifiers.update("modifiers", |modifiers| match modifiers.as_stream_opt().into_result() {
            Ok(entries) => modifiers.create_list(fix_modifiers(entries)),
            Err(_) => modifiers,
        })
    })
}

fn fix_attribute(attribute: Dynamic) -> Dynamic {
    attribute.rename_field("Name", "id").rename_field("Base", "base").rename_and_fix_field("Modifiers", "modifiers", |modifiers| match modifiers.as_stream_opt().into_result() {
        Ok(entries) => {
            let converted = entries.into_iter().map(convert_modifier_for_entity).collect();
            modifiers.create_list(fix_modifiers(converted))
        }
        Err(_) => modifiers,
    })
}

fn fix_entity(entity: Typed) -> Typed {
    entity.update_remainder(|tag| {
        tag.rename_and_fix_field("Attributes", "attributes", |attribute_list| match attribute_list.as_stream_opt().into_result() {
            Ok(entries) => attribute_list.create_list(entries.into_iter().map(fix_attribute)),
            Err(_) => attribute_list,
        })
    })
}

impl DataFix for AttributeModifierIdFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let item_stack_type = self.get_input_schema().get_type(ITEM_STACK);
        let components_finder = find_field(&item_stack_type, "components");
        TypeRewriteRule::seq(vec![
            self.fix_type_everywhere_typed("AttributeIdFix (ItemStack)", item_stack_type, move |item_stack| {
                item_stack.update_typed(&components_finder, None, |components| components.update_remainder(fix_item_stack_components))
            }),
            self.fix_type_everywhere_typed("AttributeIdFix (Entity)", self.get_input_schema().get_type(ENTITY), fix_entity),
            self.fix_type_everywhere_typed("AttributeIdFix (Player)", self.get_input_schema().get_type(PLAYER), fix_entity),
        ])
    }
}
