use wafer_core::random::RandomSource;
use wafer_core::{AABB, BlockPos, Vec3};
use wafer_data::{blocks, registry};
use wafer_protocol::nbt::Nbt;

use super::spawn_data::SpawnData;
use crate::server::level::ServerLevel;
use crate::server::level::events;
use crate::util::problem_reporter::PathElement;
use crate::util::random::WeightedList;
use crate::world::difficulty::Difficulty;
use crate::world::entity::entity_save::{LOAD_ENTITY_RECURSIVE_REASON, block_entity_tick_site};
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::entity_type::EntityType;
use crate::world::item::component::codec::Codec;
use crate::world::entity::{LevelEntity, mob, spawn_placements};
use crate::world::level::BlockGetter;
use crate::world::level::server_level_accessor::ServerLevelAccessor;
use crate::world::level::storage::tag_value_input;

const SERVER_TICK_SITE: [&str; 3] = [
    LOAD_ENTITY_RECURSIVE_REASON,
    "net.minecraft.world.level.BaseSpawner.serverTick(BaseSpawner.java:156)",
    "net.minecraft.world.level.block.entity.SpawnerBlockEntity.serverTick(SpawnerBlockEntity.java:60)",
];

const BASE_SPAWNER_LOGGER: &str = "net.minecraft.world.level.BaseSpawner";
pub const SPAWN_DATA_TAG: &str = "SpawnData";
const EVENT_SPAWN: i32 = 1;
const DEFAULT_SPAWN_DELAY: i32 = 20;
const DEFAULT_MIN_SPAWN_DELAY: i32 = 200;
const DEFAULT_MAX_SPAWN_DELAY: i32 = 800;
const DEFAULT_SPAWN_COUNT: i32 = 4;
const DEFAULT_MAX_NEARBY_ENTITIES: i32 = 6;
const DEFAULT_REQUIRED_PLAYER_RANGE: i32 = 16;
const DEFAULT_SPAWN_RANGE: i32 = 4;

#[derive(Clone)]
pub struct BaseSpawner {
    spawn_delay: i32,
    spawn_potentials: WeightedList<SpawnData>,
    next_spawn_data: Option<SpawnData>,
    min_spawn_delay: i32,
    max_spawn_delay: i32,
    spawn_count: i32,
    max_nearby_entities: i32,
    required_player_range: i32,
    spawn_range: i32,
    pub entity_owner: Option<crate::world::entity::EntityId>,
}

impl Default for BaseSpawner {
    fn default() -> BaseSpawner {
        BaseSpawner {
            spawn_delay: DEFAULT_SPAWN_DELAY,
            spawn_potentials: WeightedList::empty(),
            next_spawn_data: None,
            min_spawn_delay: DEFAULT_MIN_SPAWN_DELAY,
            max_spawn_delay: DEFAULT_MAX_SPAWN_DELAY,
            spawn_count: DEFAULT_SPAWN_COUNT,
            max_nearby_entities: DEFAULT_MAX_NEARBY_ENTITIES,
            required_player_range: DEFAULT_REQUIRED_PLAYER_RANGE,
            spawn_range: DEFAULT_SPAWN_RANGE,
            entity_owner: None,
        }
    }
}

fn short_value(tag: &Nbt) -> Option<i32> {
    super::block::entity::int_value(tag).map(|v| v as i16 as i32)
}

pub fn read_spawn_pos(entity_tag: &Nbt) -> Option<Vec3> {
    tag_value_input::read(entity_tag, "Pos", crate::util::extra_codecs::vec3)
}

pub fn entity_type_by(entity_tag: &Nbt) -> Option<&'static EntityType> {
    let id = tag_value_input::read_or_error(entity_tag, "id", |tag| Codec::Element("minecraft:entity_type").decode(tag))?;
    let id = id.as_str()?;
    EntityType::by_name(id).filter(|_| crate::server::commands::summon_command::MODELLED.contains(&id))
}

