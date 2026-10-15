use wafer_core::random::RandomSource;
use wafer_core::{Vec3, mth};
use wafer_data::{item_tags, items, mob_effects, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use super::abstract_villager::{self, AbstractVillager};
use super::villager_data::{self, VillagerData};
use super::villager_profession::ProfessionId;
use super::villager_type::{self, VillagerType};
use crate::core::global_pos::GlobalPos;
use crate::network::chat::Component;
use crate::server::level::ServerLevel;
use crate::stats::stats;
use crate::util::spawn_util::{self, Strategy};
use crate::world::InteractionHand;
use crate::world::InteractionResult;
use crate::world::attribute::environment_attributes;
use crate::world::damagesource::DamageSource;
use crate::world::difficulty::Difficulty;
use crate::world::effect::MobEffectInstance;
use crate::world::entity::ai::activity_data::ActivityData;
use crate::world::entity::ai::behavior::villager_goal_packages;
use crate::world::entity::ai::brain::{self, Brain, Provider};
use crate::world::entity::ai::gossip::gossip_container::GossipContainer;
use crate::world::entity::ai::gossip::gossip_type::GossipType;
use crate::world::entity::ai::memory::memory_module_type::{self, MemoryModuleType};
use crate::world::entity::ai::sensing::golem_sensor;
use crate::world::entity::ai::sensing::sensor_type::SensorType;
use crate::world::entity::ai::village::poi::poi_types::PoiType;
use crate::world::entity::ai::village::reputation_event_type::ReputationEventType;
use crate::world::entity::attributes::VILLAGER_ATTRIBUTES;
use crate::world::entity::conversion_params::ConversionParams;
use crate::world::entity::data::{DataValue, ageable_mob_defaults};
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::schedule::Activity;
use crate::world::entity::spawn_group_data::SpawnGroupData;
use crate::world::entity::{Entity, EntityDimensions, EntityId, LevelEntity, entity_types, sound_event};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::item::trading::merchant_offer::MerchantOffer;
use crate::world::level::server_level_accessor::ServerLevelAccessor;

pub const DATA_VILLAGER_DATA: usize = 19;
pub const DATA_VILLAGER_DATA_FINALIZED: usize = 20;
pub const BREEDING_FOOD_THRESHOLD: i32 = 12;
const MAX_GOSSIP_TOPICS: i32 = 10;
const GOSSIP_COOLDOWN: i64 = 1200;
const GOSSIP_DECAY_INTERVAL: i64 = 24000;
const HOW_FAR_AWAY_TO_TALK_TO_OTHER_VILLAGERS_ABOUT_GOLEMS: f64 = 10.0;
const HOW_MANY_VILLAGERS_NEED_TO_AGREE_TO_SPAWN_A_GOLEM: usize = 5;
const TIME_SINCE_SLEEPING_FOR_GOLEM_SPAWNING: i64 = 24000;
pub const SPEED_MODIFIER: f32 = 0.5;
const DEFAULT_XP: i32 = 0;
const DEFAULT_FOOD_LEVEL: i8 = 0;
const DEFAULT_LAST_RESTOCK: i64 = 0;
const DEFAULT_LAST_GOSSIP_DECAY: i64 = 0;
const DEFAULT_RESTOCKS_TODAY: i32 = 0;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.6, 1.95).with_eye_height(1.62);
pub const BABY_DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.49, 0.98).with_eye_height(0.63);

pub static BRAIN_PROVIDER: Provider = brain::provider(
    &[
        SensorType::NearestLivingEntities,
        SensorType::NearestPlayers,
        SensorType::NearestItems,
        SensorType::NearestBed,
        SensorType::HurtBy,
        SensorType::VillagerHostiles,
        SensorType::VillagerBabies,
        SensorType::SecondaryPois,
        SensorType::GolemDetected,
    ],
    get_activities,
);

fn get_activities(body: &Mob) -> Vec<ActivityData> {
    let profession = get_villager_data(&body.entity).profession;
    let mut activities = Vec::new();
    if body.is_baby() {
        activities.push(ActivityData::create_pairs(Activity::PLAY, villager_goal_packages::get_play_package(SPEED_MODIFIER)));
    } else {
        activities.push(ActivityData::create_pairs_with_conditions(
            Activity::WORK,
            villager_goal_packages::get_work_package(profession, SPEED_MODIFIER),
            vec![(memory_module_type::JOB_SITE.erase(), crate::world::entity::ai::memory::memory_status::MemoryStatus::ValuePresent)],
        ));
    }
    activities.push(ActivityData::create_pairs(Activity::CORE, villager_goal_packages::get_core_package(profession, SPEED_MODIFIER)));
    activities.push(ActivityData::create_pairs_with_conditions(
        Activity::MEET,
        villager_goal_packages::get_meet_package(SPEED_MODIFIER),
        vec![(memory_module_type::MEETING_POINT.erase(), crate::world::entity::ai::memory::memory_status::MemoryStatus::ValuePresent)],
    ));
    activities.push(ActivityData::create_pairs(Activity::REST, villager_goal_packages::get_rest_package(SPEED_MODIFIER)));
    activities.push(ActivityData::create_pairs(Activity::IDLE, villager_goal_packages::get_idle_package(SPEED_MODIFIER)));
    activities.push(ActivityData::create_pairs(Activity::PANIC, villager_goal_packages::get_panic_package(SPEED_MODIFIER)));
    activities.push(ActivityData::create_pairs(Activity::PRE_RAID, villager_goal_packages::get_pre_raid_package(SPEED_MODIFIER)));
    activities.push(ActivityData::create_pairs(Activity::RAID, villager_goal_packages::get_raid_package(SPEED_MODIFIER)));
    activities.push(ActivityData::create_pairs(Activity::HIDE, villager_goal_packages::get_hide_package(SPEED_MODIFIER)));
    activities
}

