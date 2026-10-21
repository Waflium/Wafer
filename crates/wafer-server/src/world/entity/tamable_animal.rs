use wafer_core::BlockPos;
use wafer_core::random::RandomSource;
use wafer_data::{Tag, game_rule};
use wafer_protocol::nbt::Nbt;

use super::ai::goal::panic_goal::PanicGoal;
use super::animal::Animal;
use super::animal::feline::cat;
use super::animal::wolf::wolf;
use super::data::{DataValue, ageable_mob_defaults};
use super::entity_reference::EntityReference;
use super::mob::{Mob, MobKind};
use super::{Entity, EntityId};
use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::item::component::values::FoodProperties;
use crate::world::level::BlockGetter;
use crate::world::level::path_navigation_region::PathNavigationRegion;
use crate::world::level::pathfinder::path_type::PathType;
use crate::world::level::pathfinder::pathfinding_context::PathfindingContext;
use crate::world::level::pathfinder::walk_node_evaluator;
use crate::world::scores::player_team::PlayerTeam;
use crate::world::InteractionHand;

pub const TELEPORT_WHEN_DISTANCE_IS_SQ: f64 = 144.0;
pub const DATA_FLAGS_ID: usize = 18;
pub const DATA_OWNERUUID_ID: usize = 19;

#[derive(Default)]
pub struct TamableAnimal {
    pub animal: Animal,
    pub ordered_to_sit: bool,
}

pub fn tamable_animal_defaults() -> Vec<DataValue> {
    let mut data = ageable_mob_defaults();
    data.extend([DataValue::Byte(0), DataValue::OptionalLivingEntityReference(None)]);
    data
}

fn flags(entity: &Entity) -> i8 {
    match entity.data.get(DATA_FLAGS_ID) {
        DataValue::Byte(flags) => *flags,
        _ => 0,
    }
}

pub fn is_tame(entity: &Entity) -> bool {
    flags(entity) & 4 != 0
}

pub fn set_tame(mob: &mut Mob, is_tame: bool, include_side_effects: bool) {
    let current = flags(&mob.entity);
    mob.entity.data.set(DATA_FLAGS_ID, DataValue::Byte(if is_tame { current | 4 } else { current & -5 }));
    if include_side_effects {
        apply_taming_side_effects(mob);
    }
    if let MobKind::Cat(_) = mob.kind {
        cat::reassess_tame_goals(mob);
    }
}

fn apply_taming_side_effects(mob: &mut Mob) {
    if let MobKind::Wolf(_) = mob.kind {
        wolf::apply_taming_side_effects(mob);
    }
}

pub fn is_in_sitting_pose(entity: &Entity) -> bool {
    flags(entity) & 1 != 0
}

pub fn set_in_sitting_pose(entity: &mut Entity, value: bool) {
    let current = flags(entity);
    entity.data.set(DATA_FLAGS_ID, DataValue::Byte(if value { current | 1 } else { current & -2 }));
}

pub fn get_owner_reference(entity: &Entity) -> Option<EntityReference> {
    match entity.data.get(DATA_OWNERUUID_ID) {
        DataValue::OptionalLivingEntityReference(owner) => owner.map(EntityReference::of),
        _ => None,
    }
}

pub fn set_owner_reference(entity: &mut Entity, owner: Option<EntityReference>) {
    entity.data.set(DATA_OWNERUUID_ID, DataValue::OptionalLivingEntityReference(owner.map(EntityReference::get_uuid)));
}

pub fn tamable_animal(mob: &Mob) -> &TamableAnimal {
    mob.tamable_animal().expect("a tamable animal")
}

pub fn tamable_animal_mut(mob: &mut Mob) -> &mut TamableAnimal {
    mob.tamable_animal_mut().expect("a tamable animal")
}

impl TamableAnimal {
    pub fn add_additional_save_data(mob: &Mob, output: &mut Vec<(String, Nbt)>) {
        EntityReference::store(get_owner_reference(&mob.entity), output, "Owner");
        output.push(("Sitting".into(), Nbt::Byte(tamable_animal(mob).ordered_to_sit as i8)));
    }