fn exact_class(entity_type: i32) -> &'static str {
    let name = registry::ENTITY_TYPE[entity_type as usize];
    if name.ends_with("_chest_boat") {
        "minecraft:oak_chest_boat"
    } else if name.ends_with("_boat") {
        "minecraft:oak_boat"
    } else {
        name
    }
}

impl BaseSpawner {
    pub fn set_entity_id(&mut self, entity_type: &str, random: &mut impl RandomSource) {
        let spawn_data = self.get_or_create_next_spawn_data(None, random, BlockPos::ZERO);
        let entity = spawn_data.get_entity_to_spawn_mut();
        match entity.iter_mut().find(|(key, _)| key == "id") {
            Some((_, id)) => *id = Nbt::String(entity_type.into()),
            None => entity.push(("id".into(), Nbt::String(entity_type.into()))),
        }
    }

    pub fn set_entity_data(&mut self, entity_data: &Nbt, random: &mut impl RandomSource) {
        let (entity_type, data) = crate::world::item::component::values::typed_entity_data(entity_data);
        let entity_type = crate::commands::arguments::parse_identifier(&entity_type).unwrap_or(entity_type);
        self.set_entity_id(&entity_type, random);
        let spawn_data = self.get_or_create_next_spawn_data(None, random, BlockPos::ZERO);
        if let Nbt::Compound(data) = &data {
            crate::nbt::merge(spawn_data.get_entity_to_spawn_mut(), data);
        }
    }

    fn problem_path(&self, pos: BlockPos) -> String {
        let class = if self.entity_owner.is_some() {
            "net.minecraft.world.entity.vehicle.minecart.MinecartSpawner$1"
        } else {
            "net.minecraft.world.level.block.entity.SpawnerBlockEntity$1"
        };
        format!("{class}@{:x}", pos.as_long() as u32)
    }

    fn is_near_player(&self, level: &ServerLevel, pos: BlockPos) -> bool {
        level.has_nearby_alive_player(pos.x as f64 + 0.5, pos.y as f64 + 0.5, pos.z as f64 + 0.5, self.required_player_range as f64)
    }