pub fn poi_memory_matches(memory_type: MemoryModuleType<GlobalPos>, mob: &Mob, poi_type: PoiType) -> bool {
    if memory_type == memory_module_type::HOME {
        poi_type == PoiType::Home
    } else if memory_type == memory_module_type::JOB_SITE {
        get_villager_data(&mob.entity).profession.value().held_job_site.test(poi_type)
    } else if memory_type == memory_module_type::POTENTIAL_JOB_SITE {
        super::villager_profession::ALL_ACQUIRABLE_JOBS.test(poi_type)
    } else if memory_type == memory_module_type::MEETING_POINT {
        poi_type == PoiType::Meeting
    } else {
        false
    }
}

pub struct Villager {
    pub abstract_villager: AbstractVillager,
    last_traded_player: Option<EntityId>,
    pub food_level: i32,
    pub gossips: GossipContainer,
    last_gossip_time: i64,
    last_gossip_decay_time: i64,
    villager_xp: i32,
    last_restock_game_time: i64,
    number_of_restocks_today: i32,
    last_restock_check_day: i64,
}

impl Default for Villager {
    fn default() -> Villager {
        Villager {
            abstract_villager: AbstractVillager::default(),
            last_traded_player: None,
            food_level: DEFAULT_FOOD_LEVEL as i32,
            gossips: GossipContainer::default(),
            last_gossip_time: 0,
            last_gossip_decay_time: DEFAULT_LAST_GOSSIP_DECAY,
            villager_xp: DEFAULT_XP,
            last_restock_game_time: DEFAULT_LAST_RESTOCK,
            number_of_restocks_today: DEFAULT_RESTOCKS_TODAY,
            last_restock_check_day: 0,
        }
    }
}

pub fn villager(mob: &Mob) -> &Villager {
    match &mob.kind {
        MobKind::Villager(villager) => villager,
        _ => unreachable!("a villager"),
    }
}

pub fn villager_mut(mob: &mut Mob) -> &mut Villager {
    match &mut mob.kind {
        MobKind::Villager(villager) => villager,
        _ => unreachable!("a villager"),
    }
}

pub fn create_default_villager_data() -> VillagerData {
    VillagerData::new(villager_data::DEFAULT_TYPE, ProfessionId::NONE, 1)
}

pub fn get_villager_data(entity: &Entity) -> VillagerData {
    match entity.data.get(DATA_VILLAGER_DATA) {
        DataValue::VillagerData(data) => *data,
        _ => create_default_villager_data(),
    }
}

pub fn get_villager_data_finalized(entity: &Entity) -> bool {
    matches!(entity.data.get(DATA_VILLAGER_DATA_FINALIZED), DataValue::Boolean(true))
}

pub fn set_villager_data_finalized(entity: &mut Entity, finalized: bool) {
    entity.data.set(DATA_VILLAGER_DATA_FINALIZED, DataValue::Boolean(finalized));
}

pub fn set_villager_data(mob: &mut Mob, data: VillagerData) {
    let current_data = get_villager_data(&mob.entity);
    if current_data.profession != data.profession {
        mob.abstract_villager_mut().expect("a villager").offers = None;
    }
    mob.entity.data.set(DATA_VILLAGER_DATA, DataValue::VillagerData(data));
}

pub fn finalize_villager_type(level: &mut (impl ServerLevelAccessor + ?Sized), mob: &mut Mob, pos: wafer_core::BlockPos) {
    if !get_villager_data_finalized(&mob.entity) {
        let ty = villager_type::by_biome(level.get_biome(pos).name());
        let data = get_villager_data(&mob.entity).with_type(ty);
        set_villager_data(mob, data);
        set_villager_data_finalized(&mut mob.entity, true);
    }
}

pub fn finalize_spawn(level: &mut (impl ServerLevelAccessor + ?Sized), mob: &mut Mob, spawn_reason: EntitySpawnReason, group_data: Option<SpawnGroupData>) -> Option<SpawnGroupData> {
    if spawn_reason == EntitySpawnReason::Breeding {
        let data = get_villager_data(&mob.entity).with_profession(ProfessionId::NONE);
        set_villager_data(mob, data);
    }
    let pos = mob.entity.block_position();
    finalize_villager_type(level, mob, pos);
    abstract_villager::finalize_spawn(group_data)
}

