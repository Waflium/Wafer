use std::sync::LazyLock;

use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, Vec3, mth};
use wafer_data::{DamageType, MobEffect, registry};
use wafer_protocol::nbt::Nbt;

use crate::core::particles::particle_types;
use crate::util::valueproviders::float_provider_codec;
use crate::world::item::component::codec::{Codec, Dispatch, Field, Selector, Stream};
use crate::world::item::component::custom;
use crate::world::item::component::types::{self, Pair, def, opt, record, req};
use crate::world::item::enchantment::level_based_value;
use crate::world::level::levelgen::blockpredicates::block_predicate;
use crate::world::level::levelgen::feature::stateproviders::block_state_provider;

use crate::world::level::BlockGetter;
use crate::core::holder_set::HolderSet;
use crate::core::particles::particle_options::ParticleOptions;
use crate::world::level::levelgen::blockpredicates::block_predicate::{BlockPredicate, parse_offset};
use crate::world::level::levelgen::feature::stateproviders::block_state_provider::BlockStateProviderHolder;
use crate::world::level::update_flags::UPDATE_ALL;
use crate::world::level::is_in_spawnable_bounds;
use crate::world::level::server_explosion::{ExplosionDamageCalculator, ExplosionInteraction, ExplosionParticleInfo};
use crate::server::level::ServerLevel;
use crate::sounds::sound_event::SoundEvent;
use crate::util::codec::{self, DataResult};
use crate::util::java_round_f;
use crate::util::valueproviders::FloatProvider;
use crate::world::damagesource::DamageSource;
use crate::world::effect::MobEffectInstance;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::enchantment::enchanted_item_in_use::EnchantedItemInUse;
use crate::world::item::enchantment::level_based_value::LevelBasedValue;

fn decode_vec3(tag: Nbt) -> Result<Nbt, String> {
    let values = Codec::list(Codec::Double).decode(&tag)?;
    if values.as_list().len() != 3 {
        return Err("Input is not a list of 3 elements".into());
    }
    Ok(values)
}

fn vec3_codec() -> Pair {
    types::transform(decode_vec3)
}

fn zero_vec3() -> Nbt {
    Nbt::List(vec![Nbt::Double(0.0); 3])
}

fn zero_vec3i() -> Nbt {
    Nbt::IntArray(vec![0; 3])
}

fn position_source_codec() -> Pair {
    record(vec![
        req("type", types::enumeration(&["entity_position", "in_bounding_box"])),
        def("offset", types::float(Codec::float()), types::f(0.0)),
        def("scale", types::float(types::positive_float()), types::f(1.0)),
    ])
}

fn velocity_source_codec() -> Pair {
    record(vec![
        def("movement_scale", types::float(Codec::float()), types::f(0.0)),
        def("base", float_provider_codec(), types::f(0.0)),
    ])
}

fn replace_fields(radius: bool) -> Vec<Field> {
    let level_based = |name: &'static str| req(name, level_based_value::codec());
    let mut fields = Vec::new();
    if radius {
        fields.push(level_based("radius"));
        fields.push(level_based("height"));
    }
    fields.push(def("offset", custom::block_pos(), zero_vec3i()));
    fields.push(opt("predicate", block_predicate::codec()));
    fields.push(req("block_state", block_state_provider::holder_codec()));
    fields.push(opt("trigger_game_event", types::fixed("minecraft:game_event")));
    fields
}

