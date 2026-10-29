use wafer_protocol::nbt::Nbt;

use crate::util::codec::{self, DecodeResult, Record};

#[derive(Clone, Debug, PartialEq)]
pub struct WorldOptions {
    seed: i64,
    generate_structures: bool,
    generate_bonus_chest: bool,
    legacy_custom_options: Option<String>,
}

impl WorldOptions {
    pub fn new(seed: i64, generate_structures: bool, generate_bonus_chest: bool) -> WorldOptions {
        WorldOptions { seed, generate_structures, generate_bonus_chest, legacy_custom_options: None }
    }

    pub fn demo_options() -> WorldOptions {
        WorldOptions::new("North Carolina".encode_utf16().fold(0i32, |hash, unit| hash.wrapping_mul(31).wrapping_add(unit as i32)) as i64, true, true)
    }

    pub fn default_with_random_seed() -> WorldOptions {
        WorldOptions::new(WorldOptions::random_seed(), true, false)
    }

    pub fn random_seed() -> i64 {
        rand::random()
    }

    pub fn seed(&self) -> i64 {
        self.seed
    }

    pub fn generate_structures(&self) -> bool {
        self.generate_structures
    }

    pub fn generate_bonus_chest(&self) -> bool {
        self.generate_bonus_chest
    }

    pub fn with_bonus_chest(&self, generate_bonus_chest: bool) -> WorldOptions {
        WorldOptions { generate_bonus_chest, ..self.clone() }
    }

    pub fn decode(tag: &Nbt) -> DecodeResult<WorldOptions> {
        let mut record = Record::default();
        let seed = record.required(tag, "seed", |seed| codec::long(seed));
        let generate_structures = record.with_default(tag, "generate_structures", true, |generate_structures| codec::bool_value(generate_structures));
        let generate_bonus_chest = record.with_default(tag, "bonus_chest", false, |bonus_chest| codec::bool_value(bonus_chest));
        let legacy_custom_options = tag.get("legacy_custom_options").and_then(|options| codec::string(options).ok()).map(str::to_string);
        record.build(|| Some(WorldOptions { seed: seed?, generate_structures, generate_bonus_chest, legacy_custom_options }))
    }

    pub fn encode(&self, fields: &mut Vec<(String, Nbt)>) {
        fields.push(("seed".into(), Nbt::Long(self.seed)));
        fields.push(("generate_structures".into(), Nbt::Byte(self.generate_structures as i8)));
        fields.push(("bonus_chest".into(), Nbt::Byte(self.generate_bonus_chest as i8)));
        if let Some(options) = &self.legacy_custom_options {
            fields.push(("legacy_custom_options".into(), Nbt::String(options.clone())));
        }
    }
}