pub fn get_ambient_sound(mob: &Mob) -> Option<&'static str> {
    if matches!(mob.entity.data.get(crate::world::entity::data::ids::SLEEPING_POS), DataValue::OptionalBlockPos(Some(_))) {
        None
    } else if mob.abstract_villager().is_some_and(|villager| villager.trading_player.is_some()) {
        Some("minecraft:entity.villager.trade")
    } else {
        Some("minecraft:entity.villager.ambient")
    }
}

pub fn get_hurt_sound() -> &'static str {
    "minecraft:entity.villager.hurt"
}

pub fn get_death_sound() -> &'static str {
    "minecraft:entity.villager.death"
}

pub fn get_type_name(mob: &Mob) -> Component {
    get_villager_data(&mob.entity).profession.value().name()
}

pub fn wants_more_food(mob: &Mob) -> bool {
    count_food_points_in_inventory(mob) < 12
}

pub fn has_excess_food(mob: &Mob) -> bool {
    count_food_points_in_inventory(mob) >= 24
}

fn villager_food_nutrition(item: &ItemStack) -> Option<i32> {
    item.get(dc::VILLAGER_FOOD).and_then(|food| food.get("nutrition")).and_then(Nbt::as_i64).map(|nutrition| nutrition as i32)
}

pub fn count_food_points_in_inventory(mob: &Mob) -> i32 {
    let inventory = &mob.abstract_villager().expect("a villager").inventory;
    inventory.items.iter().filter_map(|item| villager_food_nutrition(item).map(|nutrition| item.count * nutrition)).sum()
}

pub fn has_farm_seeds(mob: &Mob) -> bool {
    mob.abstract_villager().expect("a villager").inventory.has_any_matching(|item| item.get_item().is(&item_tags::VILLAGER_PLANTABLE_SEEDS))
}

pub fn can_breed(mob: &Mob) -> bool {
    let food_level = villager(mob).food_level;
    let sleeping = matches!(mob.entity.data.get(crate::world::entity::data::ids::SLEEPING_POS), DataValue::OptionalBlockPos(Some(_)));
    food_level + count_food_points_in_inventory(mob) >= BREEDING_FOOD_THRESHOLD && !sleeping && mob.ageable().is_some_and(|ageable| ageable.age == 0)
}

impl Villager {
    pub fn create() -> Mob {
        let mut data = ageable_mob_defaults();
        data.extend([DataValue::Int(0), DataValue::VillagerData(create_default_villager_data()), DataValue::Boolean(false)]);
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:villager"), DIMENSIONS, data);
        let mut mob = Mob::new(entity, LivingEntity::new(&VILLAGER_ATTRIBUTES), MobKind::Villager(Villager::default()));
        AbstractVillager::init(&mut mob);
        mob.ai.navigation.set_can_open_doors(true);
        mob.ai.navigation.set_can_float(true);
        let follow_range = mob.living.attributes.value(crate::world::entity::attributes::Attribute::FollowRange);
        mob.ai.navigation.set_required_path_length_detached(follow_range, 48.0);
        mob.can_pick_up_loot = true;
        mob
    }

    pub fn make_brain(mob: &mut Mob, packed_brain: brain::Packed) -> Brain {
        let mut brain = BRAIN_PROVIDER.make_brain(mob, packed_brain);
        Villager::register_brain_goals(mob, &mut brain);
        brain
    }

    pub fn refresh_brain_detached(mob: &mut Mob) {
        let packed = mob.living.brain.pack();
        mob.living.brain = Villager::make_brain(mob, packed);
    }

    fn register_brain_goals(mob: &Mob, brain: &mut Brain) {
        if mob.is_baby() {
            brain.set_schedule(environment_attributes::BABY_VILLAGER_ACTIVITY);
        } else {
            brain.set_schedule(environment_attributes::VILLAGER_ACTIVITY);
        }
    }

    pub fn add_additional_save_data(mob: &Mob, out: &mut Vec<(String, Nbt)>) {
        AbstractVillager::add_additional_save_data(mob, out);
        let villager = villager(mob);
        out.push(("VillagerData".into(), get_villager_data(&mob.entity).to_nbt()));
        out.push(("VillagerDataFinalized".into(), Nbt::Byte(get_villager_data_finalized(&mob.entity) as i8)));
        out.push(("FoodLevel".into(), Nbt::Byte(villager.food_level as i8)));
        out.push(("Gossips".into(), villager.gossips.to_nbt()));
        out.push(("Xp".into(), Nbt::Int(villager.villager_xp)));
        out.push(("LastRestock".into(), Nbt::Long(villager.last_restock_game_time)));
        out.push(("LastGossipDecay".into(), Nbt::Long(villager.last_gossip_decay_time)));
        out.push(("RestocksToday".into(), Nbt::Int(villager.number_of_restocks_today)));
    }