pub fn shared_cases() -> Vec<(&'static str, Vec<Field>)> {
    let level_based = |name: &'static str| req(name, level_based_value::codec());
    let particle = |name: &'static str| req(name, particle_types::dispatch_codec());
    vec![
        (
            "minecraft:apply_mob_effect",
            vec![
                req("to_apply", types::holder_set("minecraft:mob_effect")),
                level_based("min_duration"),
                level_based("max_duration"),
                level_based("min_amplifier"),
                level_based("max_amplifier"),
            ],
        ),
        ("minecraft:change_item_damage", vec![level_based("amount")]),
        (
            "minecraft:damage_entity",
            vec![level_based("min_damage"), level_based("max_damage"), req("damage_type", types::fixed("minecraft:damage_type"))],
        ),
        (
            "minecraft:explode",
            vec![
                def("attribute_to_user", types::bool(), types::b(false)),
                opt("damage_type", types::fixed("minecraft:damage_type")),
                opt("knockback_multiplier", level_based_value::codec()),
                opt("immune_blocks", types::holder_set("minecraft:block")),
                def("offset", vec3_codec(), zero_vec3()),
                level_based("radius"),
                def("create_fire", types::bool(), types::b(false)),
                req("block_interaction", types::enumeration(&["none", "block", "mob", "tnt", "trigger"])),
                particle("small_particle"),
                particle("large_particle"),
                def(
                    "block_particles",
                    types::list(record(vec![
                        req("particle", particle_types::dispatch_codec()),
                        def("scaling", types::float(Codec::float()), types::f(1.0)),
                        def("speed", types::float(Codec::float()), types::f(1.0)),
                        req("weight", types::var_int(types::non_negative_int())),
                    ])),
                    types::empty_list(),
                ),
                req("sound", types::sound_event()),
            ],
        ),
        ("minecraft:ignite", vec![level_based("duration")]),
        ("minecraft:apply_impulse", vec![req("direction", vec3_codec()), req("coordinate_scale", vec3_codec()), level_based("magnitude")]),
        ("minecraft:apply_exhaustion", vec![level_based("amount")]),
        (
            "minecraft:play_sound",
            vec![req("sound", types::compact_list(types::sound_event())), req("volume", float_provider_codec()), req("pitch", float_provider_codec())],
        ),
        ("minecraft:replace_block", replace_fields(false)),
        ("minecraft:replace_disk", replace_fields(true)),
        ("minecraft:run_function", vec![req("function", types::identifier())]),
        (
            "minecraft:set_block_properties",
            vec![
                req("properties", (Codec::Map(Box::new(Codec::string()), Box::new(Codec::string())), Stream::Nbt)),
                def("offset", custom::block_pos(), zero_vec3i()),
                opt("trigger_game_event", types::fixed("minecraft:game_event")),
            ],
        ),
        (
            "minecraft:spawn_particles",
            vec![
                particle("particle"),
                req("horizontal_position", position_source_codec()),
                req("vertical_position", position_source_codec()),
                req("horizontal_velocity", velocity_source_codec()),
                req("vertical_velocity", velocity_source_codec()),
                def("speed", float_provider_codec(), types::f(0.0)),
            ],
        ),
        ("minecraft:summon_entity", vec![req("entity", types::holder_set("minecraft:entity_type")), def("join_team", types::bool(), types::b(false))]),
    ]
}

fn codec_lazy() -> &'static Pair {
    static CODEC: LazyLock<Pair> = LazyLock::new(codec);
    &CODEC
}

pub fn codec() -> Pair {
    let mut cases = shared_cases();
    cases.push(("minecraft:all_of", vec![req("effects", types::list((Codec::Lazy(codec_lazy), Stream::Lazy)))]));
    (Codec::Dispatch(Box::new(Dispatch { key: "type", selector: Selector::Registry("minecraft:enchantment_entity_effect_type"), cases })), Stream::Dispatch)
}

const POST_IMPULSE_CONTEXT_RESET_GRACE_TIME_TICKS: i32 = 10;

fn vec3(tag: &Nbt) -> DataResult<Vec3> {
    let list = codec::list_of(tag, codec::double)?;
    if list.len() != 3 {
        return Err("Input is not a list of 3 elements".to_string());
    }
    Ok(Vec3::new(list[0], list[1], list[2]))
}

