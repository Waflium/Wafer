use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, Direction};
use wafer_data::Tag;

use super::behavior::{Behavior, BehaviorState};
use super::long_jump_to_random_pos::{self, LongJumpToRandomPos, PossibleJump};
use crate::server::level::ServerLevel;
use crate::util::valueproviders::IntProvider;
use crate::world::entity::EntityId;
use crate::world::level::BlockGetter;

pub struct LongJumpToPreferredBlock {
    long_jump_to_random_pos: LongJumpToRandomPos,
    preferred_block_tag: &'static Tag,
    preferred_blocks_chance: f32,
    not_preffered_jump_candidates: Vec<PossibleJump>,
    currently_wanting_preferred_ones: bool,
}

impl LongJumpToPreferredBlock {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        time_between_long_jumps: IntProvider,
        max_long_jump_height: i32,
        max_long_jump_width: i32,
        max_jump_velocity: f32,
        get_jump_sound: fn(&ServerLevel, EntityId) -> &'static str,
        preferred_block_tag: &'static Tag,
        preferred_blocks_chance: f32,
        acceptable_landing_spot: fn(&mut ServerLevel, EntityId, BlockPos) -> bool,
    ) -> LongJumpToPreferredBlock {
        LongJumpToPreferredBlock {
            long_jump_to_random_pos: LongJumpToRandomPos::with_landing_spot(time_between_long_jumps, max_long_jump_height, max_long_jump_width, max_jump_velocity, get_jump_sound, acceptable_landing_spot),
            preferred_block_tag,
            preferred_blocks_chance,
            not_preffered_jump_candidates: Vec::new(),
            currently_wanting_preferred_ones: false,
        }
    }
}

fn get_jump_candidate(
    jump_candidates: &mut Vec<PossibleJump>,
    level: &mut ServerLevel,
    currently_wanting_preferred_ones: bool,
    preferred_block_tag: &Tag,
    not_preffered_jump_candidates: &mut Vec<PossibleJump>,
) -> Option<PossibleJump> {
    if !currently_wanting_preferred_ones {
        return long_jump_to_random_pos::get_jump_candidate(jump_candidates, level);
    }
    while !jump_candidates.is_empty() {
        if let Some(possible_jump) = long_jump_to_random_pos::get_jump_candidate(jump_candidates, level) {
            if level.block_state(possible_jump.target_pos.relative(Direction::Down)).is(preferred_block_tag) {
                return Some(possible_jump);
            }
            not_preffered_jump_candidates.push(possible_jump);
        }
    }
    if !not_preffered_jump_candidates.is_empty() { Some(not_preffered_jump_candidates.remove(0)) } else { None }
}

impl Behavior for LongJumpToPreferredBlock {
    fn behavior(&self) -> &BehaviorState {
        self.long_jump_to_random_pos.behavior()
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        self.long_jump_to_random_pos.behavior_mut()
    }

    fn check_extra_start_conditions(&mut self, level: &mut ServerLevel, body: EntityId) -> bool {
        self.long_jump_to_random_pos.check_extra_start_conditions(level, body)
    }

    fn can_still_use(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) -> bool {
        self.long_jump_to_random_pos.can_still_use(level, body, timestamp)
    }

    fn start(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) {
        self.long_jump_to_random_pos.start(level, body, timestamp);
        self.not_preffered_jump_candidates.clear();
        self.currently_wanting_preferred_ones = level.entity_mut(body).random.next_float() < self.preferred_blocks_chance;
    }

    fn tick(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) {
        let (currently_wanting_preferred_ones, preferred_block_tag) = (self.currently_wanting_preferred_ones, self.preferred_block_tag);
        let not_preffered_jump_candidates = &mut self.not_preffered_jump_candidates;
        self.long_jump_to_random_pos.tick_with(level, body, timestamp, &mut |jump_candidates, level| {
            get_jump_candidate(jump_candidates, level, currently_wanting_preferred_ones, preferred_block_tag, not_preffered_jump_candidates)
        });
    }
}