    pub fn read_additional_save_data(mob: &mut Mob, nbt: &Nbt) {
        AbstractVillager::read_additional_save_data(mob, nbt);
        let villager_data_optional = nbt.get("VillagerData").and_then(VillagerData::from_nbt);
        let finalized = nbt.get("VillagerDataFinalized").and_then(Nbt::as_i64).is_some_and(|v| v != 0);
        if finalized || villager_data_optional.is_some() {
            set_villager_data_finalized(&mut mob.entity, true);
            let villager_data = villager_data_optional.unwrap_or_else(create_default_villager_data);
            mob.entity.data.set(DATA_VILLAGER_DATA, DataValue::VillagerData(villager_data));
        }
        let long = |key: &str, default: i64| nbt.get(key).and_then(Nbt::as_i64).unwrap_or(default);
        let villager = villager_mut(mob);
        villager.food_level = nbt.get("FoodLevel").and_then(Nbt::as_i64).map_or(DEFAULT_FOOD_LEVEL as i32, |v| v as i8 as i32);
        villager.gossips.clear();
        if let Some(gossips) = nbt.get("Gossips").and_then(GossipContainer::from_nbt) {
            villager.gossips.put_all(&gossips);
        }
        villager.villager_xp = long("Xp", DEFAULT_XP as i64) as i32;
        villager.last_restock_game_time = long("LastRestock", DEFAULT_LAST_RESTOCK);
        villager.last_gossip_decay_time = long("LastGossipDecay", DEFAULT_LAST_GOSSIP_DECAY);
        villager.number_of_restocks_today = long("RestocksToday", DEFAULT_RESTOCKS_TODAY as i64) as i32;
        Villager::refresh_brain_detached(mob);
    }

    pub fn get_default_dimensions(mob: &Mob) -> EntityDimensions {
        if mob.is_baby() { BABY_DIMENSIONS } else { DIMENSIONS }
    }
}

impl ServerLevel {
    pub fn villager(&self, id: EntityId) -> &Villager {
        villager(self.mob(id).expect("a villager"))
    }

    pub fn villager_mut(&mut self, id: EntityId) -> &mut Villager {
        villager_mut(self.mob_mut(id).expect("a villager"))
    }

    pub fn is_villager(&self, id: EntityId) -> bool {
        matches!(self.mob(id).map(|mob| &mob.kind), Some(MobKind::Villager(_)))
    }

    pub fn get_villager_data(&self, id: EntityId) -> VillagerData {
        get_villager_data(self.entity(id))
    }

    pub fn set_villager_data(&mut self, id: EntityId, data: VillagerData) {
        set_villager_data(self.mob_mut(id).expect("a villager"), data);
    }

    pub fn villager_register_brain_goals(&mut self, id: EntityId, pos: Vec3) {
        let game_time = self.game_time;
        Brain::update_activity_from_schedule(self, id, game_time, pos);
    }

    pub fn villager_register_brain_goals_detached(&self, mob: &mut Mob, pos: Vec3) {
        mob.living.brain.update_activity_from_schedule_detached(self, self.game_time, pos);
    }

    pub fn refresh_brain(&mut self, id: EntityId) {
        if self.get_brain(id).is_ticking() {
            self.get_brain_mut(id).request_refresh();
            return;
        }
        Brain::stop_all(self, id);
        self.villager_rebuild_brain(id);
    }

    pub fn villager_rebuild_brain(&mut self, id: EntityId) {
        let mob = self.mob_mut(id).expect("a villager");
        let packed = mob.living.brain.pack();
        let mut brain = BRAIN_PROVIDER.make_brain(mob, packed);
        Villager::register_brain_goals(mob, &mut brain);
        mob.living.brain = brain;
        let pos = self.entity(id).position();
        self.villager_register_brain_goals(id, pos);
    }

    pub fn villager_age_boundary_reached(&mut self, id: EntityId) {
        self.refresh_brain(id);
    }

    pub fn villager_custom_server_ai_step(&mut self, id: EntityId) {
        Brain::tick(self, id);
        if self.mob(id).is_none() {
            return;
        }
        if let Some(last_traded_player) = self.villager_mut(id).last_traded_player.take() {
            self.on_reputation_event(ReputationEventType::Trade, last_traded_player, id);
            self.broadcast_entity_event(id, crate::world::entity::entity_event::VILLAGER_HAPPY);
        }
        let no_ai = self.mob(id).expect("a villager").is_no_ai();
        if !no_ai && self.entity_mut(id).random.next_int_bounded(100) == 0 {
            let pos = self.entity(id).block_position();
            if self.get_raid_at(pos).is_some_and(|raid| raid.is_active() && !raid.is_over()) {
                self.broadcast_entity_event(id, crate::world::entity::entity_event::VILLAGER_SWEAT);
            }
        }
        if self.abstract_villager(id).offers.as_ref().is_none_or(|offers| offers.is_empty()) && self.is_trading(id) {
            self.stop_trading(id);
        }
    }

    pub fn villager_tick(&mut self, id: EntityId) {
        self.mob_tick(id);
        let Some(mob) = self.mob_mut(id) else { return };
        let unhappy_counter = abstract_villager::get_unhappy_counter(mob);
        if unhappy_counter > 0 {
            abstract_villager::set_unhappy_counter(mob, unhappy_counter - 1);
        }
        self.maybe_decay_gossip(id);
    }