    pub fn server_tick(&mut self, level: &mut ServerLevel, pos: BlockPos) {
        if !self.is_near_player(level, pos) || !level.is_spawner_block_enabled() {
            return;
        }
        if self.spawn_delay == -1 {
            self.delay(level, pos);
        }
        if self.spawn_delay > 0 {
            self.spawn_delay -= 1;
            return;
        }
        let mut delay = false;
        level.with_level_random(|level, random| {
            self.get_or_create_next_spawn_data(Some(level), random, pos);
        });
        let next_spawn_data = self.next_spawn_data.as_ref().expect("the next spawn data");
        let entity_tag = next_spawn_data.get_entity_to_spawn();
        for _ in 0..self.spawn_count {
            let _collector = tag_value_input::ScopedCollector::new(PathElement::Text(self.problem_path(pos)), BASE_SPAWNER_LOGGER);
            let Some(entity_type) = entity_type_by(entity_tag) else {
                self.delay(level, pos);
                return;
            };
            let spawn_pos = match read_spawn_pos(entity_tag) {
                Some(spawn_pos) => spawn_pos,
                None => {
                    let random = &mut level.random;
                    let x = pos.x as f64 + (random.next_double() - random.next_double()) * self.spawn_range as f64 + 0.5;
                    let y = (pos.y + random.next_int_bounded(3) - 1) as f64;
                    let z = pos.z as f64 + (random.next_double() - random.next_double()) * self.spawn_range as f64 + 0.5;
                    Vec3::new(x, y, z)
                }
            };
            if !ServerLevelAccessor::no_collision(level, &entity_type.get_spawn_aabb(spawn_pos.x, spawn_pos.y, spawn_pos.z)) {
                continue;
            }
            let spawn_block_pos = BlockPos::containing_vec(spawn_pos);
            if let Some(custom_spawn_rules) = next_spawn_data.get_custom_spawn_rules() {
                if !entity_type.get_category().is_friendly() && level.difficulty == Difficulty::Peaceful {
                    continue;
                }
                if !custom_spawn_rules.is_valid_position(spawn_block_pos, level) {
                    continue;
                }
            } else if !spawn_placements::check_spawn_rules(entity_type, level, EntitySpawnReason::Spawner, spawn_block_pos) {
                continue;
            }
            let site = block_entity_tick_site(&SERVER_TICK_SITE);
            let mut loaded = level.load_entity_recursive_for_spawn(entity_tag, &site, &mut |entity| {
                let e = entity.entity_mut();
                let (y_rot, x_rot) = (e.y_rot, e.x_rot);
                e.snap_to(spawn_pos.x, spawn_pos.y, spawn_pos.z, y_rot, x_rot);
            });
            if loaded.is_empty() {
                self.delay(level, pos);
                return;
            }
            let class = exact_class(loaded[0].entity().entity_type);
            let area = AABB::new(pos.x as f64, pos.y as f64, pos.z as f64, (pos.x + 1) as f64, (pos.y + 1) as f64, (pos.z + 1) as f64).inflate(
                self.spawn_range as f64,
                self.spawn_range as f64,
                self.spawn_range as f64,
            );
            let near_by = level.get_entities(None, &area).into_iter().filter(|&id| exact_class(level.entity(id).entity_type) == class).count();
            if near_by >= self.max_nearby_entities as usize {
                self.delay(level, pos);
                return;
            }
            let y_rot = level.random.next_float() * 360.0;
            let e = loaded[0].entity_mut();
            let (x, y, z) = (e.x(), e.y(), e.z());
            e.snap_to(x, y, z, y_rot, 0.0);
            if let LevelEntity::Mob(mob) = &loaded[0] {
                if next_spawn_data.get_custom_spawn_rules().is_none() && !mob::check_spawn_rules(mob, level, EntitySpawnReason::Spawner) || !mob::check_spawn_obstruction(mob, level) {
                    continue;
                }
                let has_no_configuration = matches!(entity_tag, Nbt::Compound(fields) if fields.len() == 1) && entity_tag.get("id").and_then(Nbt::as_str).is_some();
                if has_no_configuration {
                    let entity = loaded.remove(0);
                    let difficulty = level.current_difficulty_at(entity.entity().block_position());
                    let (entity, _) = level.finalize_spawn_detached(entity, &difficulty, EntitySpawnReason::Spawner, None);
                    loaded.insert(0, entity);
                }
                if let Some(equipment) = next_spawn_data.get_equipment() {
                    let entity = loaded.remove(0);
                    loaded.insert(0, level.mob_equip_detached(entity, equipment));
                }
            }
            let id = loaded[0].entity().id;
            let is_mob = matches!(loaded[0], LevelEntity::Mob(_));
            if !level.try_add_fresh_entity_with_passengers(loaded) {
                self.delay(level, pos);
                return;
            }
            level.level_event(None, events::PARTICLES_MOBBLOCK_SPAWN, pos, 0);
            level.game_event_at(Some(id), crate::world::level::gameevent::game_event::ENTITY_PLACE, spawn_block_pos);
            if is_mob {
                level.spawn_anim(id);
            }
            delay = true;
        }
        if delay {
            self.delay(level, pos);
        }
    }

    fn delay(&mut self, level: &mut ServerLevel, pos: BlockPos) {
        if self.max_spawn_delay <= self.min_spawn_delay {
            self.spawn_delay = self.min_spawn_delay;
        } else {
            self.spawn_delay = self.min_spawn_delay + level.random.next_int_bounded(self.max_spawn_delay - self.min_spawn_delay);
        }
        if let Some(entry) = self.spawn_potentials.get_random(&mut level.random).cloned() {
            self.set_next_spawn_data(Some(level), pos, entry);
        }
        self.broadcast_event(level, pos, EVENT_SPAWN);
    }

