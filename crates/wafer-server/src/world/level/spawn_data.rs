use wafer_core::BlockPos;
use wafer_protocol::nbt::{Nbt, TagMap};

use super::light_layer::LightLayer;
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult, DecodeResult, Record};
use crate::util::inclusive_range::InclusiveRange;
use crate::world::entity::equipment_table::EquipmentTable;

pub const ENTITY_TAG: &str = "entity";

#[derive(Clone, PartialEq, Debug)]
pub struct SpawnData {
    entity_to_spawn: Nbt,
    custom_spawn_rules: Option<CustomSpawnRules>,
    equipment: Option<EquipmentTable>,
}

impl Default for SpawnData {
    fn default() -> SpawnData {
        SpawnData::new(TagMap::new(), None, None)
    }
}

impl SpawnData {
    pub fn new(mut entity_to_spawn: TagMap, custom_spawn_rules: Option<CustomSpawnRules>, equipment: Option<EquipmentTable>) -> SpawnData {
        let id = entity_to_spawn.iter().find(|(key, _)| key == "id").and_then(|(_, id)| codec::identifier(id).ok());
        match id {
            Some(id) => entity_to_spawn.put("id", Nbt::String(id)),
            None => {
                entity_to_spawn.remove_key("id");
            }
        }
        SpawnData { entity_to_spawn: Nbt::Compound(entity_to_spawn), custom_spawn_rules, equipment }
    }

    pub fn get_entity_to_spawn(&self) -> &Nbt {
        &self.entity_to_spawn
    }

    pub fn get_entity_to_spawn_mut(&mut self) -> &mut TagMap {
        let Nbt::Compound(fields) = &mut self.entity_to_spawn else { unreachable!("the entity to spawn is a compound") };
        fields
    }

    pub fn get_custom_spawn_rules(&self) -> Option<&CustomSpawnRules> {
        self.custom_spawn_rules.as_ref()
    }

    pub fn get_equipment(&self) -> Option<&EquipmentTable> {
        self.equipment.as_ref()
    }

    pub fn decode(tag: &Nbt) -> DataResult<SpawnData> {
        SpawnData::decode_partial(tag).into_result()
    }

    pub fn decode_partial(tag: &Nbt) -> DecodeResult<SpawnData> {
        if let Err(error) = codec::map(tag) {
            return DecodeResult::error(error);
        }
        codec::as_codec(|| {
            let mut record = Record::default();
            let entity_to_spawn = record.required(tag, ENTITY_TAG, codec::compound_tag);
            let custom_spawn_rules = record.optional(tag, "custom_spawn_rules", CustomSpawnRules::decode_partial);
            let equipment = record.optional(tag, "equipment", EquipmentTable::decode_partial);
            record.build(|| match entity_to_spawn? {
                Nbt::Compound(fields) => Some(SpawnData::new(fields, custom_spawn_rules, equipment)),
                _ => None,
            })
        })
    }

    pub fn encode(&self) -> Nbt {
        let mut fields = vec![(ENTITY_TAG.to_string(), self.entity_to_spawn.copy())];
        if let Some(custom_spawn_rules) = &self.custom_spawn_rules {
            fields.push(("custom_spawn_rules".into(), custom_spawn_rules.encode()));
        }
        if let Some(equipment) = &self.equipment {
            fields.push(("equipment".into(), equipment.encode()));
        }
        Nbt::Compound(fields.into())
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct CustomSpawnRules {
    pub block_light_limit: InclusiveRange,
    pub sky_light_limit: InclusiveRange,
}

const LIGHT_RANGE: InclusiveRange = InclusiveRange::new(0, 15);

impl CustomSpawnRules {
    fn check_light_boundaries(range: InclusiveRange) -> DataResult<InclusiveRange> {
        if LIGHT_RANGE.contains(&range) { Ok(range) } else { Err(format!("Light values must be withing range {LIGHT_RANGE}")) }
    }

    fn light_limit(record: &mut Record, tag: &Nbt, name: &str) -> Option<InclusiveRange> {
        let range = codec::optional_field(tag, name).and_then(|value| InclusiveRange::decode_int(value).ok()).unwrap_or(LIGHT_RANGE);
        record.field(CustomSpawnRules::check_light_boundaries(range))
    }

    pub fn decode(tag: &Nbt) -> DataResult<CustomSpawnRules> {
        CustomSpawnRules::decode_partial(tag).into_result()
    }

    pub fn decode_partial(tag: &Nbt) -> DecodeResult<CustomSpawnRules> {
        if let Err(error) = codec::map(tag) {
            return DecodeResult::error(error);
        }
        let mut record = Record::default();
        let block_light_limit = CustomSpawnRules::light_limit(&mut record, tag, "block_light_limit");
        let sky_light_limit = CustomSpawnRules::light_limit(&mut record, tag, "sky_light_limit");
        record.build(|| Some(CustomSpawnRules { block_light_limit: block_light_limit?, sky_light_limit: sky_light_limit? }))
    }

    pub fn encode(&self) -> Nbt {
        let mut fields = Vec::new();
        if self.block_light_limit != LIGHT_RANGE {
            fields.push(("block_light_limit".to_string(), self.block_light_limit.encode_int()));
        }
        if self.sky_light_limit != LIGHT_RANGE {
            fields.push(("sky_light_limit".to_string(), self.sky_light_limit.encode_int()));
        }
        Nbt::Compound(fields.into())
    }

    pub fn is_valid_position(&self, block_spawn_pos: BlockPos, level: &mut ServerLevel) -> bool {
        self.block_light_limit.is_value_in_range(level.brightness(LightLayer::Block, block_spawn_pos))
            && self.sky_light_limit.is_value_in_range(level.get_effective_sky_brightness(block_spawn_pos))
    }
}