#[allow(dead_code)]
pub enum EnchantmentEntityEffect {
    AllOf(Vec<EnchantmentEntityEffect>),
    ApplyMobEffect { to_apply: HolderSet, min_duration: LevelBasedValue, max_duration: LevelBasedValue, min_amplifier: LevelBasedValue, max_amplifier: LevelBasedValue },
    ChangeItemDamage { amount: LevelBasedValue },
    DamageEntity { min_damage: LevelBasedValue, max_damage: LevelBasedValue, damage_type: DamageType },
    Explode {
        attribute_to_user: bool,
        damage_type: Option<DamageType>,
        knockback_multiplier: Option<LevelBasedValue>,
        immune_blocks: Option<HolderSet>,
        offset: Vec3,
        radius: LevelBasedValue,
        create_fire: bool,
        block_interaction: ExplosionInteraction,
        small_particle: ParticleOptions,
        large_particle: ParticleOptions,
        block_particles: Vec<(ExplosionParticleInfo, i32)>,
        sound: SoundEvent,
    },
    Ignite { duration: LevelBasedValue },
    ApplyImpulse { direction: Vec3, coordinate_scale: Vec3, magnitude: LevelBasedValue },
    ApplyExhaustion { amount: LevelBasedValue },
    PlaySound { sound_events: Vec<SoundEvent>, volume: FloatProvider, pitch: FloatProvider },
    ReplaceBlock { offset: (i32, i32, i32), predicate: Option<BlockPredicate>, block_state: BlockStateProviderHolder, trigger_game_event: Option<crate::world::level::gameevent::game_event::GameEvent> },
    ReplaceDisk { radius: LevelBasedValue, height: LevelBasedValue, offset: (i32, i32, i32), predicate: Option<BlockPredicate>, block_state: BlockStateProviderHolder, trigger_game_event: Option<crate::world::level::gameevent::game_event::GameEvent> },
    RunFunction(String),
    SetBlockProperties { properties: Vec<(String, String)>, offset: (i32, i32, i32), trigger_game_event: Option<crate::world::level::gameevent::game_event::GameEvent> },
    SpawnParticles {
        particle: ParticleOptions,
        horizontal_position: PositionSource,
        vertical_position: PositionSource,
        horizontal_velocity: VelocitySource,
        vertical_velocity: VelocitySource,
        speed: FloatProvider,
    },
    SummonEntity { entity_types: HolderSet, join_team: bool },
}

#[derive(Clone, Copy, PartialEq)]
pub enum PositionSourceType {
    EntityPosition,
    BoundingBox,
}

impl PositionSourceType {
    fn get_coordinate(self, position: f64, center: f64, bounding_box_span: f32, random: &mut impl RandomSource) -> f64 {
        match self {
            PositionSourceType::EntityPosition => position,
            PositionSourceType::BoundingBox => center + (random.next_double() - 0.5) * bounding_box_span as f64,
        }
    }
}

pub struct PositionSource {
    kind: PositionSourceType,
    offset: f32,
    scale: f32,
}

impl PositionSource {
    fn decode(tag: &Nbt) -> DataResult<PositionSource> {
        let kind = codec::decode_field(tag, "type", |t| match codec::string(t)? {
            "entity_position" => Ok(PositionSourceType::EntityPosition),
            "in_bounding_box" => Ok(PositionSourceType::BoundingBox),
            other => Err(format!("Unknown element name:{other}")),
        })?;
        let offset = codec::decode_or(tag, "offset", 0.0, codec::float)?;
        let scale = codec::decode_or(tag, "scale", 1.0, |t| {
            let scale = codec::float(t)?;
            if scale > 0.0 { Ok(scale) } else { Err(format!("Value must be positive: {scale}")) }
        })?;
        if kind == PositionSourceType::EntityPosition && scale != 1.0 {
            return Err("Cannot scale an entity position coordinate source".into());
        }
        Ok(PositionSource { kind, offset, scale })
    }

    fn get_coordinate(&self, position: f64, center: f64, bounding_box_span: f32, random: &mut impl RandomSource) -> f64 {
        self.kind.get_coordinate(position, center, bounding_box_span * self.scale, random) + self.offset as f64
    }
}

pub struct VelocitySource {
    movement_scale: f32,
    base: FloatProvider,
}

impl VelocitySource {
    fn decode(tag: &Nbt) -> DataResult<VelocitySource> {
        Ok(VelocitySource {
            movement_scale: codec::decode_or(tag, "movement_scale", 0.0, codec::float)?,
            base: codec::decode_or(tag, "base", FloatProvider::Constant(0.0), |t| Ok(FloatProvider::parse(&codec::to_json(t))?))?,
        })
    }