    pub fn load(&mut self, input: &Nbt) {
        let int = |key: &str, default: i32| tag_value_input::get_numeric(input, key).and_then(super::block::entity::int_value).unwrap_or(default);
        self.spawn_delay = tag_value_input::get_numeric(input, "Delay").and_then(short_value).unwrap_or(DEFAULT_SPAWN_DELAY);
        if let Some(next_spawn_data) = tag_value_input::read_or_error(input, SPAWN_DATA_TAG, SpawnData::decode) {
            self.set_next_spawn_data(None, BlockPos::ZERO, next_spawn_data);
        }
        let potentials = tag_value_input::read_or_error(input, "SpawnPotentials", crate::util::codec::list);
        self.spawn_potentials = match potentials {
            Some(potentials) => WeightedList::new(
                potentials
                    .iter()
                    .filter_map(|entry| {
                        let weight = entry.get("weight").and_then(|weight| crate::util::codec::int(weight).ok()).filter(|&weight| weight >= 0)?;
                        Some((SpawnData::decode(entry.get("data")?).ok()?, weight))
                    })
                    .collect(),
            ),
            None => WeightedList::new(vec![(self.next_spawn_data.clone().unwrap_or_default(), 1)]),
        };
        self.min_spawn_delay = int("MinSpawnDelay", DEFAULT_MIN_SPAWN_DELAY);
        self.max_spawn_delay = int("MaxSpawnDelay", DEFAULT_MAX_SPAWN_DELAY);
        self.spawn_count = int("SpawnCount", DEFAULT_SPAWN_COUNT);
        self.max_nearby_entities = int("MaxNearbyEntities", DEFAULT_MAX_NEARBY_ENTITIES);
        self.required_player_range = int("RequiredPlayerRange", DEFAULT_REQUIRED_PLAYER_RANGE);
        self.spawn_range = int("SpawnRange", DEFAULT_SPAWN_RANGE);
    }

    pub fn save(&self, output: &mut Vec<(String, Nbt)>) {
        output.push(("Delay".into(), Nbt::Short(self.spawn_delay as i16)));
        output.push(("MinSpawnDelay".into(), Nbt::Short(self.min_spawn_delay as i16)));
        output.push(("MaxSpawnDelay".into(), Nbt::Short(self.max_spawn_delay as i16)));
        output.push(("SpawnCount".into(), Nbt::Short(self.spawn_count as i16)));
        output.push(("MaxNearbyEntities".into(), Nbt::Short(self.max_nearby_entities as i16)));
        output.push(("RequiredPlayerRange".into(), Nbt::Short(self.required_player_range as i16)));
        output.push(("SpawnRange".into(), Nbt::Short(self.spawn_range as i16)));
        if let Some(next_spawn_data) = &self.next_spawn_data {
            output.push((SPAWN_DATA_TAG.into(), next_spawn_data.encode()));
        }
        let potentials = self
            .spawn_potentials
            .items
            .iter()
            .map(|(data, weight)| Nbt::Compound(vec![("data".into(), data.encode()), ("weight".into(), Nbt::Int(*weight))]))
            .collect();
        output.push(("SpawnPotentials".into(), Nbt::List(potentials)));
    }

    pub fn on_event_triggered(&mut self, id: i32) -> bool {
        id == EVENT_SPAWN
    }

    fn set_next_spawn_data(&mut self, level: Option<&mut ServerLevel>, pos: BlockPos, next_spawn_data: SpawnData) {
        self.next_spawn_data = Some(next_spawn_data);
        if self.entity_owner.is_none()
            && let Some(level) = level
        {
            let state = level.block_state(pos);
            level.send_block_updated(pos, state, state);
        }
    }

    fn get_or_create_next_spawn_data(&mut self, level: Option<&mut ServerLevel>, random: &mut impl RandomSource, pos: BlockPos) -> &mut SpawnData {
        if self.next_spawn_data.is_none() {
            let next_spawn_data = self.spawn_potentials.get_random(random).cloned().unwrap_or_default();
            self.set_next_spawn_data(level, pos, next_spawn_data);
        }
        self.next_spawn_data.as_mut().expect("the next spawn data")
    }

    fn broadcast_event(&self, level: &mut ServerLevel, pos: BlockPos, id: i32) {
        match self.entity_owner {
            Some(entity) => level.broadcast_entity_event(entity, id as u8),
            None => level.block_event(pos, blocks::SPAWNER, id, 0),
        }
    }
}