    pub fn villager_mob_interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand) -> InteractionResult {
        let item_stack = self.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
        if item_stack.is(items::VILLAGER_SPAWN_EGG) || !self.is_alive(id) || self.is_trading(id) || self.is_sleeping(id) {
            return self.ageable_mob_mob_interact(id, player, hand);
        }
        if self.is_baby(id) {
            self.set_unhappy(id);
            return InteractionResult::SUCCESS;
        }
        let no_offers = self.get_offers(id).is_empty();
        if hand == InteractionHand::MainHand {
            if no_offers {
                self.set_unhappy(id);
            }
            self.award_stat(player, stats::TALKED_TO_VILLAGER, 1);
        }
        if no_offers {
            return InteractionResult::CONSUME;
        }
        self.start_trading(id, player);
        InteractionResult::SUCCESS
    }

    fn set_unhappy(&mut self, id: EntityId) {
        abstract_villager::set_unhappy_counter(self.mob_mut(id).expect("a villager"), 40);
        self.make_sound(id, sound_event("minecraft:entity.villager.no"));
    }

    fn start_trading(&mut self, id: EntityId, player: EntityId) {
        self.update_special_prices(id, player);
        self.set_trading_player(id, Some(player));
        let level = self.get_villager_data(id).level;
        self.open_trading_screen(id, player, level);
    }

    pub fn villager_set_trading_player(&mut self, id: EntityId, player: Option<EntityId>) {
        let should_stop = self.get_trading_player(id).is_some() && player.is_none();
        self.abstract_villager_set_trading_player(id, player);
        if should_stop {
            self.stop_trading(id);
        }
    }

    fn reset_special_prices(&mut self, id: EntityId) {
        self.get_offers(id);
        for offer in &mut self.abstract_villager_mut(id).offers.as_mut().expect("offers").0 {
            offer.reset_special_price_diff();
        }
    }

    pub fn restock(&mut self, id: EntityId) {
        self.update_demand(id);
        self.get_offers(id);
        for offer in &mut self.abstract_villager_mut(id).offers.as_mut().expect("offers").0 {
            offer.reset_uses();
        }
        if let Some(trading_player) = self.get_trading_player(id) {
            self.update_special_prices(id, trading_player);
        }
        let game_time = self.game_time;
        let villager = self.villager_mut(id);
        villager.last_restock_game_time = game_time;
        villager.number_of_restocks_today += 1;
    }

    fn needs_to_restock(&mut self, id: EntityId) -> bool {
        self.get_offers(id).0.iter().any(MerchantOffer::needs_restock)
    }

    fn allowed_to_restock(&self, id: EntityId) -> bool {
        let villager = self.villager(id);
        villager.number_of_restocks_today == 0 || villager.number_of_restocks_today < 2 && self.game_time > villager.last_restock_game_time + 2400
    }

    pub fn should_restock(&mut self, id: EntityId) -> bool {
        let half_day_passed_time = self.villager(id).last_restock_game_time + 12000;
        let game_time = self.game_time;
        let mut is_new_day = game_time > half_day_passed_time;
        let current_day = self.overworld_day_period_count() as i64;
        let villager = self.villager_mut(id);
        is_new_day |= villager.last_restock_check_day > 0 && current_day > villager.last_restock_check_day;
        villager.last_restock_check_day = current_day;
        if is_new_day {
            villager.last_restock_game_time = game_time;
            self.reset_number_of_restocks(id);
        }
        self.allowed_to_restock(id) && self.needs_to_restock(id)
    }

    fn overworld_day_period_count(&self) -> i32 {
        crate::world::clock::Timeline::get("minecraft:overworld_day")
            .and_then(|timeline| timeline.period_ticks.map(|period| (self.clock_ticks.get(timeline.clock).copied().unwrap_or(0) / period as i64) as i32))
            .unwrap_or(0)
    }

    fn catch_up_demand(&mut self, id: EntityId) {
        let missed_updates = 2 - self.villager(id).number_of_restocks_today;
        if missed_updates > 0 {
            self.get_offers(id);
            for offer in &mut self.abstract_villager_mut(id).offers.as_mut().expect("offers").0 {
                offer.reset_uses();
            }
        }
        for _ in 0..missed_updates {
            self.update_demand(id);
        }
        if let Some(trading_player) = self.get_trading_player(id) {
            self.update_special_prices(id, trading_player);
        }
    }

    fn update_demand(&mut self, id: EntityId) {
        self.get_offers(id);
        for offer in &mut self.abstract_villager_mut(id).offers.as_mut().expect("offers").0 {
            offer.update_demand();
        }
    }

    pub fn update_special_prices(&mut self, id: EntityId, player: EntityId) {
        self.reset_special_prices(id);
        let reputation = self.get_player_reputation(id, player);
        let hero_of_the_village = self.living(player).effect(mob_effects::HERO_OF_THE_VILLAGE).cloned();
        let hero_modifier = hero_of_the_village.map_or(0.0, |effect| (0.3f32 + 0.0625f32 * effect.amplifier as f32) as f64);
        for offer in &mut self.abstract_villager_mut(id).offers.as_mut().expect("offers").0 {
            if reputation != 0 {
                offer.add_to_special_price_diff(-mth::floor_f(reputation as f32 * offer.get_price_multiplier()));
            }
            if hero_modifier > 0.0 {
                let cost_reduction = (hero_modifier * offer.get_base_cost_a().count as f64).floor() as i32;
                offer.add_to_special_price_diff(-cost_reduction.max(1));
            }
        }
        if let Some(trading_player) = self.get_trading_player(id)
            && !self.abstract_villager(id).offers.as_ref().expect("offers").is_empty()
        {
            self.merchant_menu_update_sell_item(trading_player);
            let container_id = self.container_menu_id(trading_player);
            let level = self.get_villager_data(id).level;
            let xp = self.villager(id).villager_xp;
            self.send_merchant_offers(trading_player, id, container_id, level, xp, true, true);
        }
    }

    pub fn post_data_manipulated(&mut self, id: EntityId) {
        if self.is_villager(id)
            && let Some(trading_player) = self.get_trading_player(id)
        {
            self.update_special_prices(id, trading_player);
        }
    }

    pub fn play_work_sound(&mut self, id: EntityId) {
        if let Some(work_sound) = self.get_villager_data(id).profession.value().work_sound {
            self.make_sound(id, sound_event(work_sound));
        }
    }

    pub fn villager_reward_trade_xp(&mut self, id: EntityId, offer: &MerchantOffer) {
        let mut pop_xp = 3 + self.entity_mut(id).random.next_int_bounded(4);
        let trading_player = self.get_trading_player(id);
        let villager = self.villager_mut(id);
        villager.villager_xp += offer.get_xp();
        villager.last_traded_player = trading_player;
        if self.should_increase_level(id) {
            self.increase_merchant_career(id);
            self.add_effect(id, MobEffectInstance::new(mob_effects::REGENERATION, 200, 0), None);
            pop_xp += 5;
        }
        if offer.should_reward_exp() {
            let e = self.entity(id);
            let pos = Vec3::new(e.x(), e.y() + 0.5, e.z());
            let orb = self.new_experience_orb(pos, Vec3::ZERO, pop_xp);
            self.add_fresh_entity(LevelEntity::ExperienceOrb(orb));
        }
    }

    pub fn villager_set_last_hurt_by_mob(&mut self, id: EntityId, hurt_by: Option<EntityId>) {
        if let Some(hurt_by) = hurt_by {
            self.on_reputation_event(ReputationEventType::VillagerHurt, hurt_by, id);
            if self.is_alive(id) && self.is_player(hurt_by) {
                self.broadcast_entity_event(id, crate::world::entity::entity_event::VILLAGER_ANGRY);
            }
        }
        self.living_entity_set_last_hurt_by_mob(id, hurt_by);
    }

    pub fn villager_die(&mut self, id: EntityId, source: DamageSource) {
        let message = source.localized_death_message(self, id);
        info!("Villager {} died, message: '{}'", self.entity_to_string(id), message.get_string());
        if let Some(murderer) = source.causing_entity {
            self.tell_witnesses_that_i_was_murdered(id, murderer);
        }
        self.release_all_pois(id);
        self.abstract_villager_die(id, source);
    }

    fn release_all_pois(&mut self, id: EntityId) {
        self.release_poi(id, memory_module_type::HOME);
        self.release_poi(id, memory_module_type::JOB_SITE);
        self.release_poi(id, memory_module_type::POTENTIAL_JOB_SITE);
        self.release_poi(id, memory_module_type::MEETING_POINT);
    }

    fn tell_witnesses_that_i_was_murdered(&mut self, id: EntityId, murderer: EntityId) {
        let Some(witnesses) = self.get_brain(id).get_memory(memory_module_type::NEAREST_VISIBLE_LIVING_ENTITIES).cloned() else { return };
        for witness in witnesses.find_all(self, |level, entity| level.is_reputation_event_handler(entity)) {
            self.on_reputation_event(ReputationEventType::VillagerKilled, murderer, witness);
        }
    }

    pub fn release_poi(&mut self, id: EntityId, memory_type: MemoryModuleType<GlobalPos>) {
        let Some(memory) = self.get_brain(id).get_memory_internal(memory_type).flatten().cloned() else { return };
        if memory.dimension != self.dimension {
            return;
        }
        let ty = self.chunks.poi_manager.get_type(memory.pos);
        if let Some(ty) = ty
            && poi_memory_matches(memory_type, self.mob(id).expect("a villager"), ty)
        {
            self.chunks.poi_manager.release(memory.pos);
        }
    }

    fn hungry(&self, id: EntityId) -> bool {
        self.villager(id).food_level < BREEDING_FOOD_THRESHOLD
    }

    fn eat_until_full(&mut self, id: EntityId) {
        if !self.hungry(id) {
            return;
        }
        let size = self.abstract_villager(id).inventory.get_container_size();
        for slot in 0..size {
            let item_stack = self.abstract_villager(id).inventory.get_item(slot).clone();
            if let Some(nutrition) = villager_food_nutrition(&item_stack) {
                let item_count = item_stack.count;
                let mut to_remove = 0;
                let mut count = item_count;
                while count > 0 {
                    self.villager_mut(id).food_level += nutrition;
                    to_remove += 1;
                    if !self.hungry(id) {
                        self.abstract_villager_mut(id).inventory.remove_item(slot, to_remove);
                        return;
                    }
                    count -= 1;
                }
                self.abstract_villager_mut(id).inventory.remove_item(slot, to_remove);
            }
        }
    }

    pub fn get_player_reputation(&self, id: EntityId, player: EntityId) -> i32 {
        let uuid = self.entity(player).uuid;
        self.villager(id).gossips.get_reputation(uuid, |_| true)
    }

    pub fn eat_and_digest_food(&mut self, id: EntityId) {
        self.eat_until_full(id);
        self.villager_mut(id).food_level -= 12;
    }

    fn should_increase_level(&self, id: EntityId) -> bool {
        let current_level = self.get_villager_data(id).level;
        villager_data::can_level_up(current_level) && self.villager(id).villager_xp >= villager_data::get_max_xp_per_level(current_level)
    }

    fn increase_merchant_career(&mut self, id: EntityId) {
        let data = self.get_villager_data(id);
        self.set_villager_data(id, data.with_level(data.level + 1));
        self.villager_update_trades(id);
    }

    pub fn villager_get_breed_offspring(&mut self, id: EntityId, partner: EntityId) -> Option<Box<Mob>> {
        let biome_roll = self.entity_mut(id).random.next_double();
        let ty = if biome_roll < 0.5 {
            let pos = self.entity(id).block_position();
            villager_type::by_biome(ServerLevelAccessor::get_biome(self, pos).name())
        } else if biome_roll < 0.75 {
            self.get_villager_data(id).ty
        } else {
            self.get_villager_data(partner).ty
        };
        let mut baby = Box::new(Villager::create());
        let data = get_villager_data(&baby.entity).with_type(ty).with_profession(ProfessionId::NONE);
        set_villager_data(&mut baby, data);
        set_villager_data_finalized(&mut baby.entity, true);
        Some(baby)
    }

    pub fn villager_thunder_hit(&mut self, id: EntityId, lightning_bolt: EntityId) {
        if self.difficulty != Difficulty::Peaceful {
            info!("Villager {} was struck by lightning {}.", self.entity_to_string(id), self.entity_to_string(lightning_bolt));
            let witch = self.convert_to(id, "minecraft:witch", ConversionParams::single(self, id, false, false), |level, witch| {
                let pos = level.entity(witch).block_position();
                let difficulty = level.current_difficulty_at(pos);
                level.finalize_spawn(witch, &difficulty, EntitySpawnReason::Conversion, None);
                level.mob_mut(witch).expect("a witch").persistence_required = true;
                level.release_all_pois(id);
            });
            if witch.is_none() {
                self.entity_thunder_hit(id, lightning_bolt);
            }
        } else {
            self.entity_thunder_hit(id, lightning_bolt);
        }
    }

    pub fn villager_pick_up_item(&mut self, id: EntityId, entity: EntityId) {
        self.inventory_carrier_pick_up_item(id, entity, move |level| &mut level.abstract_villager_mut(id).inventory);
    }

    pub fn villager_wants_to_pick_up(&self, id: EntityId, item_stack: &ItemStack) -> bool {
        let item = item_stack.get_item();
        (item_stack.get_item().is(&item_tags::VILLAGER_PICKS_UP) || item_stack.has(dc::VILLAGER_FOOD) || self.get_villager_data(id).profession.value().requested_items.contains(&item))
            && self.abstract_villager(id).inventory.can_add_item(item_stack)
    }

    pub fn villager_update_trades(&mut self, id: EntityId) {
        let data = self.get_villager_data(id);
        if let Some(trades) = data.profession.value().get_trades(data.level) {
            self.add_offers_from_trade_set(id, trades);
            if let Some(trading_player) = self.get_trading_player(id) {
                self.update_special_prices(id, trading_player);
            }
        }
    }

    pub fn gossip(&mut self, id: EntityId, target: EntityId, timestamp: i64) {
        let last = self.villager(id).last_gossip_time;
        let target_last = self.villager(target).last_gossip_time;
        if (timestamp < last || timestamp >= last + GOSSIP_COOLDOWN) && (timestamp < target_last || timestamp >= target_last + GOSSIP_COOLDOWN) {
            let source_gossips = self.villager(target).gossips.clone();
            let mut random = std::mem::replace(&mut self.entity_mut(id).random, wafer_core::random::LegacyRandomSource::new(0));
            let new_gossip = self.villager_mut(id).gossips.transfer_from(&source_gossips, &mut random, MAX_GOSSIP_TOPICS);
            self.entity_mut(id).random = random;
            self.villager_mut(id).last_gossip_time = timestamp;
            self.villager_mut(target).last_gossip_time = timestamp;
            self.spawn_golem_if_needed(id, timestamp, HOW_MANY_VILLAGERS_NEED_TO_AGREE_TO_SPAWN_A_GOLEM);
            if let Some(trading_player) = self.get_trading_player(id)
                && new_gossip > 0
            {
                self.update_special_prices(id, trading_player);
            }
        }
    }

    fn maybe_decay_gossip(&mut self, id: EntityId) {
        let timestamp = self.game_time;
        let villager = self.villager_mut(id);
        if villager.last_gossip_decay_time == 0 {
            villager.last_gossip_decay_time = timestamp;
        } else if timestamp >= villager.last_gossip_decay_time + GOSSIP_DECAY_INTERVAL {
            villager.gossips.decay();
            villager.last_gossip_decay_time = timestamp;
        }
    }

    pub fn spawn_golem_if_needed(&mut self, id: EntityId, timestamp: i64, villagers_needed_to_agree: usize) {
        if !self.wants_to_spawn_golem(id, timestamp) {
            return;
        }
        let r = HOW_FAR_AWAY_TO_TALK_TO_OTHER_VILLAGERS_ABOUT_GOLEMS;
        let villager_search_box = self.entity(id).bounding_box().inflate(r, r, r);
        let nearby_villagers = self.get_living_entities(&villager_search_box, |level, entity| level.is_villager(entity));
        let nearby_villagers_that_want_a_golem = nearby_villagers.iter().filter(|&&villager| self.wants_to_spawn_golem(villager, timestamp)).take(5).count();
        if nearby_villagers_that_want_a_golem >= villagers_needed_to_agree {
            let pos = self.entity(id).block_position();
            if spawn_util::try_spawn_mob(self, &entity_types::IRON_GOLEM, EntitySpawnReason::MobSummoned, pos, 10, 8, 6, Strategy::LegacyIronGolem, false).is_some() {
                for villager in nearby_villagers {
                    golem_sensor::golem_detected(self, villager);
                }
            }
        }
    }

    pub fn wants_to_spawn_golem(&self, id: EntityId, _timestamp: i64) -> bool {
        self.golem_spawn_conditions_met(id, self.game_time) && !self.get_brain(id).has_memory_value(memory_module_type::GOLEM_DETECTED_RECENTLY)
    }

    pub fn is_reputation_event_handler(&self, id: EntityId) -> bool {
        self.is_villager(id)
    }

    pub fn on_reputation_event(&mut self, ty: ReputationEventType, source: EntityId, target: EntityId) {
        if self.is_villager(target) {
            self.villager_on_reputation_event_from(target, ty, source);
        }
    }

    pub fn villager_on_reputation_event_from(&mut self, id: EntityId, ty: ReputationEventType, source: EntityId) {
        let uuid = self.entity(source).uuid;
        let gossips = &mut self.villager_mut(id).gossips;
        match ty {
            ReputationEventType::ZombieVillagerCured => {
                gossips.add(uuid, GossipType::MajorPositive, 20);
                gossips.add(uuid, GossipType::MinorPositive, 25);
            }
            ReputationEventType::Trade => gossips.add(uuid, GossipType::Trading, 2),
            ReputationEventType::VillagerHurt => gossips.add(uuid, GossipType::MinorNegative, 25),
            ReputationEventType::VillagerKilled => gossips.add(uuid, GossipType::MajorNegative, 25),
            ReputationEventType::GolemKilled => {}
        }
        if let Some(trading_player) = self.get_trading_player(id)
            && self.entity(trading_player).uuid == uuid
        {
            self.update_special_prices(id, trading_player);
        }
    }

    pub fn get_villager_xp(&self, id: EntityId) -> i32 {
        self.mob(id).and_then(|mob| match &mob.kind {
            MobKind::Villager(villager) => Some(villager.villager_xp),
            _ => None,
        }).unwrap_or(0)
    }

    pub fn set_villager_xp(&mut self, id: EntityId, value: i32) {
        self.villager_mut(id).villager_xp = value;
    }

    fn reset_number_of_restocks(&mut self, id: EntityId) {
        self.catch_up_demand(id);
        self.villager_mut(id).number_of_restocks_today = 0;
    }

    pub fn villager_stop_sleeping(&mut self, id: EntityId) {
        self.living_entity_stop_sleeping(id);
        let game_time = self.game_time;
        self.get_brain_mut(id).set_memory(memory_module_type::LAST_WOKEN, game_time);
    }

    fn golem_spawn_conditions_met(&self, id: EntityId, game_time: i64) -> bool {
        self.get_brain(id).get_memory(memory_module_type::LAST_SLEPT).is_some_and(|&last_slept| game_time - last_slept < TIME_SINCE_SLEEPING_FOR_GOLEM_SPAWNING)
    }
}

pub fn get_villager_variant(mob: &Mob) -> Nbt {
    Nbt::String(get_villager_data(&mob.entity).ty.name().to_string())
}

pub fn apply_villager_variant(mob: &mut Mob, value: &Nbt) {
    if let Some(ty) = value.as_str().and_then(VillagerType::by_name) {
        let data = get_villager_data(&mob.entity).with_type(ty);
        set_villager_data(mob, data);
        set_villager_data_finalized(&mut mob.entity, true);
    }
}
