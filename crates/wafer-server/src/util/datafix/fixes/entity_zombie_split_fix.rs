use std::sync::Arc;

use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::Typed;
use crate::datafixers::types::Type;
use crate::util::datafix::fixes::entity_rename_fix::EntityRenameFix;
use crate::util::datafix::fixes::references::ENTITY;

pub struct EntityZombieSplitFix;

impl EntityZombieSplitFix {
    pub fn new(output_schema: &Arc<Schema>) -> EntityRenameFix {
        let output = output_schema.clone();
        let zombie_villager_type = std::sync::OnceLock::new();
        EntityRenameFix::new("EntityZombieSplitFix", output_schema, true, move |name, entity| {
            if name != "Zombie" {
                return (name.to_string(), entity);
            }
            let tag = entity.get_optional_remainder().expect("a remainder");
            let ty = tag.get("ZombieType").as_int(0);
            let (new_name, new_entity) = match ty {
                1..=5 => ("ZombieVillager", change_schema_to_zombie_villager(&entity, zombie_villager_type.get_or_init(|| output.get_choice_type(ENTITY, "ZombieVillager")), ty - 1)),
                6 => ("Husk", entity),
                _ => ("Zombie", entity),
            };
            (new_name.to_string(), new_entity.update_remainder(|e| e.remove("ZombieType")))
        })
    }
}

fn change_schema_to_zombie_villager(entity: &Typed, zombie_villager_type: &Arc<Type>, profession: i32) -> Typed {
    crate::util::write_and_read_typed_or_throw(entity, zombie_villager_type, |serialized_entity| {
        let value = serialized_entity.create_int(profession);
        serialized_entity.set("Profession", value)
    })
}
