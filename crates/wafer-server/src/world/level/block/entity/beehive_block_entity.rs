use wafer_core::random::RandomSource;
use wafer_core::BlockPos;
use wafer_data::properties::{HORIZONTAL_FACING, LEVEL_HONEY};
use wafer_data::{BlockState, block_tags, entity_type_tags, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use crate::server::level::{ServerLevel, SoundSource};
use crate::world::attribute::environment_attributes;
use crate::world::entity::animal::bee::bee;
use crate::world::entity::entity_save::{LOAD_ENTITY_RECURSIVE_TYPED, block_entity_tick_site};
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::component::DataComponentType;
use crate::world::item::component::{self as dc};
use crate::world::level::BlockGetter;
use crate::world::level::block::campfire_block;
use crate::world::level::block::entity::BlockEntityKind;
use crate::world::level::update_flags::UPDATE_ALL;
use crate::world::phys::shapes::collision_context::CollisionContext;
use crate::world::level::storage::tag_value_input;

const IGNORED_BEE_TAGS: [&str; 25] = [
    "Air",
    "drop_chances",
    "equipment",
    "Brain",
    "CanPickUpLoot",
    "DeathTime",
    "fall_distance",
    "FallFlying",
    "Fire",
    "HurtTime",
    "LeftHanded",
    "Motion",
    "NoGravity",
    "OnGround",
    "PortalCooldown",
    "Pos",
    "Rotation",
    "sleeping_pos",
    "CannotEnterHiveTicks",
    "TicksSincePollination",
    "CropsGrownSincePollination",
    "hive_pos",
    "Passengers",
    "leash",
    "UUID",
];
pub const MAX_OCCUPANTS: usize = 3;
pub const MIN_TICKS_BEFORE_REENTERING_HIVE: i32 = 400;
const MIN_OCCUPATION_TICKS_NECTAR: i32 = 2400;
pub const MIN_OCCUPATION_TICKS_NECTARLESS: i32 = 600;

#[derive(Clone)]
pub struct Occupant {
    pub entity_data: Nbt,
    pub ticks_in_hive: i32,
    pub min_ticks_in_hive: i32,
}

impl Occupant {
    pub fn of(level: &ServerLevel, entity: EntityId) -> Occupant {
        let entity_type = registry::ENTITY_TYPE[level.entity(entity).entity_type as usize];
        let mut entity_tag = match level.save_entity(entity) {
            Some(Nbt::Compound(fields)) => fields,
            _ => vec![("id".into(), Nbt::String(entity_type.into()))].into(),
        };
        entity_tag.retain(|(key, _)| !IGNORED_BEE_TAGS.contains(&key.as_str()));
        let has_nectar = entity_tag.iter().find(|(key, _)| key == "HasNectar").and_then(|(_, value)| value.as_i64()).is_some_and(|v| v != 0);
        Occupant { entity_data: Nbt::Compound(entity_tag), ticks_in_hive: 0, min_ticks_in_hive: if has_nectar { MIN_OCCUPATION_TICKS_NECTAR } else { MIN_OCCUPATION_TICKS_NECTARLESS } }
    }

    pub fn create(ticks_in_hive: i32) -> Occupant {
        Occupant { entity_data: Nbt::Compound(vec![("id".into(), Nbt::String("minecraft:bee".into()))].into()), ticks_in_hive, min_ticks_in_hive: 600 }
    }

    pub fn save(&self) -> Nbt {
        Nbt::Compound(vec![
            ("entity_data".into(), self.entity_data.clone()),
            ("ticks_in_hive".into(), Nbt::Int(self.ticks_in_hive)),
            ("min_ticks_in_hive".into(), Nbt::Int(self.min_ticks_in_hive)),
        ].into())
    }

    pub fn load(tag: &Nbt) -> Option<Occupant> {
        let entity_data = tag.get("entity_data").filter(|data| data.get("id").and_then(Nbt::as_str).is_some())?.clone();
        let int = |key: &str| match tag.get(key)? {
            Nbt::Byte(v) => Some(*v as i32),
            Nbt::Short(v) => Some(*v as i32),
            Nbt::Int(v) => Some(*v),
            Nbt::Long(v) => Some(*v as i32),
            _ => None,
        };
        Some(Occupant { entity_data, ticks_in_hive: int("ticks_in_hive")?, min_ticks_in_hive: int("min_ticks_in_hive")? })
    }

    pub fn create_entity(&self, level: &mut ServerLevel, hive_pos: BlockPos, site: &dyn Fn() -> Vec<&'static str>) -> Option<LevelEntity> {
        let Nbt::Compound(mut entity_tag) = self.entity_data.clone() else { return None };
        entity_tag.retain(|(key, _)| !IGNORED_BEE_TAGS.contains(&key.as_str()));
        let collector = tag_value_input::ScopedCollector::new_empty("net.minecraft.world.entity.EntityType");
        let entity = level.load_entity_typed(&Nbt::Compound(entity_tag), site);
        drop(collector);
        let mut entity = entity?;
        if !entity_type_tags::BEEHIVE_INHABITORS.contains(entity.entity().entity_type as usize) {
            return None;
        }
        entity.entity_mut().set_no_gravity(true);
        if let LevelEntity::Mob(mob) = &mut entity
            && bee::is_bee(mob)
        {
            bee::bee_mut(mob).set_hive_pos(hive_pos);
            Occupant::set_bee_release_data(self.ticks_in_hive, mob);
        }
        Some(entity)
    }

    fn set_bee_release_data(ticks_in_hive: i32, bee: &mut crate::world::entity::mob::Mob) {
        Occupant::update_bee_age(ticks_in_hive, bee);
        let animal = bee.animal_mut().expect("an animal");
        animal.in_love = 0.max(animal.in_love - ticks_in_hive);
    }

    fn update_bee_age(ticks_in_hive: i32, bee: &mut crate::world::entity::mob::Mob) {
        if !bee.is_age_locked() {
            let age = bee.ageable().expect("an ageable mob").age;
            if age < 0 {
                bee.set_age(0.min(age + ticks_in_hive));
            } else if age > 0 {
                bee.set_age(0.max(age - ticks_in_hive));
            }
        }
    }
}

#[derive(Clone)]
struct BeeData {
    occupant: Occupant,
    ticks_in_hive: i32,
}

impl BeeData {
    fn new(occupant: Occupant) -> BeeData {
        let ticks_in_hive = occupant.ticks_in_hive;
        BeeData { occupant, ticks_in_hive }
    }

    fn tick(&mut self) -> bool {
        let ticks_in_hive = self.ticks_in_hive;
        self.ticks_in_hive += 1;
        ticks_in_hive > self.occupant.min_ticks_in_hive
    }

    fn to_occupant(&self) -> Occupant {
        Occupant { entity_data: self.occupant.entity_data.clone(), ticks_in_hive: self.ticks_in_hive, min_ticks_in_hive: self.occupant.min_ticks_in_hive }
    }

    fn has_nectar(&self) -> bool {
        self.occupant.entity_data.get("HasNectar").and_then(Nbt::as_i64).is_some_and(|v| v != 0)
    }
}

#[derive(Clone, Default)]
pub struct BeehiveBlockEntity {
    stored: Vec<BeeData>,
    saved_flower_pos: Option<BlockPos>,
}

impl BeehiveBlockEntity {
    pub fn is_empty(&self) -> bool {
        self.stored.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.stored.len() == MAX_OCCUPANTS
    }

    pub fn store_bee(&mut self, occupant: Occupant) {
        self.stored.push(BeeData::new(occupant));
    }

    pub fn get_occupant_count(&self) -> i32 {
        self.stored.len() as i32
    }

    fn has_saved_flower_pos(&self) -> bool {
        self.saved_flower_pos.is_some()
    }

    fn get_bees(&self) -> Vec<Occupant> {
        self.stored.iter().map(BeeData::to_occupant).collect()
    }

    pub fn load_additional(&mut self, input: &Nbt) {
        self.stored.clear();
        if let Some(bees) = tag_value_input::read(input, "bees", |tag| dc::BEES.decode_partial(tag)) {
            bees.as_list().iter().filter_map(Occupant::load).for_each(|occupant| self.store_bee(occupant));
        }
        self.saved_flower_pos = tag_value_input::read(input, "flower_pos", crate::util::extra_codecs::block_pos);
    }

    pub fn save_additional(&self, output: &mut Vec<(String, Nbt)>) {
        output.push(("bees".into(), Nbt::List(self.get_bees().iter().map(Occupant::save).collect())));
        if let Some(pos) = self.saved_flower_pos {
            output.push(("flower_pos".into(), Nbt::IntArray(vec![pos.x, pos.y, pos.z])));
        }
    }

    pub fn apply_implicit_components<'a>(&mut self, get: &mut impl FnMut(DataComponentType) -> Option<&'a Nbt>) {
        self.stored.clear();
        let bees: Vec<Occupant> = get(dc::BEES).map_or(&[][..], Nbt::as_list).iter().filter_map(Occupant::load).collect();
        bees.into_iter().for_each(|occupant| self.store_bee(occupant));
    }

    pub fn collect_implicit_components(&self, components: &mut Vec<(DataComponentType, Option<Nbt>)>) {
        components.retain(|(t, _)| *t != dc::BEES);
        components.push((dc::BEES, Some(Nbt::List(self.get_bees().iter().map(Occupant::save).collect()))));
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BeeReleaseStatus {
    HoneyDelivered,
    BeeReleased,
    Emergency,
}

pub fn get_honey_level(block_state: BlockState) -> i32 {
    block_state.get(LEVEL_HONEY)
}

impl ServerLevel {
    pub fn with_beehive<R>(&mut self, pos: BlockPos, f: impl FnOnce(&mut ServerLevel, &mut BeehiveBlockEntity, BlockState) -> R) -> Option<R> {
        let block_entity = self.get_block_entity(pos)?;
        let block_entity_state = block_entity.block_state();
        let BlockEntityKind::Beehive(beehive) = &mut block_entity.kind else { return None };
        let mut beehive = std::mem::take(beehive);
        let result = f(self, &mut beehive, block_entity_state);
        if let Some(block_entity) = self.get_block_entity(pos)
            && let BlockEntityKind::Beehive(slot) = &mut block_entity.kind
        {
            *slot = beehive;
        }
        Some(result)
    }

    pub fn beehive_set_changed(&mut self, pos: BlockPos) {
        if self.beehive_is_fire_nearby(pos) {
            let state = self.block_state(pos);
            self.with_beehive(pos, |level, beehive, block_entity_state| {
                beehive.empty_all_living_from_hive(level, pos, block_entity_state, None, state, BeeReleaseStatus::Emergency)
            });
        }
    }

    pub fn beehive_is_fire_nearby(&mut self, pos: BlockPos) -> bool {
        self.find_blocks_in(pos.offset(-1, -1, -1), pos.offset(1, 1, 1)).filter_state(|state| state.block().is_instance_of("FireBlock")).any_matched(self)
    }

    pub fn beehive_is_sedated(&mut self, pos: BlockPos) -> bool {
        campfire_block::is_smokey_pos(self, pos)
    }

    pub fn beehive_add_occupant(&mut self, pos: BlockPos, bee: EntityId) {
        self.with_beehive(pos, |level, beehive, block_entity_state| beehive.add_occupant(level, pos, block_entity_state, bee));
    }

    pub fn beehive_server_tick(&mut self, pos: BlockPos, state: BlockState) {
        self.with_beehive(pos, |level, beehive, _| {
            tick_occupants(level, pos, state, &mut beehive.stored, beehive.saved_flower_pos);
            if !beehive.stored.is_empty() && level.random.next_double() < 0.005 {
                let (x, y, z) = (pos.x as f64 + 0.5, pos.y as f64, pos.z as f64 + 0.5);
                level.play_sound_at(None, x, y, z, registry_id(registry::SOUND_EVENT, "minecraft:block.beehive.work"), SoundSource::Blocks, 1.0, 1.0);
            }
        });
    }
}

impl BeehiveBlockEntity {
    pub fn empty_all_living_from_hive(&mut self, level: &mut ServerLevel, pos: BlockPos, block_entity_state: BlockState, player: Option<EntityId>, state: BlockState, release_reason: BeeReleaseStatus) {
        let released_from_hive = self.release_all_occupants(level, pos, block_entity_state, state, release_reason);
        let Some(player) = player else { return };
        for released in released_from_hive {
            if level.mob(released).is_none() {
                continue;
            }
            if level.entity(player).position().distance_to_sqr(level.entity(released).position()) <= 16.0 {
                if !level.beehive_is_sedated(pos) {
                    level.set_target(released, Some(player));
                } else {
                    bee::bee_mut(level.mob_mut(released).expect("a bee")).set_stay_out_of_hive_countdown(MIN_TICKS_BEFORE_REENTERING_HIVE);
                }
            }
        }
    }

    fn release_all_occupants(&mut self, level: &mut ServerLevel, pos: BlockPos, block_entity_state: BlockState, state: BlockState, release_status: BeeReleaseStatus) -> Vec<EntityId> {
        let mut spawned = Vec::new();
        let saved_flower_pos = self.saved_flower_pos;
        self.stored.retain(|occupant_entry| !release_occupant(level, pos, state, occupant_entry.to_occupant(), Some(&mut spawned), release_status, saved_flower_pos, &Vec::new));
        if !spawned.is_empty() {
            level.block_entity_set_changed_static(pos, block_entity_state);
        }
        spawned
    }

    fn add_occupant(&mut self, level: &mut ServerLevel, pos: BlockPos, block_entity_state: BlockState, bee: EntityId) {
        if self.stored.len() >= MAX_OCCUPANTS {
            return;
        }
        level.stop_riding(bee);
        level.eject_passengers(bee);
        level.drop_leash(bee);
        self.store_bee(Occupant::of(level, bee));
        let bee_saved_flower_pos = bee::bee(level.mob(bee).expect("a bee")).get_saved_flower_pos();
        if let Some(bee_saved_flower_pos) = bee_saved_flower_pos
            && (!self.has_saved_flower_pos() || level.random.next_boolean())
        {
            self.saved_flower_pos = Some(bee_saved_flower_pos);
        }
        level.play_sound_at(None, pos.x as f64, pos.y as f64, pos.z as f64, registry_id(registry::SOUND_EVENT, "minecraft:block.beehive.enter"), SoundSource::Blocks, 1.0, 1.0);
        level.game_event_at_with(crate::world::level::gameevent::game_event::BLOCK_CHANGE, pos, crate::world::level::gameevent::game_event::Context::of_entity_and_state(Some(bee), Some(block_entity_state)));
        level.discard(bee);
        level.block_entity_set_changed_static(pos, block_entity_state);
    }
}

fn release_occupant(
    level: &mut ServerLevel,
    block_pos: BlockPos,
    state: BlockState,
    bee_data: Occupant,
    spawned: Option<&mut Vec<EntityId>>,
    release_status: BeeReleaseStatus,
    saved_flower_pos: Option<BlockPos>,
    site: &dyn Fn() -> Vec<&'static str>,
) -> bool {
    if level.environment_attributes().get_value_at(environment_attributes::BEES_STAY_IN_HIVE, block_pos) && release_status != BeeReleaseStatus::Emergency {
        return false;
    }
    let facing = state.get(HORIZONTAL_FACING);
    let facing_pos = block_pos.relative(facing);
    let front_blocked = !CollisionContext::EMPTY.get_collision_shape(level.block_state(facing_pos), facing_pos).0.is_empty();
    if front_blocked && release_status != BeeReleaseStatus::Emergency {
        return false;
    }
    let Some(mut entity) = bee_data.create_entity(level, block_pos, site) else { return false };
    if let LevelEntity::Mob(mob) = &mut entity
        && bee::is_bee(mob)
    {
        if let Some(saved_flower_pos) = saved_flower_pos
            && !bee::bee(mob).has_saved_flower_pos()
            && level.random.next_float() < 0.9
        {
            bee::bee_mut(mob).set_saved_flower_pos(saved_flower_pos);
        }
        if release_status == BeeReleaseStatus::HoneyDelivered {
            bee::drop_off_nectar(mob);
            if state.is(&block_tags::BEEHIVES) && state.has(LEVEL_HONEY) {
                let honey_level = get_honey_level(state);
                if honey_level < 5 {
                    let mut level_increase = if level.random.next_int_bounded(100) == 0 { 2 } else { 1 };
                    if honey_level + level_increase > 5 {
                        level_increase -= 1;
                    }
                    level.set_block(block_pos, state.set(LEVEL_HONEY, honey_level + level_increase), UPDATE_ALL);
                }
            }
        }
        if let Some(spawned) = spawned {
            spawned.push(mob.entity.id);
        }
        let e = &mut mob.entity;
        let bb_width = e.dimensions.width;
        let delta = if front_blocked { 0.0 } else { 0.55 + (bb_width / 2.0) as f64 };
        let spawn_x = block_pos.x as f64 + 0.5 + delta * facing.step_x() as f64;
        let spawn_y = block_pos.y as f64 + 0.5 - (e.dimensions.height / 2.0) as f64;
        let spawn_z = block_pos.z as f64 + 0.5 + delta * facing.step_z() as f64;
        let (y_rot, x_rot) = (e.y_rot, e.x_rot);
        e.snap_to(spawn_x, spawn_y, spawn_z, y_rot, x_rot);
    }
    level.play_sound(None, block_pos, registry_id(registry::SOUND_EVENT, "minecraft:block.beehive.exit"), SoundSource::Blocks, 1.0, 1.0);
    let id = entity.entity().id;
    let added = level.add_fresh_entity(entity);
    let block_state = level.block_state(block_pos);
    level.game_event_at_with(crate::world::level::gameevent::game_event::BLOCK_CHANGE, block_pos, crate::world::level::gameevent::game_event::Context::of_entity_and_state(Some(id), Some(block_state)));
    added
}

fn tick_occupants_site() -> Vec<&'static str> {
    block_entity_tick_site(&[
        LOAD_ENTITY_RECURSIVE_TYPED,
        "net.minecraft.world.level.block.entity.BeehiveBlockEntity$Occupant.createEntity(BeehiveBlockEntity.java:399)",
        "net.minecraft.world.level.block.entity.BeehiveBlockEntity.releaseOccupant(BeehiveBlockEntity.java:209)",
        "net.minecraft.world.level.block.entity.BeehiveBlockEntity.tickOccupants(BeehiveBlockEntity.java:267)",
        "net.minecraft.world.level.block.entity.BeehiveBlockEntity.serverTick(BeehiveBlockEntity.java:279)",
    ])
}

fn tick_occupants(level: &mut ServerLevel, pos: BlockPos, state: BlockState, stored: &mut Vec<BeeData>, saved_flower_pos: Option<BlockPos>) {
    let mut changed = false;
    stored.retain_mut(|data| {
        if data.tick() {
            let release_status = if data.has_nectar() { BeeReleaseStatus::HoneyDelivered } else { BeeReleaseStatus::BeeReleased };
            if release_occupant(level, pos, state, data.to_occupant(), None, release_status, saved_flower_pos, &tick_occupants_site) {
                changed = true;
                return false;
            }
        }
        true
    });
    if changed {
        level.block_entity_set_changed_static(pos, state);
    }
}