    pub fn read_additional_save_data(mob: &mut Mob, input: &Nbt) {
        match EntityReference::read(input, "Owner") {
            Some(owner) => {
                set_owner_reference(&mut mob.entity, Some(owner));
                set_tame(mob, true, false);
            }
            None => {
                set_owner_reference(&mut mob.entity, None);
                set_tame(mob, false, true);
            }
        }
        let ordered_to_sit = input.get("Sitting").and_then(Nbt::as_i64).is_some_and(|v| v != 0);
        tamable_animal_mut(mob).ordered_to_sit = ordered_to_sit;
        set_in_sitting_pose(&mut mob.entity, ordered_to_sit);
    }
}

pub struct TamableAnimalPanicGoal {
    pub panic_goal: PanicGoal,
}

impl TamableAnimalPanicGoal {
    pub fn new(speed_modifier: f64, panic_causing_damage_types: fn(&ServerLevel, EntityId) -> &'static Tag) -> TamableAnimalPanicGoal {
        TamableAnimalPanicGoal { panic_goal: PanicGoal::with_damage_types(speed_modifier, panic_causing_damage_types) }
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        if !level.unable_to_move_to_owner(mob) && level.should_try_teleport_to_owner(mob) {
            level.try_to_teleport_to_owner(mob);
        }
    }
}

impl ServerLevel {
    pub fn is_tamable_animal(&self, id: EntityId) -> bool {
        self.mob(id).is_some_and(|mob| mob.tamable_animal().is_some())
    }

    pub fn is_tame(&self, id: EntityId) -> bool {
        is_tame(self.entity(id))
    }

    pub fn set_tame(&mut self, id: EntityId, is_tame: bool, include_side_effects: bool) {
        set_tame(self.mob_mut(id).expect("a tamable animal"), is_tame, include_side_effects);
    }

    pub fn is_in_sitting_pose(&self, id: EntityId) -> bool {
        is_in_sitting_pose(self.entity(id))
    }

    pub fn set_in_sitting_pose(&mut self, id: EntityId, value: bool) {
        set_in_sitting_pose(self.entity_mut(id), value);
    }

    pub fn is_ordered_to_sit(&self, id: EntityId) -> bool {
        tamable_animal(self.mob(id).expect("a tamable animal")).ordered_to_sit
    }

    pub fn set_ordered_to_sit(&mut self, id: EntityId, ordered_to_sit: bool) {
        tamable_animal_mut(self.mob_mut(id).expect("a tamable animal")).ordered_to_sit = ordered_to_sit;
    }

    pub fn set_owner(&mut self, id: EntityId, owner: Option<EntityId>) {
        let owner = owner.map(|owner| EntityReference::of(self.entity(owner).uuid));
        set_owner_reference(self.entity_mut(id), owner);
    }

    pub fn tamable_animal_feed(&mut self, id: EntityId, player: EntityId, hand: InteractionHand, item_stack: &mut ItemStack, healing_factor: f32, default_heal: f32) {
        let food_properties = item_stack.get(dc::FOOD).map(FoodProperties::from_nbt);
        self.use_player_item(player, hand, item_stack);
        self.heal(id, food_properties.map_or(default_heal, |food_properties| healing_factor * food_properties.nutrition as f32));
        self.play_eating_sound(id);
    }

    pub fn tame(&mut self, id: EntityId, player: EntityId) {
        self.set_tame(id, true, true);
        self.set_owner(id, Some(player));
        if self.is_player(player) {
            crate::advancements::triggers::criteria_triggers::TAME_ANIMAL.trigger(self, player, id);
        }
    }

    pub fn tamable_animal_can_attack(&self, id: EntityId, target: EntityId) -> bool {
        if self.is_owned_by(id, target) { false } else { self.mob_can_attack(id, target) }
    }

    pub fn is_owned_by(&self, id: EntityId, entity: EntityId) -> bool {
        self.get_owner(id) == Some(entity)
    }

    pub fn wants_to_attack(&self, id: EntityId, target: EntityId, owner: EntityId) -> bool {
        match self.mob(id).map(|mob| &mob.kind) {
            Some(MobKind::Wolf(_)) => self.wolf_wants_to_attack(target, owner),
            _ => true,
        }
    }

