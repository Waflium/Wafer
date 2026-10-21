use wafer_core::Vec3;
use wafer_core::random::RandomSource;
use wafer_data::{EquipmentSlot, items, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use crate::server::level::ServerLevel;
use crate::world::difficulty_instance::DifficultyInstance;
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::goal::avoid_entity_goal::AvoidEntityGoal;
use crate::world::entity::ai::goal::float_goal::FloatGoal;
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::ai::goal::look_at_player_goal::LookAtPlayerGoal;
use crate::world::entity::ai::goal::random_stroll_goal::RandomStrollGoal;
use crate::world::entity::ai::goal::ranged_crossbow_attack_goal::RangedCrossbowAttackGoal;
use crate::world::entity::ai::goal::target::hurt_by_target_goal::HurtByTargetGoal;
use crate::world::entity::ai::goal::target::nearest_attackable_target_goal::{NearestAttackableTargetGoal, TargetType};
use crate::world::entity::attributes::PILLAGER_ATTRIBUTES;
use crate::world::entity::data::{DataValue, mob_defaults};
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::monster::monster;
use crate::world::entity::npc::inventory_carrier;
use crate::world::entity::raid::raider::{self, HoldGroundAttackGoal, RAIDER_TYPES, Raider};
use crate::world::entity::spawn_group_data::SpawnGroupData;
use crate::world::entity::{Entity, EntityDimensions, EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::item::enchantment::enchantment_helper;
use crate::world::item::enchantment::providers::vanilla_enchantment_providers;
use crate::world::simple_container::{SimpleContainer, SimpleContainerKind};

pub const IS_CHARGING_CROSSBOW: usize = 17;
const INVENTORY_SIZE: usize = 5;
pub const SLOT_OFFSET: i32 = 300;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.6, 1.95)
    .with_passenger_attachments(&[Vec3::new(0.0, 2.0, 0.0)])
    .with_vehicle_attachment(&[Vec3::new(0.0, 0.6f32 as f64, 0.0)]);

pub struct Pillager {
    pub raider: Raider,
    pub inventory: SimpleContainer,
}

pub fn pillager(mob: &Mob) -> &Pillager {
    match &mob.kind {
        MobKind::Pillager(pillager) => pillager,
        _ => unreachable!("a pillager"),
    }
}

pub fn pillager_mut(mob: &mut Mob) -> &mut Pillager {
    match &mut mob.kind {
        MobKind::Pillager(pillager) => pillager,
        _ => unreachable!("a pillager"),
    }
}

impl Pillager {
    pub fn create() -> Mob {
        let mut data = mob_defaults();
        data.push(DataValue::Boolean(false));
        data.push(DataValue::Boolean(false));
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:pillager"), DIMENSIONS, data);
        let pillager = Pillager { raider: Raider::default(), inventory: SimpleContainer::new(SimpleContainerKind::Plain, INVENTORY_SIZE) };
        let mut mob = Mob::new(entity, LivingEntity::new(&PILLAGER_ATTRIBUTES), MobKind::Pillager(pillager));
        mob.xp_reward = monster::XP_REWARD;
        mob
    }

    pub fn register_goals(ai: &mut MobAi) {
        Raider::register_goals(ai);
        let float_goal = FloatGoal::new(&mut ai.navigation);
        let goal_selector = &mut ai.goal_selector;
        goal_selector.add_goal(0, Goal::Float(float_goal));
        goal_selector.add_goal(1, Goal::AvoidEntity(AvoidEntityGoal::new("minecraft:creaking", 8.0, 1.0, 1.2)));
        goal_selector.add_goal(2, Goal::HoldGroundAttack(HoldGroundAttackGoal::new(10.0)));
        goal_selector.add_goal(3, Goal::RangedCrossbowAttack(RangedCrossbowAttackGoal::new(1.0, 8.0)));
        goal_selector.add_goal(8, Goal::RandomStroll(RandomStrollGoal::new(0.6)));
        goal_selector.add_goal(9, Goal::LookAtPlayer(LookAtPlayerGoal::new(15.0).with_probability(1.0)));
        goal_selector.add_goal(10, Goal::LookAtPlayer(LookAtPlayerGoal::of_type(TargetType::Mob, 15.0)));
        let target_selector = &mut ai.target_selector;
        target_selector.add_goal(1, Goal::HurtByTarget(HurtByTargetGoal::new(RAIDER_TYPES).set_alert_others(&[])));
        target_selector.add_goal(2, Goal::NearestAttackableTarget(NearestAttackableTargetGoal::new(TargetType::Player, true)));
        target_selector.add_goal(3, Goal::NearestAttackableTarget(NearestAttackableTargetGoal::new(TargetType::AbstractVillager, false)));
        target_selector.add_goal(3, Goal::NearestAttackableTarget(NearestAttackableTargetGoal::new(TargetType::IronGolem, true)));
    }

    pub fn can_use_non_melee_weapon(item: &ItemStack) -> bool {
        item.get_item() == items::CROSSBOW
    }

    pub fn get_preferred_weapon_type() -> &'static wafer_data::Tag {
        &wafer_data::item_tags::PILLAGER_PREFERRED_WEAPONS
    }

    pub fn add_additional_save_data(mob: &Mob, out: &mut Vec<(String, Nbt)>) {
        let pillager = pillager(mob);
        pillager.raider.add_additional_save_data(out);
        inventory_carrier::write_inventory_to_tag(&pillager.inventory, out);
    }

    pub fn read_additional_save_data(mob: &mut Mob, input: &Nbt) {
        let pillager = pillager_mut(mob);
        pillager.raider.read_additional_save_data(input);
        inventory_carrier::read_inventory_from_tag(&mut pillager.inventory, input);
        mob.can_pick_up_loot = true;
    }

    pub fn get_walk_target_value() -> f32 {
        0.0
    }

    pub fn get_max_spawn_cluster_size() -> i32 {
        1
    }

    pub fn get_ambient_sound() -> &'static str {
        "minecraft:entity.pillager.ambient"
    }

    pub fn get_death_sound() -> &'static str {
        "minecraft:entity.pillager.death"
    }

    pub fn get_hurt_sound() -> &'static str {
        "minecraft:entity.pillager.hurt"
    }

    pub fn get_celebrate_sound() -> &'static str {
        "minecraft:entity.pillager.celebrate"
    }
}

