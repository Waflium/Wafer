use wafer_core::random::RandomSource;
use wafer_data::items;
use wafer_protocol::nbt::Nbt;

use super::behavior::{Behavior, BehaviorState};
use super::move_to_sky_seeing_spot;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::brain::Brain;
use crate::world::entity::projectile::firework_rocket_entity::FireworkRocketEntity;
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::item::dye_color::DyeColor;

pub struct CelebrateVillagersSurvivedRaid {
    behavior: BehaviorState,
    has_current_raid: bool,
}

impl CelebrateVillagersSurvivedRaid {
    pub fn new(min_duration: i32, max_duration: i32) -> CelebrateVillagersSurvivedRaid {
        CelebrateVillagersSurvivedRaid { behavior: BehaviorState::with_durations(Vec::new(), min_duration, max_duration), has_current_raid: false }
    }

    fn get_firework(color: DyeColor, flight_duration: i32) -> ItemStack {
        let mut rocket = ItemStack::new(items::FIREWORK_ROCKET, 1);
        let explosion = Nbt::Compound(vec![("shape".into(), Nbt::String("burst".into())), ("colors".into(), Nbt::IntArray(vec![color.get_firework_color()]))].into());
        let fireworks = Nbt::Compound(vec![("flight_duration".into(), Nbt::Byte(flight_duration as i8)), ("explosions".into(), Nbt::List(vec![explosion]))].into());
        let fireworks = dc::FIREWORKS.decode(&fireworks).unwrap_or(fireworks);
        rocket.set(dc::FIREWORKS, Some(fireworks));
        rocket
    }
}

impl Behavior for CelebrateVillagersSurvivedRaid {
    fn behavior(&self) -> &BehaviorState {
        &self.behavior
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.behavior
    }

    fn check_extra_start_conditions(&mut self, level: &mut ServerLevel, body: EntityId) -> bool {
        let test_pos = level.entity(body).block_position();
        let victory = level.get_raid_at(test_pos).map(|raid| raid.is_victory());
        self.has_current_raid = victory.is_some();
        victory == Some(true) && move_to_sky_seeing_spot::has_no_blocks_above(level, body, test_pos)
    }

    fn can_still_use(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) -> bool {
        let pos = level.entity(body).block_position();
        self.has_current_raid && level.get_raid_at(pos).is_some_and(|raid| !raid.is_stopped())
    }

    fn stop(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        self.has_current_raid = false;
        let (game_time, position) = (level.game_time, level.entity(body).position());
        Brain::update_activity_from_schedule(level, body, game_time, position);
    }

    fn tick(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        if level.entity_mut(body).random.next_int_bounded(100) == 0 {
            level.play_celebrate_sound(body);
        }
        let pos = level.entity(body).block_position();
        if level.entity_mut(body).random.next_int_bounded(200) == 0 && move_to_sky_seeing_spot::has_no_blocks_above(level, body, pos) {
            let random = &mut level.entity_mut(body).random;
            let color = DyeColor::VALUES[random.next_int_bounded(DyeColor::VALUES.len() as i32) as usize];
            let flight_duration = random.next_int_bounded(3);
            let firework = CelebrateVillagersSurvivedRaid::get_firework(color, flight_duration);
            let e = level.entity(body);
            let projectile = FireworkRocketEntity::with_owner(Some(e.uuid), e.x(), e.eye_y(), e.z(), &firework);
            level.spawn_projectile(projectile, &firework, |_| {});
        }
    }
}