    fn get_velocity(&self, movement: f64, random: &mut impl RandomSource) -> f64 {
        movement * self.movement_scale as f64 + self.base.sample(random) as f64
    }
}

fn particle_options(tag: &Nbt) -> DataResult<ParticleOptions> {
    let ty = codec::decode_field(tag, "type", codec::identifier)?;
    let Some(&particle_type) = registry::PARTICLE_TYPE.iter().find(|&&name| name == ty) else {
        return Err(format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:particle_type]: {ty}"));
    };
    crate::core::particles::particle_types::parse(particle_type, tag)
}

fn float_provider(tag: &Nbt, min: f32, max: f32) -> DataResult<FloatProvider> {
    let provider = FloatProvider::parse(&codec::to_json(tag))?;
    let (lo, hi) = match provider {
        FloatProvider::Constant(value) => (value, value),
        FloatProvider::Uniform { min_inclusive, max_exclusive } => (min_inclusive, max_exclusive),
        FloatProvider::ClampedNormal { min, max, .. } => (min, max),
        FloatProvider::Trapezoid { min, max, .. } => (min, max),
    };
    if lo < min || hi > max {
        return Err(format!("Value provider too low or high: {lo} [{min}-{max}]"));
    }
    Ok(provider)
}

fn damage_type(tag: &Nbt) -> DataResult<DamageType> {
    let id = codec::identifier(tag)?;
    DamageType::by_name(&id).ok_or_else(|| format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:damage_type]: {id}"))
}

fn explosion_interaction(tag: &Nbt) -> DataResult<ExplosionInteraction> {
    let name = codec::string(tag)?;
    Ok(match name {
        "none" => ExplosionInteraction::None,
        "block" => ExplosionInteraction::Block,
        "mob" => ExplosionInteraction::Mob,
        "tnt" => ExplosionInteraction::Tnt,
        "trigger" => ExplosionInteraction::Trigger,
        other => return Err(format!("Unknown element name:{other}")),
    })
}

fn explosion_particle_info(tag: &Nbt) -> DataResult<(ExplosionParticleInfo, i32)> {
    let info = ExplosionParticleInfo {
        particle: codec::decode_field(tag, "particle", particle_options)?,
        scaling: codec::decode_or(tag, "scaling", 1.0, codec::float)?,
        speed: codec::decode_or(tag, "speed", 1.0, codec::float)?,
    };
    Ok((info, codec::decode_field(tag, "weight", codec::int)?))
}

fn sound_event(tag: &Nbt) -> DataResult<SoundEvent> {
    Ok(match tag {
        Nbt::String(_) => {
            let key = codec::identifier(tag)?;
            match registry::SOUND_EVENT.iter().position(|&name| name == key) {
                Some(id) => SoundEvent::Reference(id as i32),
                None if crate::core::data_registries::is_loading() => return Err(crate::core::data_registries::unknown_reference("minecraft:sound_event", &key)),
                None => return Err(format!("Failed to get element {key} from registry minecraft:sound_event")),
            }
        }
        _ => SoundEvent::Direct {
            location: codec::decode_field(tag, "sound_id", codec::identifier)?,
            fixed_range: tag.get("range").and_then(Nbt::as_f64).map(|range| range as f32),
        },
    })
}

