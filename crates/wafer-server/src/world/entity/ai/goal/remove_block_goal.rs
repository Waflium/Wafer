use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, Vec3};
use wafer_data::{Block, game_rule};
use wafer_protocol::nbt::Nbt;

use super::goal::reduced_tick_delay;
use super::move_to_block_goal::{self, MoveToBlockGoal};
use crate::core::particles::particle_options::ParticleOptions;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::level::BlockGetter;

const WAIT_AFTER_BLOCK_FOUND: i32 = 20;

pub trait RemoveBlockSounds {
    fn play_destroy_progress_sound(&self, level: &mut ServerLevel, mob: EntityId, pos: BlockPos);

    fn play_break_sound(&self, level: &mut ServerLevel, pos: BlockPos);
}

pub struct RemoveBlockGoal {
    pub move_to_block_goal: MoveToBlockGoal,
    block_to_remove: Block,
    ticks_since_reached_goal: i32,
}

impl RemoveBlockGoal {
    pub fn new(block_to_remove: Block, speed_modifier: f64, vertical_search_range: i32) -> RemoveBlockGoal {
        RemoveBlockGoal { move_to_block_goal: MoveToBlockGoal::new(speed_modifier, 24, vertical_search_range), block_to_remove, ticks_since_reached_goal: 0 }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if !level.game_rules.get_bool(game_rule::MOB_GRIEFING) {
            return false;
        }
        if self.move_to_block_goal.next_start_tick > 0 {
            self.move_to_block_goal.next_start_tick -= 1;
            return false;
        }
        let block_to_remove = self.block_to_remove;
        if self.move_to_block_goal.find_nearest_block(level, mob, &|level, pos| is_valid_target(level, block_to_remove, pos)) {
            self.move_to_block_goal.next_start_tick = reduced_tick_delay(WAIT_AFTER_BLOCK_FOUND);
            true
        } else {
            self.move_to_block_goal.next_start_tick = move_to_block_goal::next_start_tick(level, mob);
            false
        }
    }

    pub fn can_continue_to_use(&self, level: &ServerLevel) -> bool {
        let block_to_remove = self.block_to_remove;
        self.move_to_block_goal.can_continue_to_use(level, &|level, pos| is_valid_target(level, block_to_remove, pos))
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        level.entity_mut(mob).fall_distance = 1.0;
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.move_to_block_goal.start(level, mob);
        self.ticks_since_reached_goal = 0;
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId, accepted_distance: f64, sounds: &dyn RemoveBlockSounds) {
        self.move_to_block_goal.tick(level, mob, accepted_distance);
        let mob_pos = level.entity(mob).block_position();
        let eat_pos = self.get_pos_with_block(level, mob_pos);
        let Some(eat_pos) = eat_pos.filter(|_| self.move_to_block_goal.is_reached_target()) else { return };
        if self.ticks_since_reached_goal > 0 {
            let e = level.entity_mut(mob);
            let movement = e.delta_movement;
            e.set_delta_movement(Vec3::new(movement.x, 0.3, movement.z));
            let random = &mut level.entity_mut(mob).random;
            let xd = ((random.next_float() - 0.5) * 0.08) as f64;
            let yd = ((random.next_float() - 0.5) * 0.08) as f64;
            let zd = ((random.next_float() - 0.5) * 0.08) as f64;
            let particle = ParticleOptions { particle_type: "minecraft:item", options: Nbt::Compound(vec![("item".into(), Nbt::Compound(vec![("id".into(), Nbt::String("minecraft:egg".into()))].into()))].into()) };
            level.send_particles("minecraft:item", |w| particle.write_options(w), eat_pos.x as f64 + 0.5, eat_pos.y as f64 + 0.7, eat_pos.z as f64 + 0.5, 3, (xd, yd, zd), 0.15f32 as f64);
        }
        if self.ticks_since_reached_goal % 2 == 0 {
            let e = level.entity_mut(mob);
            let movement = e.delta_movement;
            e.set_delta_movement(Vec3::new(movement.x, -0.3, movement.z));
            if self.ticks_since_reached_goal % 6 == 0 {
                sounds.play_destroy_progress_sound(level, mob, self.move_to_block_goal.block_pos);
            }
        }
        if self.ticks_since_reached_goal > 60 {
            level.remove_block(eat_pos, false);
            for _ in 0..20 {
                let random = &mut level.entity_mut(mob).random;
                let xa = random.next_gaussian() * 0.02;
                let ya = random.next_gaussian() * 0.02;
                let za = random.next_gaussian() * 0.02;
                level.send_particles("minecraft:poof", |_| {}, eat_pos.x as f64 + 0.5, eat_pos.y as f64, eat_pos.z as f64 + 0.5, 1, (xa, ya, za), 0.15f32 as f64);
            }
            sounds.play_break_sound(level, eat_pos);
        }
        self.ticks_since_reached_goal += 1;
    }

    fn get_pos_with_block(&self, level: &mut ServerLevel, pos: BlockPos) -> Option<BlockPos> {
        if level.block_state(pos).is_block(self.block_to_remove) {
            return Some(pos);
        }
        let neighbours = [pos.below(), pos.west(), pos.east(), pos.north(), pos.south(), pos.below().below()];
        neighbours.into_iter().find(|&neighbor_pos| level.block_state(neighbor_pos).is_block(self.block_to_remove))
    }
}

fn is_valid_target(level: &ServerLevel, block_to_remove: Block, pos: BlockPos) -> bool {
    match level.chunks.chunk_without_loading(pos.chunk()) {
        Some(chunk) => chunk.block_state(pos).is_block(block_to_remove) && chunk.block_state(pos.above()).is_air() && chunk.block_state(pos.above().above()).is_air(),
        None => false,
    }
}