    pub fn tamable_animal_get_team(&self, id: EntityId) -> Option<std::cell::Ref<'_, PlayerTeam>> {
        if self.scoreboard.borrow().team_count() == 0 {
            return None;
        }
        if let Some(own_team) = self.entity_get_team(id) {
            return Some(own_team);
        }
        if self.is_tame(id)
            && let Some(owner) = self.get_root_owner(id)
        {
            return self.team_of(owner);
        }
        None
    }

    pub fn tamable_animal_considers_entity_as_ally(&self, id: EntityId, other: EntityId) -> bool {
        if self.is_tame(id) {
            let owner = self.get_root_owner(id);
            if Some(other) == owner {
                return true;
            }
            if let Some(owner) = owner {
                return self.considers_entity_as_ally(owner, other);
            }
        }
        self.entity_considers_entity_as_ally(id, other)
    }

    pub fn tamable_animal_die(&mut self, id: EntityId, source: DamageSource) {
        if self.game_rules.get_bool(game_rule::SHOW_DEATH_MESSAGES)
            && let Some(owner) = self.get_owner(id).filter(|&owner| self.is_player(owner))
        {
            let message = self.death_message(id);
            self.send_system_message(owner, &message);
        }
        self.living_entity_die(id, source);
    }

    pub fn try_to_teleport_to_owner(&mut self, id: EntityId) {
        if let Some(owner) = self.get_owner(id) {
            let target_pos = self.entity(owner).block_position();
            self.teleport_to_around_block_pos(id, target_pos);
        }
    }

    pub fn should_try_teleport_to_owner(&self, id: EntityId) -> bool {
        self.get_owner(id).is_some_and(|owner| self.entity(id).distance_to_sqr(self.entity(owner)) >= TELEPORT_WHEN_DISTANCE_IS_SQ)
    }

    fn teleport_to_around_block_pos(&mut self, id: EntityId, target_pos: BlockPos) {
        for _ in 0..10 {
            let random = &mut self.entity_mut(id).random;
            let xd = random.next_int_between_inclusive(-3, 3);
            let zd = random.next_int_between_inclusive(-3, 3);
            if xd.abs() >= 2 || zd.abs() >= 2 {
                let yd = self.entity_mut(id).random.next_int_between_inclusive(-1, 1);
                if self.maybe_teleport_to(id, target_pos.x + xd, target_pos.y + yd, target_pos.z + zd) {
                    return;
                }
            }
        }
    }

    fn maybe_teleport_to(&mut self, id: EntityId, x: i32, y: i32, z: i32) -> bool {
        if !self.can_teleport_to(id, BlockPos::new(x, y, z)) {
            return false;
        }
        let e = self.entity_mut(id);
        let (y_rot, x_rot) = (e.y_rot, e.x_rot);
        e.snap_to(x as f64 + 0.5, y as f64, z as f64 + 0.5, y_rot, x_rot);
        self.entity_moved(id);
        self.get_navigation(id, |navigation, _| navigation.stop());
        true
    }

    fn can_teleport_to(&mut self, id: EntityId, pos: BlockPos) -> bool {
        let path_type = walk_node_evaluator::get_path_type_static(&PathfindingContext::new(PathNavigationRegion::new(self), id), pos.x, pos.y, pos.z);
        if path_type != PathType::Walkable {
            return false;
        }
        let block_state_below = self.block_state(pos.below());
        if !self.can_fly_to_owner(id) && block_state_below.block().is_instance_of("LeavesBlock") {
            return false;
        }
        let e = self.entity(id);
        let block_position = e.block_position();
        let delta = (pos.x - block_position.x, pos.y - block_position.y, pos.z - block_position.z);
        self.no_collision_for(id, &e.bounding_box().move_by(delta.0 as f64, delta.1 as f64, delta.2 as f64))
    }

    pub fn unable_to_move_to_owner(&self, id: EntityId) -> bool {
        self.is_ordered_to_sit(id) || self.is_passenger(id) || self.may_be_leashed(id) || self.get_owner(id).is_some_and(|owner| self.is_spectator(owner))
    }

    fn can_fly_to_owner(&self, _id: EntityId) -> bool {
        false
    }
}