impl ServerLevel {
    pub fn set_charging_crossbow(&mut self, id: EntityId, is_charging: bool) {
        if let Some(mob) = self.mob_mut(id)
            && let MobKind::Pillager(_) = mob.kind
        {
            mob.entity.data.set(IS_CHARGING_CROSSBOW, DataValue::Boolean(is_charging));
        }
    }

    pub fn pillager_on_crossbow_attack_performed(&mut self, id: EntityId) {
        self.living_mut(id).no_action_time = 0;
    }

    pub fn pillager_finalize_spawn(&mut self, id: EntityId, difficulty: &DifficultyInstance, spawn_reason: EntitySpawnReason, group_data: Option<SpawnGroupData>) -> Option<SpawnGroupData> {
        self.pillager_populate_default_equipment_slots(id);
        self.populate_default_equipment_enchantments(id, difficulty);
        let (index, _, entity) = self.entities.shift_remove_full(&id).expect("the pillager to finalize");
        let LevelEntity::Mob(mut mob) = entity else { unreachable!("a mob") };
        let spawn_group_data = raider::finalize_spawn(self, &mut mob, difficulty, spawn_reason, group_data);
        self.entities.shift_insert(index, id, LevelEntity::Mob(mob));
        spawn_group_data
    }

    pub fn pillager_populate_default_equipment_slots(&mut self, id: EntityId) {
        self.living_set_item_slot(id, EquipmentSlot::Mainhand, ItemStack::new(items::CROSSBOW, 1));
    }

    pub fn pillager_enchant_spawned_weapon(&mut self, id: EntityId, difficulty: &DifficultyInstance) {
        self.mob_enchant_spawned_weapon(id, difficulty);
        if self.random.next_int_bounded(300) == 0
            && let Some(mut weapon) = self.item_by_slot(id, EquipmentSlot::Mainhand)
            && weapon.is(items::CROSSBOW)
        {
            enchantment_helper::enchant_item_from_provider(&mut weapon, &vanilla_enchantment_providers::PILLAGER_SPAWN_CROSSBOW, difficulty, &mut self.random);
            self.set_item_slot(id, EquipmentSlot::Mainhand, weapon);
        }
    }

    pub fn pillager_perform_ranged_attack(&mut self, id: EntityId) {
        self.perform_crossbow_attack(id, 1.6);
    }

    pub fn perform_crossbow_attack(&mut self, body: EntityId, crossbow_power: f32) {
        let hand = crate::world::entity::projectile::projectile_util::get_weapon_holding_hand(self, body, items::CROSSBOW);
        if self.get_item_in_hand(body, hand).is_some_and(|used_item| used_item.get_item().is_instance_of("CrossbowItem")) {
            let uncertainty = self.ranged_attack_uncertainty(body);
            let target = self.get_target(body);
            crate::world::item::crossbow_item::perform_shooting(self, body, hand, crossbow_power, uncertainty, target);
        }
        self.pillager_on_crossbow_attack_performed(body);
    }

    pub fn pillager_pick_up_item(&mut self, id: EntityId, entity: EntityId) {
        let Some(LevelEntity::Item(item)) = self.entities.get(&entity) else { return };
        let item_stack = item.item.clone();
        if item_stack.get_item().is_instance_of("BannerItem") {
            self.raider_pick_up_item(id, entity);
        } else if self.pillager_wants_item(id, &item_stack) {
            self.on_item_pickup(id, entity);
            let remainder = pillager_mut(self.mob_mut(id).expect("a pillager")).inventory.add_item(&item_stack);
            if remainder.is_empty() {
                self.discard(entity);
            } else if let Some(LevelEntity::Item(item)) = self.entities.get_mut(&entity) {
                item.item.count = remainder.count;
            }
        }
    }

    fn pillager_wants_item(&self, id: EntityId, item_stack: &ItemStack) -> bool {
        self.raider(id).has_active_raid() && item_stack.is(items::WHITE_BANNER)
    }
}