impl EnchantmentEntityEffect {
    pub fn decode(tag: &Nbt) -> DataResult<EnchantmentEntityEffect> {
        codec::map(tag)?;
        let ty = codec::decode_field(tag, "type", codec::identifier)?;
        let value = |key: &str| codec::decode_field(tag, key, LevelBasedValue::decode);
        Ok(match ty.as_str() {
            "minecraft:all_of" => EnchantmentEntityEffect::AllOf(codec::decode_field(tag, "effects", |t| codec::list_of(t, EnchantmentEntityEffect::decode))?),
            "minecraft:apply_mob_effect" => EnchantmentEntityEffect::ApplyMobEffect {
                to_apply: codec::decode_field(tag, "to_apply", |t| HolderSet::decode("minecraft:mob_effect", t))?,
                min_duration: value("min_duration")?,
                max_duration: value("max_duration")?,
                min_amplifier: value("min_amplifier")?,
                max_amplifier: value("max_amplifier")?,
            },
            "minecraft:change_item_damage" => EnchantmentEntityEffect::ChangeItemDamage { amount: value("amount")? },
            "minecraft:damage_entity" => EnchantmentEntityEffect::DamageEntity {
                min_damage: value("min_damage")?,
                max_damage: value("max_damage")?,
                damage_type: codec::decode_field(tag, "damage_type", |t| {
                    let id = codec::identifier(t)?;
                    DamageType::by_name(&id).ok_or_else(|| format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:damage_type]: {id}"))
                })?,
            },
            "minecraft:explode" => EnchantmentEntityEffect::Explode {
                attribute_to_user: codec::decode_or(tag, "attribute_to_user", false, codec::bool_value)?,
                damage_type: codec::decode_optional(tag, "damage_type", damage_type)?,
                knockback_multiplier: codec::decode_optional(tag, "knockback_multiplier", LevelBasedValue::decode)?,
                immune_blocks: codec::decode_optional(tag, "immune_blocks", |t| HolderSet::decode("minecraft:block", t))?,
                offset: codec::decode_or(tag, "offset", Vec3::ZERO, vec3)?,
                radius: value("radius")?,
                create_fire: codec::decode_or(tag, "create_fire", false, codec::bool_value)?,
                block_interaction: codec::decode_field(tag, "block_interaction", explosion_interaction)?,
                small_particle: codec::decode_field(tag, "small_particle", particle_options)?,
                large_particle: codec::decode_field(tag, "large_particle", particle_options)?,
                block_particles: codec::decode_or(tag, "block_particles", Vec::new(), |t| codec::list_of(t, explosion_particle_info))?,
                sound: codec::decode_field(tag, "sound", sound_event)?,
            },
            "minecraft:ignite" => EnchantmentEntityEffect::Ignite { duration: value("duration")? },
            "minecraft:apply_impulse" => EnchantmentEntityEffect::ApplyImpulse {
                direction: codec::decode_field(tag, "direction", vec3)?,
                coordinate_scale: codec::decode_field(tag, "coordinate_scale", vec3)?,
                magnitude: value("magnitude")?,
            },
            "minecraft:apply_exhaustion" => EnchantmentEntityEffect::ApplyExhaustion { amount: value("amount")? },
            "minecraft:play_sound" => EnchantmentEntityEffect::PlaySound {
                sound_events: codec::decode_field(tag, "sound", |t| codec::compact_list_of(t, sound_event))?,
                volume: codec::decode_field(tag, "volume", |t| float_provider(t, 1.0e-5, 10.0))?,
                pitch: codec::decode_field(tag, "pitch", |t| float_provider(t, 1.0e-5, 2.0))?,
            },
            "minecraft:replace_block" => EnchantmentEntityEffect::ReplaceBlock {
                offset: codec::decode_or(tag, "offset", (0, 0, 0), |t| parse_offset(&serde_json::json!({ "offset": codec::to_json(t) })))?,
                predicate: codec::decode_optional(tag, "predicate", |t| BlockPredicate::parse(&codec::to_json(t)))?,
                block_state: codec::decode_field(tag, "block_state", |t| BlockStateProviderHolder::parse(&codec::to_json(t)))?,
                trigger_game_event: codec::decode_optional(tag, "trigger_game_event", game_event)?,
            },
            "minecraft:replace_disk" => EnchantmentEntityEffect::ReplaceDisk {
                radius: value("radius")?,
                height: value("height")?,
                offset: codec::decode_or(tag, "offset", (0, 0, 0), |t| parse_offset(&serde_json::json!({ "offset": codec::to_json(t) })))?,
                predicate: codec::decode_optional(tag, "predicate", |t| BlockPredicate::parse(&codec::to_json(t)))?,
                block_state: codec::decode_field(tag, "block_state", |t| BlockStateProviderHolder::parse(&codec::to_json(t)))?,
                trigger_game_event: codec::decode_optional(tag, "trigger_game_event", game_event)?,
            },
            "minecraft:run_function" => EnchantmentEntityEffect::RunFunction(codec::decode_field(tag, "function", codec::identifier)?),
            "minecraft:set_block_properties" => EnchantmentEntityEffect::SetBlockProperties {
                properties: codec::decode_field(tag, "properties", |t| Ok(crate::world::item::component::values::block_state_properties(t)))?,
                offset: codec::decode_or(tag, "offset", (0, 0, 0), |t| parse_offset(&serde_json::json!({ "offset": codec::to_json(t) })))?,
                trigger_game_event: codec::decode_optional(tag, "trigger_game_event", game_event)?,
            },
            "minecraft:spawn_particles" => EnchantmentEntityEffect::SpawnParticles {
                particle: codec::decode_field(tag, "particle", particle_options)?,
                horizontal_position: codec::decode_field(tag, "horizontal_position", PositionSource::decode)?,
                vertical_position: codec::decode_field(tag, "vertical_position", PositionSource::decode)?,
                horizontal_velocity: codec::decode_field(tag, "horizontal_velocity", VelocitySource::decode)?,
                vertical_velocity: codec::decode_field(tag, "vertical_velocity", VelocitySource::decode)?,
                speed: codec::decode_or(tag, "speed", FloatProvider::Constant(0.0), |t| Ok(FloatProvider::parse(&codec::to_json(t))?))?,
            },
            "minecraft:summon_entity" => EnchantmentEntityEffect::SummonEntity {
                entity_types: codec::decode_field(tag, "entity", |t| HolderSet::decode("minecraft:entity_type", t))?,
                join_team: codec::decode_or(tag, "join_team", false, codec::bool_value)?,
            },
            other => return Err(format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:enchantment_entity_effect_type]: {other}")),
        })
    }

