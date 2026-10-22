use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, Vec3, mth};

use crate::world::clock::ServerClockManager;
use crate::server::level::ServerLevel;
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::entity_types;
use crate::world::entity::monster::monster;
use crate::world::level::chunk::HeightmapType;
use crate::world::level::server_level_accessor::ServerLevelAccessor;

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    SiegeTonight,
    SiegeDone,
}

pub struct VillageSiege {
    has_setup_siege: bool,
    siege_state: State,
    zombies_to_spawn: i32,
    next_spawn_time: i32,
    spawn_x: i32,
    spawn_y: i32,
    spawn_z: i32,
}

impl Default for VillageSiege {
    fn default() -> VillageSiege {
        VillageSiege { has_setup_siege: false, siege_state: State::SiegeDone, zombies_to_spawn: 0, next_spawn_time: 0, spawn_x: 0, spawn_y: 0, spawn_z: 0 }
    }
}

impl VillageSiege {
    pub fn tick(&mut self, level: &mut ServerLevel, spawn_enemies: bool) {
        if level.is_bright_outside() || !spawn_enemies {
            self.siege_state = State::SiegeDone;
            self.has_setup_siege = false;
            return;
        }
        let default_clock = level.dimension_type().default_clock.and_then(ServerClockManager::id_of);
        if let Some(clock) = default_clock
            && ServerClockManager::time_marker(clock, "minecraft:roll_village_siege").is_some_and(|marker| marker.occurs_at(level.clock_ticks[clock]))
        {
            self.siege_state = if level.random.next_int_bounded(10) == 0 { State::SiegeTonight } else { State::SiegeDone };
        }
        if self.siege_state == State::SiegeDone {
            return;
        }
        if !self.has_setup_siege {
            if !self.try_to_setup_siege(level) {
                return;
            }
            self.has_setup_siege = true;
        }
        if self.next_spawn_time > 0 {
            self.next_spawn_time -= 1;
        } else {
            self.next_spawn_time = 2;
            if self.zombies_to_spawn > 0 {
                self.try_spawn(level);
                self.zombies_to_spawn -= 1;
            } else {
                self.siege_state = State::SiegeDone;
            }
        }
    }

    fn try_to_setup_siege(&mut self, level: &mut ServerLevel) -> bool {
        let players: Vec<_> = level.players.keys().copied().collect();
        for player in players {
            if level.is_spectator(player) {
                continue;
            }
            let center = level.entity(player).block_position();
            if level.is_village(center) && !level.get_biome(center).is("minecraft:without_zombie_sieges") {
                for _ in 0..10 {
                    let angle = level.random.next_float() * std::f32::consts::TAU;
                    self.spawn_x = center.x + mth::floor_f(mth::cos(angle as f64) * 32.0);
                    self.spawn_y = center.y;
                    self.spawn_z = center.z + mth::floor_f(mth::sin(angle as f64) * 32.0);
                    if self.find_random_spawn_pos(level, BlockPos::new(self.spawn_x, self.spawn_y, self.spawn_z)).is_some() {
                        self.next_spawn_time = 0;
                        self.zombies_to_spawn = 20;
                        break;
                    }
                }
                return true;
            }
        }
        false
    }

    fn try_spawn(&mut self, level: &mut ServerLevel) {
        let Some(spawn_pos) = self.find_random_spawn_pos(level, BlockPos::new(self.spawn_x, self.spawn_y, self.spawn_z)) else { return };
        let Some(zombie) = ServerLevel::create_entity("minecraft:zombie") else { return };
        let difficulty = level.current_difficulty_at(zombie.entity().block_position());
        let (mut zombie, _) = level.finalize_spawn_detached(zombie, &difficulty, EntitySpawnReason::Event, None);
        let y_rot = level.random.next_float() * 360.0;
        zombie.entity_mut().snap_to(spawn_pos.x, spawn_pos.y, spawn_pos.z, y_rot, 0.0);
        level.add_fresh_entity_with_passengers(zombie);
    }

    fn find_random_spawn_pos(&mut self, level: &mut ServerLevel, pos: BlockPos) -> Option<Vec3> {
        for _ in 0..10 {
            let x = pos.x + level.random.next_int_bounded(16) - 8;
            let z = pos.z + level.random.next_int_bounded(16) - 8;
            let y = level.get_height(HeightmapType::WorldSurface, x, z);
            let offset = BlockPos::new(x, y, z);
            if level.is_village(offset) && monster::check_monster_spawn_rules(&entity_types::ZOMBIE, level, EntitySpawnReason::Event, offset) {
                return Some(Vec3::at_bottom_center_of(offset));
            }
        }
        None
    }
}
