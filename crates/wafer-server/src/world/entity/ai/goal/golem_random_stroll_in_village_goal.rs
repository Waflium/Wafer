use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, Vec3};

use super::random_stroll_goal::RandomStrollGoal;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::util::land_random_pos;
use crate::world::entity::ai::village::poi::poi_manager::Occupancy;

const POI_SECTION_SCAN_RADIUS: i32 = 2;
const RANDOM_POS_XY_DISTANCE: i32 = 10;
const RANDOM_POS_Y_DISTANCE: i32 = 7;

pub struct GolemRandomStrollInVillageGoal {
    pub random_stroll_goal: RandomStrollGoal,
}

impl GolemRandomStrollInVillageGoal {
    pub fn new(speed_modifier: f64) -> GolemRandomStrollInVillageGoal {
        GolemRandomStrollInVillageGoal { random_stroll_goal: RandomStrollGoal::with(speed_modifier, 240, false) }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        self.random_stroll_goal.can_use(level, mob, get_position)
    }
}

fn get_position(level: &mut ServerLevel, mob: EntityId) -> Option<Vec3> {
    if level.random.next_float() < 0.3 {
        return get_position_towards_anywhere(level, mob);
    }
    let target = if level.random.next_float() < 0.7 {
        get_position_towards_villager_who_wants_golem(level, mob).or_else(|| get_position_towards_poi(level, mob))
    } else {
        get_position_towards_poi(level, mob).or_else(|| get_position_towards_villager_who_wants_golem(level, mob))
    };
    target.or_else(|| get_position_towards_anywhere(level, mob))
}

fn get_position_towards_anywhere(level: &mut ServerLevel, mob: EntityId) -> Option<Vec3> {
    land_random_pos::get_pos(level, mob, RANDOM_POS_XY_DISTANCE, RANDOM_POS_Y_DISTANCE)
}

fn get_position_towards_villager_who_wants_golem(level: &mut ServerLevel, mob: EntityId) -> Option<Vec3> {
    let area = level.entity(mob).bounding_box().inflate(32.0, 32.0, 32.0);
    let game_time = level.game_time;
    let villagers = level.get_living_entities(&area, |level, entity| level.is_villager(entity) && level.wants_to_spawn_golem(entity, game_time));
    if villagers.is_empty() {
        return None;
    }
    let villager = villagers[level.random.next_int_bounded(villagers.len() as i32) as usize];
    let target_pos = level.entity(villager).position();
    land_random_pos::get_pos_towards(level, mob, RANDOM_POS_XY_DISTANCE, RANDOM_POS_Y_DISTANCE, target_pos)
}

fn get_position_towards_poi(level: &mut ServerLevel, mob: EntityId) -> Option<Vec3> {
    let target_section = get_random_village_section(level, mob)?;
    let target_pos = get_random_poi_within_section(level, target_section)?;
    land_random_pos::get_pos_towards(level, mob, RANDOM_POS_XY_DISTANCE, RANDOM_POS_Y_DISTANCE, Vec3::at_bottom_center_of(target_pos))
}

fn get_random_village_section(level: &mut ServerLevel, mob: EntityId) -> Option<BlockPos> {
    let pos = level.entity(mob).block_position();
    let center = BlockPos::new(pos.x >> 4, pos.y >> 4, pos.z >> 4);
    let mut village_sections = Vec::new();
    for z in center.z - POI_SECTION_SCAN_RADIUS..=center.z + POI_SECTION_SCAN_RADIUS {
        for y in center.y - POI_SECTION_SCAN_RADIUS..=center.y + POI_SECTION_SCAN_RADIUS {
            for x in center.x - POI_SECTION_SCAN_RADIUS..=center.x + POI_SECTION_SCAN_RADIUS {
                let section_pos = BlockPos::new(x, y, z);
                if level.sections_to_village(section_pos) == 0 {
                    village_sections.push(section_pos);
                }
            }
        }
    }
    if village_sections.is_empty() {
        return None;
    }
    let index = level.random.next_int_bounded(village_sections.len() as i32) as usize;
    Some(village_sections[index])
}

fn get_random_poi_within_section(level: &mut ServerLevel, section_pos: BlockPos) -> Option<BlockPos> {
    let center = BlockPos::new((section_pos.x << 4) + 8, (section_pos.y << 4) + 8, (section_pos.z << 4) + 8);
    let pois: Vec<BlockPos> = level.chunks.poi_manager.get_in_range(&|_| true, center, 8, Occupancy::IsOccupied).into_iter().map(|record| record.pos).collect();
    if pois.is_empty() {
        return None;
    }
    let index = level.random.next_int_bounded(pois.len() as i32) as usize;
    Some(pois[index])
}