    pub fn apply(&self, level: &mut ServerLevel, enchantment_level: i32, item: &EnchantedItemInUse, entity: EntityId, position: Vec3) {
        match self {
            EnchantmentEntityEffect::AllOf(effects) => {
                for effect in effects {
                    effect.apply(level, enchantment_level, item, entity, position);
                }
            }
            EnchantmentEntityEffect::ApplyMobEffect { to_apply, min_duration, max_duration, min_amplifier, max_amplifier } => {
                if !level.is_living(entity) {
                    return;
                }
                let contents = to_apply.contents();
                if contents.is_empty() {
                    return;
                }
                let random = &mut level.entity_mut(entity).random;
                let selected = MobEffect(contents[random.next_int_bounded(contents.len() as i32) as usize] as u8);
                let ticks = java_round_f(mth::random_between(random, min_duration.calculate(enchantment_level), max_duration.calculate(enchantment_level)) * 20.0);
                let amplifier = java_round_f(mth::random_between(random, min_amplifier.calculate(enchantment_level), max_amplifier.calculate(enchantment_level))).max(0);
                level.add_effect(entity, MobEffectInstance::new(selected, ticks, amplifier), None);
            }
            EnchantmentEntityEffect::ChangeItemDamage { amount } => {
                let change = amount.calculate(enchantment_level) as i32;
                item.hurt_and_break(level, change);
            }
            EnchantmentEntityEffect::DamageEntity { min_damage, max_damage, damage_type } => {
                let random = &mut level.entity_mut(entity).random;
                let damage = mth::random_between(random, min_damage.calculate(enchantment_level), max_damage.calculate(enchantment_level));
                let source = DamageSource { causing_entity: item.owner, direct_entity: item.owner, ..DamageSource::new(*damage_type) };
                level.hurt_server(entity, source, damage);
            }
            EnchantmentEntityEffect::Ignite { duration } => {
                level.ignite_for_seconds(entity, duration.calculate(enchantment_level));
            }
            EnchantmentEntityEffect::ApplyExhaustion { amount } => {
                if level.is_player(entity) {
                    level.cause_food_exhaustion(entity, amount.calculate(enchantment_level));
                }
            }
            EnchantmentEntityEffect::PlaySound { sound_events, volume, pitch } => {
                if level.entity(entity).is_silent() {
                    return;
                }
                let index = mth::clamp_i(enchantment_level - 1, 0, sound_events.len() as i32 - 1) as usize;
                let source = level.sound_source(entity);
                let random = &mut level.entity_mut(entity).random;
                let (volume, pitch) = (volume.sample(random), pitch.sample(random));
                level.play_sound_at(None, position.x, position.y, position.z, sound_events[index].clone(), source, volume, pitch);
            }
            EnchantmentEntityEffect::SummonEntity { entity_types, join_team } => {
                let block_pos = BlockPos::containing(position.x, position.y, position.z);
                if !is_in_spawnable_bounds(block_pos) {
                    return;
                }
                let contents = entity_types.contents();
                if contents.is_empty() {
                    return;
                }
                let entity_type = registry::ENTITY_TYPE[contents[level.random.next_int_bounded(contents.len() as i32) as usize] as usize];
                let Some(spawned) = level.spawn_entity_type(entity_type, block_pos, crate::world::entity::entity_spawn_reason::EntitySpawnReason::Triggered) else { return };
                if let Some(owner) = item.owner.filter(|&owner| level.is_player(owner))
                    && let Some(LevelEntity::LightningBolt(bolt)) = level.find_entity_mut(spawned)
                {
                    bolt.cause = Some(owner);
                }
                if *join_team && let Some(team) = level.team_of(entity).map(|team| team.name().to_string()) {
                    let name = level.scoreboard_name(spawned);
                    level.scoreboard.borrow_mut().add_player_to_team(&name, &team);
                }
                let e = level.entity_mut(spawned);
                let (y_rot, x_rot) = (e.y_rot, e.x_rot);
                e.snap_to(position.x, position.y, position.z, y_rot, x_rot);
            }
            EnchantmentEntityEffect::ReplaceDisk { radius, height, offset, predicate, block_state, trigger_game_event } => {
                let center_block = BlockPos::containing(position.x, position.y, position.z).offset(offset.0, offset.1, offset.2);
                let dist = radius.calculate(enchantment_level) as i32;
                let height = height.calculate(enchantment_level) as i32;
                for pos in BlockPos::between_closed(center_block.offset(-dist, 0, -dist), center_block.offset(dist, (height - 1).min(0), dist)) {
                    if pos.dist_to_center_sqr(position.x, pos.y as f64 + 0.5, position.z) < (dist * dist) as f64 && predicate.as_ref().is_none_or(|p| p.test(level, pos)) {
                        let mut random = std::mem::replace(&mut level.entity_mut(entity).random, wafer_core::random::LegacyRandomSource::new(0));
                        let state = block_state.value().get_state(level, &mut random, pos);
                        level.entity_mut(entity).random = random;
                        if level.set_block(pos, state, UPDATE_ALL)
                            && let Some(event) = *trigger_game_event
                        {
                            level.game_event_at(Some(entity), event, pos);
                        }
                    }
                }
            }
            EnchantmentEntityEffect::SpawnParticles { particle, horizontal_position, vertical_position, horizontal_velocity, vertical_velocity, speed } => {
                let movement = level.known_movement(entity);
                let e = level.entity_mut(entity);
                let (bb_width, bb_height) = (e.dimensions.width, e.dimensions.height);
                let random = &mut e.random;
                let x = horizontal_position.get_coordinate(position.x, position.x, bb_width, random);
                let y = vertical_position.get_coordinate(position.y, position.y + (bb_height / 2.0) as f64, bb_height, random);
                let z = horizontal_position.get_coordinate(position.z, position.z, bb_width, random);
                let dist = (horizontal_velocity.get_velocity(movement.x, random), vertical_velocity.get_velocity(movement.y, random), horizontal_velocity.get_velocity(movement.z, random));
                let speed = speed.sample(random) as f64;
                level.send_particles(particle.particle_type, |w| particle.write_options(w), x, y, z, 0, dist, speed);
            }
            EnchantmentEntityEffect::ApplyImpulse { direction, coordinate_scale, magnitude } => {
                let e = level.entity(entity);
                let look = crate::world::entity::calculate_view_quaternion(e.x_rot, e.y_rot);
                let (x, y, z) = look.transform_f64(direction.x, direction.y, direction.z);
                let magnitude = magnitude.calculate(enchantment_level) as f64;
                let impulse = Vec3::new(x * coordinate_scale.x * magnitude, y * coordinate_scale.y * magnitude, z * coordinate_scale.z * magnitude);
                level.entity_mut(entity).add_delta_movement(impulse);
                if let Some(player) = level.find_player_mut(entity) {
                    let packet = crate::server::level::server_entity::set_entity_motion(entity, player.entity.delta_movement);
                    player.connection.send_raw(packet);
                    player.entity.sync_velocity = false;
                } else {
                    level.entity_mut(entity).sync_velocity = true;
                }
                if level.is_living(entity) {
                    level.living_mut(entity).apply_post_impulse_grace_time(POST_IMPULSE_CONTEXT_RESET_GRACE_TIME_TICKS);
                }
            }
            EnchantmentEntityEffect::Explode {
                attribute_to_user,
                damage_type,
                knockback_multiplier,
                immune_blocks,
                offset,
                radius,
                create_fire,
                block_interaction,
                small_particle,
                large_particle,
                block_particles,
                sound,
            } => {
                let pos = position.add_vec(*offset);
                let source = attribute_to_user.then_some(entity);
                let damage_source = damage_type.map(|ty| {
                    if *attribute_to_user {
                        DamageSource { causing_entity: Some(entity), direct_entity: Some(entity), ..DamageSource::new(ty) }
                    } else {
                        DamageSource { damage_source_position: Some(pos), ..DamageSource::new(ty) }
                    }
                });
                let calculator = ExplosionDamageCalculator::Simple {
                    explodes_blocks: *block_interaction != ExplosionInteraction::None,
                    damages_entities: damage_type.is_some(),
                    knockback_multiplier: knockback_multiplier.as_ref().map(|value| value.calculate(enchantment_level)),
                    immune_blocks: immune_blocks.clone(),
                };
                let indirect_source = source.filter(|&source| level.is_living(source));
                let radius = radius.calculate(enchantment_level).max(0.0);
                level.explode_with_particles(source, indirect_source, damage_source, calculator, pos, radius, *create_fire, *block_interaction, small_particle, large_particle, block_particles, sound);
            }
            EnchantmentEntityEffect::ReplaceBlock { offset, predicate, block_state, trigger_game_event } => {
                let pos = BlockPos::containing(position.x, position.y, position.z).offset(offset.0, offset.1, offset.2);
                if predicate.as_ref().is_none_or(|p| p.test(level, pos)) {
                    let mut random = std::mem::replace(&mut level.entity_mut(entity).random, wafer_core::random::LegacyRandomSource::new(0));
                    let state = block_state.value().get_state(level, &mut random, pos);
                    level.entity_mut(entity).random = random;
                    if level.set_block(pos, state, UPDATE_ALL)
                        && let Some(event) = *trigger_game_event
                    {
                        level.game_event_at(Some(entity), event, pos);
                    }
                }
            }
            EnchantmentEntityEffect::RunFunction(function) => {
                let e = level.entity(entity);
                let rotation = (e.x_rot, e.y_rot);
                let from_player_tick = level.enchantment_tick_player;
                level.pending_enchantment_functions.push((function.clone(), entity, position, rotation, from_player_tick));
            }
            EnchantmentEntityEffect::SetBlockProperties { properties, offset, trigger_game_event } => {
                let block_pos = BlockPos::containing(position.x, position.y, position.z).offset(offset.0, offset.1, offset.2);
                let state = level.block_state(block_pos);
                let modified = properties.iter().fold(state, |state, (property, value)| state.with_property_by_name(property, value).unwrap_or(state));
                if state != modified
                    && level.set_block(block_pos, modified, UPDATE_ALL)
                    && let Some(event) = *trigger_game_event
                {
                    level.game_event_at(Some(entity), event, block_pos);
                }
            }
        }
    }
}

fn game_event(t: &Nbt) -> Result<crate::world::level::gameevent::game_event::GameEvent, String> {
    t.as_str().and_then(crate::commands::arguments::parse_identifier).and_then(|name| crate::world::level::gameevent::game_event::GameEvent::by_name(&name)).ok_or_else(|| "Unknown game event".to_string())
}
