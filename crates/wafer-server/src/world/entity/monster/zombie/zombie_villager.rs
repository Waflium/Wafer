use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, Vec3};
use wafer_data::{EquipmentSlot, block_tags, items, mob_effects, registry};
use wafer_protocol::nbt::Nbt;

use super::zombie::{Zombie, ZombieGroupData, zombie_defaults};
use crate::advancements::triggers::criteria_triggers;
use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::difficulty::Difficulty;
use crate::world::difficulty_instance::DifficultyInstance;
use crate::world::effect::MobEffectInstance;
use crate::world::entity::ai::gossip::gossip_container::GossipContainer;
use crate::world::entity::ai::village::reputation_event_type::ReputationEventType;
use crate::world::entity::attributes::ZOMBIE_ATTRIBUTES;
use crate::world::entity::conversion_params::ConversionParams;
use crate::world::entity::data::{DataValue, SynchedEntityData};
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::npc::villager::villager::{self, create_default_villager_data};
use crate::world::entity::npc::villager::villager_data::VillagerData;
use crate::world::entity::npc::villager::villager_profession::ProfessionId;
use crate::world::entity::npc::villager::villager_type::{self, VillagerType};
use crate::world::entity::spawn_group_data::SpawnGroupData;
use crate::world::entity::{EntityDimensions, EntityId, LevelEntity, entity_event};
use crate::world::item::ItemStack;
use crate::world::item::enchantment::enchantment_helper;
use crate::world::item::trading::merchant_offers::MerchantOffers;
use crate::world::level::BlockGetter;
use crate::world::level::server_level_accessor::ServerLevelAccessor;
use crate::world::{InteractionHand, InteractionResult};

pub const DATA_CONVERTING_ID: usize = 19;
pub const DATA_VILLAGER_DATA: usize = 20;
pub const DATA_VILLAGER_DATA_FINALIZED: usize = 21;
const VILLAGER_CONVERSION_WAIT_MIN: i32 = 3600;
const VILLAGER_CONVERSION_WAIT_MAX: i32 = 6000;
const MAX_SPECIAL_BLOCKS_COUNT: i32 = 14;
const SPECIAL_BLOCK_RADIUS: i32 = 4;
const NOT_CONVERTING: i32 = -1;
const DEFAULT_XP: i32 = 0;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.6, 1.95)
    .with_eye_height(1.74)
    .with_passenger_attachments(&[Vec3::new(0.0, 2.125f32 as f64, 0.0)])
    .with_vehicle_attachment(&[Vec3::new(0.0, 0.7f32 as f64, 0.0)]);
pub const BABY_DIMENSIONS: EntityDimensions =
    EntityDimensions::scalable(0.49, 0.98).with_eye_height(0.67).with_vehicle_attachment(&[Vec3::new(0.0, 0.125f32 as f64, 0.0)]);

#[derive(Default)]
pub struct ZombieVillager {
    pub zombie: Zombie,
    villager_conversion_time: i32,
    conversion_starter: Option<u128>,
    gossips: Option<GossipContainer>,
    trade_offers: Option<MerchantOffers>,
    villager_xp: i32,
}

fn zombie_villager(mob: &Mob) -> &ZombieVillager {
    match &mob.kind {
        MobKind::ZombieVillager(zombie_villager) => zombie_villager,
        _ => unreachable!("a zombie villager"),
    }
}

fn zombie_villager_mut(mob: &mut Mob) -> &mut ZombieVillager {
    match &mut mob.kind {
        MobKind::ZombieVillager(zombie_villager) => zombie_villager,
        _ => unreachable!("a zombie villager"),
    }
}

fn initialize_zombie_villager_data(random: &mut impl RandomSource) -> VillagerData {
    let villager_data = create_default_villager_data();
    let count = registry::VILLAGER_PROFESSION.len() as i32;
    if count == 0 {
        return villager_data;
    }
    villager_data.with_profession(ProfessionId(random.next_int_bounded(count)))
}

fn equipment_slot_index(slot: EquipmentSlot) -> usize {
    match slot {
        EquipmentSlot::Mainhand => 0,
        EquipmentSlot::Feet => 1,
        EquipmentSlot::Legs => 2,
        EquipmentSlot::Chest => 3,
        EquipmentSlot::Head => 4,
        EquipmentSlot::Offhand => 5,
        EquipmentSlot::Body => 6,
        EquipmentSlot::Saddle => 7,
    }
}

pub fn is_converting(mob: &Mob) -> bool {
    matches!(mob.entity.data.get(DATA_CONVERTING_ID), DataValue::Boolean(true))
}

pub fn get_villager_data(mob: &Mob) -> VillagerData {
    match mob.entity.data.get(DATA_VILLAGER_DATA) {
        DataValue::VillagerData(data) => *data,
        _ => create_default_villager_data(),
    }
}

pub fn set_villager_data(mob: &mut Mob, villager_data: VillagerData) {
    let current_data = get_villager_data(mob);
    if current_data.profession != villager_data.profession {
        zombie_villager_mut(mob).trade_offers = None;
    }
    mob.entity.data.set(DATA_VILLAGER_DATA, DataValue::VillagerData(villager_data));
}

pub fn get_villager_data_finalized(mob: &Mob) -> bool {
    matches!(mob.entity.data.get(DATA_VILLAGER_DATA_FINALIZED), DataValue::Boolean(true))
}

pub fn set_villager_data_finalized(mob: &mut Mob, villager_data_finalized: bool) {
    mob.entity.data.set(DATA_VILLAGER_DATA_FINALIZED, DataValue::Boolean(villager_data_finalized));
}

pub fn finalize_villager_type(level: &mut (impl ServerLevelAccessor + ?Sized), mob: &mut Mob, pos: BlockPos) {
    if !get_villager_data_finalized(mob) {
        let ty = villager_type::by_biome(level.get_biome(pos).name());
        let data = get_villager_data(mob).with_type(ty);
        set_villager_data(mob, data);
        set_villager_data_finalized(mob, true);
    }
}

pub fn get_villager_variant(mob: &Mob) -> Nbt {
    Nbt::String(get_villager_data(mob).ty.name().to_string())
}

pub fn apply_villager_variant(mob: &mut Mob, value: &Nbt) {
    if let Some(ty) = value.as_str().and_then(VillagerType::by_name) {
        let data = get_villager_data(mob).with_type(ty);
        set_villager_data(mob, data);
        set_villager_data_finalized(mob, true);
    }
}

pub fn remove_when_far_away(mob: &Mob) -> bool {
    !is_converting(mob) && zombie_villager(mob).villager_xp == 0
}

pub fn get_default_dimensions(mob: &Mob) -> EntityDimensions {
    if mob.is_baby() { BABY_DIMENSIONS } else { DIMENSIONS }
}

impl ZombieVillager {
    pub fn create() -> Mob {
        let mut mob = Zombie::create_with("minecraft:zombie_villager", DIMENSIONS, &ZOMBIE_ATTRIBUTES, MobKind::ZombieVillager(ZombieVillager::default()));
        let villager_data = initialize_zombie_villager_data(&mut mob.entity.random);
        let mut defaults = zombie_defaults();
        defaults.extend([DataValue::Boolean(false), DataValue::VillagerData(villager_data), DataValue::Boolean(false)]);
        mob.entity.data = SynchedEntityData::new(defaults);
        mob
    }

    pub fn add_additional_save_data(mob: &Mob, out: &mut Vec<(String, Nbt)>) {
        let is_afflicted = mob.entity.is_eye_in_fluid(&wafer_data::fluid_tags::WATER);
        Zombie::add_additional_save_data(mob, is_afflicted, out);
        let zombie_villager = zombie_villager(mob);
        out.push(("VillagerData".into(), get_villager_data(mob).to_nbt()));
        out.push(("VillagerDataFinalized".into(), Nbt::Byte(get_villager_data_finalized(mob) as i8)));
        if let Some(trade_offers) = &zombie_villager.trade_offers {
            out.push(("Offers".into(), trade_offers.to_nbt()));
        }
        if let Some(gossips) = &zombie_villager.gossips {
            out.push(("Gossips".into(), gossips.to_nbt()));
        }
        let conversion_time = if is_converting(mob) { zombie_villager.villager_conversion_time } else { NOT_CONVERTING };
        out.push(("ConversionTime".into(), Nbt::Int(conversion_time)));
        if let Some(conversion_starter) = zombie_villager.conversion_starter {
            out.push(("ConversionPlayer".into(), crate::world::level::storage::uuid_ints(conversion_starter)));
        }
        out.push(("Xp".into(), Nbt::Int(zombie_villager.villager_xp)));
    }

    pub fn read_additional_save_data(mob: &mut Mob, input: &Nbt) {
        Zombie::read_additional_save_data(mob, input);
        let villager_data_optional = input.get("VillagerData").and_then(VillagerData::from_nbt);
        let finalized = input.get("VillagerDataFinalized").and_then(Nbt::as_i64).is_some_and(|v| v != 0);
        if finalized || villager_data_optional.is_some() {
            set_villager_data_finalized(mob, true);
            let villager_data = match villager_data_optional {
                Some(villager_data) => villager_data,
                None => initialize_zombie_villager_data(&mut mob.entity.random),
            };
            mob.entity.data.set(DATA_VILLAGER_DATA, DataValue::VillagerData(villager_data));
        }
        let trade_offers = input.get("Offers").and_then(|offers| MerchantOffers::decode(offers).ok());
        let gossips = input.get("Gossips").and_then(GossipContainer::from_nbt);
        let zombie_villager = zombie_villager_mut(mob);
        zombie_villager.trade_offers = trade_offers;
        zombie_villager.gossips = gossips;
        let conversion_time = input.get("ConversionTime").and_then(Nbt::as_i64).map_or(NOT_CONVERTING, |v| v as i32);
        if conversion_time != NOT_CONVERTING {
            let conversion_starter = input.get("ConversionPlayer").and_then(crate::world::entity::entity_save::uuid_from_nbt);
            Self::start_converting_detached(mob, conversion_starter, conversion_time);
        } else {
            mob.entity.data.set(DATA_CONVERTING_ID, DataValue::Boolean(false));
            zombie_villager_mut(mob).villager_conversion_time = NOT_CONVERTING;
        }
        zombie_villager_mut(mob).villager_xp = input.get("Xp").and_then(Nbt::as_i64).map_or(DEFAULT_XP, |v| v as i32);
    }

    fn start_converting_detached(mob: &mut Mob, player: Option<u128>, time: i32) {
        let zombie_villager = zombie_villager_mut(mob);
        zombie_villager.conversion_starter = player;
        zombie_villager.villager_conversion_time = time;
        mob.entity.data.set(DATA_CONVERTING_ID, DataValue::Boolean(true));
    }
}

impl ServerLevel {
    pub fn zombie_villager_finalize_spawn(&mut self, id: EntityId, difficulty: &DifficultyInstance, spawn_reason: EntitySpawnReason, group_data: Option<SpawnGroupData>) -> Option<SpawnGroupData> {
        let pos = self.entity(id).block_position();
        let (index, _, entity) = self.entities.shift_remove_full(&id).expect("the zombie villager");
        let LevelEntity::Mob(mut mob) = entity else { unreachable!("a mob") };
        finalize_villager_type(self, &mut mob, pos);
        self.entities.shift_insert(index, id, LevelEntity::Mob(mob));
        self.zombie_finalize_spawn(id, difficulty, spawn_reason, group_data)
    }

    pub fn zombie_villager_tick(&mut self, id: EntityId) {
        if self.is_alive(id) && is_converting(self.mob(id).expect("a zombie villager")) {
            let amount = self.get_conversion_progress(id);
            let zombie_villager = zombie_villager_mut(self.mob_mut(id).expect("a zombie villager"));
            zombie_villager.villager_conversion_time -= amount;
            if zombie_villager.villager_conversion_time <= 0 {
                self.finish_conversion(id);
            }
        }
        if self.entity_ref(id).is_some() {
            self.zombie_tick(id);
        }
    }

    pub fn zombie_villager_mob_interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand) -> InteractionResult {
        let mut item_stack = self.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
        if item_stack.is(items::GOLDEN_APPLE) {
            if self.has_effect(id, mob_effects::WEAKNESS) {
                item_stack.consume(1, self, Some(player));
                self.player_mut(player).set_item_in_hand(hand, Some(item_stack));
                let uuid = self.entity(player).uuid;
                let time = self.entity_mut(id).random.next_int_bounded(VILLAGER_CONVERSION_WAIT_MAX - VILLAGER_CONVERSION_WAIT_MIN + 1) + VILLAGER_CONVERSION_WAIT_MIN;
                self.start_converting(id, Some(uuid), time);
                return InteractionResult::SUCCESS_SERVER;
            }
            return InteractionResult::CONSUME;
        }
        self.mob_mob_interact(id, player, hand)
    }

    pub fn zombie_villager_read_level_dependent_save_data(&mut self, id: EntityId, input: &Nbt) {
        let conversion_time = input.get("ConversionTime").and_then(Nbt::as_i64).map_or(NOT_CONVERTING, |v| v as i32);
        if conversion_time != NOT_CONVERTING {
            let conversion_starter = input.get("ConversionPlayer").and_then(crate::world::entity::entity_save::uuid_from_nbt);
            self.start_converting(id, conversion_starter, conversion_time);
        }
    }

    fn start_converting(&mut self, id: EntityId, player: Option<u128>, time: i32) {
        ZombieVillager::start_converting_detached(self.mob_mut(id).expect("a zombie villager"), player, time);
        self.remove_effect(id, mob_effects::WEAKNESS);
        let amplifier = (self.difficulty.id() - 1).min(0);
        self.add_effect(id, MobEffectInstance::new(mob_effects::STRENGTH, time, amplifier), None);
        self.broadcast_entity_event(id, entity_event::ZOMBIE_CONVERTING);
    }

    fn finish_conversion(&mut self, id: EntityId) {
        let params = ConversionParams::single(self, id, false, false);
        self.convert_to(id, "minecraft:villager", params, |level, villager| {
            let mut undropped_slots = Vec::new();
            for slot in EquipmentSlot::VALUES {
                let Some(item_stack) = level.item_by_slot(id, slot).filter(|stack| !stack.is_empty()) else { continue };
                if enchantment_helper::has_prevent_armor_change(&item_stack) {
                    undropped_slots.push(slot);
                } else if level.mob(id).expect("a zombie villager").drop_chances[slot as usize] > 1.0 {
                    level.set_item_slot(id, slot, ItemStack::empty());
                    level.spawn_at_location(id, item_stack, 0.0);
                }
            }
            for slot in undropped_slots {
                let item_stack = level.item_by_slot(id, slot).unwrap_or_else(ItemStack::empty);
                let inventory = &mut level.abstract_villager_mut(villager).inventory;
                let index = equipment_slot_index(slot);
                if index < inventory.get_container_size() {
                    inventory.set_item(index, item_stack);
                }
            }
            let mob = level.mob(id).expect("a zombie villager");
            let (finalized, villager_data) = (get_villager_data_finalized(mob), get_villager_data(mob));
            let zombie_villager = zombie_villager(mob);
            let (gossips, trade_offers, villager_xp, conversion_starter) =
                (zombie_villager.gossips.clone(), zombie_villager.trade_offers.clone(), zombie_villager.villager_xp, zombie_villager.conversion_starter);
            let is_silent = level.entity(id).is_silent();
            let target = level.mob_mut(villager).expect("a villager");
            villager::set_villager_data_finalized(&mut target.entity, finalized);
            villager::set_villager_data(target, villager_data);
            if let Some(gossips) = gossips {
                let target_gossips = &mut villager::villager_mut(target).gossips;
                target_gossips.clear();
                target_gossips.put_all(&gossips);
            }
            if let Some(trade_offers) = trade_offers {
                target.abstract_villager_mut().expect("a villager").offers = Some(trade_offers);
            }
            level.set_villager_xp(villager, villager_xp);
            let pos = level.entity(villager).block_position();
            let difficulty = level.current_difficulty_at(pos);
            level.finalize_spawn(villager, &difficulty, EntitySpawnReason::Conversion, None);
            level.refresh_brain(villager);
            if let Some(conversion_starter) = conversion_starter
                && let Some(player) = level.players.values().find(|player| player.entity.uuid == conversion_starter).map(|player| player.entity.id)
            {
                criteria_triggers::CURED_ZOMBIE_VILLAGER.trigger(level, player, id, villager);
                level.on_reputation_event(ReputationEventType::ZombieVillagerCured, player, villager);
            }
            level.add_effect(villager, MobEffectInstance::new(mob_effects::NAUSEA, 200, 0), None);
            if !is_silent {
                let pos = level.entity(id).block_position();
                level.level_event(None, crate::server::level::events::SOUND_ZOMBIE_CONVERTED, pos, 0);
            }
        });
    }

    fn get_conversion_progress(&mut self, id: EntityId) -> i32 {
        let mut amount = 1;
        if self.entity_mut(id).random.next_float() < 0.01 {
            let mut special_blocks_count = 0;
            let e = self.entity(id);
            let (x, y, z) = (e.x() as i32, e.y() as i32, e.z() as i32);
            let mut xx = x - SPECIAL_BLOCK_RADIUS;
            while xx < x + SPECIAL_BLOCK_RADIUS && special_blocks_count < MAX_SPECIAL_BLOCKS_COUNT {
                let mut yy = y - SPECIAL_BLOCK_RADIUS;
                while yy < y + SPECIAL_BLOCK_RADIUS && special_blocks_count < MAX_SPECIAL_BLOCKS_COUNT {
                    let mut zz = z - SPECIAL_BLOCK_RADIUS;
                    while zz < z + SPECIAL_BLOCK_RADIUS && special_blocks_count < MAX_SPECIAL_BLOCKS_COUNT {
                        if self.block_state(BlockPos::new(xx, yy, zz)).is(&block_tags::SPEEDS_UP_ZOMBIE_VILLAGER_CURING) {
                            if self.entity_mut(id).random.next_float() < 0.3 {
                                amount += 1;
                            }
                            special_blocks_count += 1;
                        }
                        zz += 1;
                    }
                    yy += 1;
                }
                xx += 1;
            }
        }
        amount
    }

    pub fn zombie_villager_voice_pitch(&mut self, id: EntityId) -> f32 {
        let baby = self.is_baby(id);
        let random = &mut self.entity_mut(id).random;
        let spread = (random.next_float() - random.next_float()) * 0.2;
        if baby { spread + 2.0 } else { spread + 1.0 }
    }

    pub fn zombie_killed_entity(&mut self, id: EntityId, entity: EntityId, source: &DamageSource) -> bool {
        let mut perished = self.entity_killed_entity(id, entity, source);
        if matches!(self.difficulty, Difficulty::Normal | Difficulty::Hard) && self.is_villager(entity) {
            if self.difficulty != Difficulty::Hard && self.entity_mut(id).random.next_boolean() {
                return perished;
            }
            if self.convert_villager_to_zombie_villager(id, entity) {
                perished = false;
            }
        }
        perished
    }

    pub fn convert_villager_to_zombie_villager(&mut self, id: EntityId, villager: EntityId) -> bool {
        let params = ConversionParams::single(self, villager, true, true);
        let is_silent = self.entity(id).is_silent();
        let zombie_pos = self.entity(id).block_position();
        let finalized = villager::get_villager_data_finalized(self.entity(villager));
        let villager_data = self.get_villager_data(villager);
        let gossips = self.villager(villager).gossips.copy();
        let trade_offers = self.get_offers(villager).clone();
        let villager_xp = self.get_villager_xp(villager);
        let zombie_villager = self.convert_to(villager, "minecraft:zombie_villager", params, |level, zombie| {
            set_villager_data_finalized(level.mob_mut(zombie).expect("a zombie villager"), finalized);
            let pos = level.entity(zombie).block_position();
            let difficulty = level.current_difficulty_at(pos);
            level.finalize_spawn(zombie, &difficulty, EntitySpawnReason::Conversion, Some(SpawnGroupData::Zombie(ZombieGroupData { is_baby: false, can_spawn_jockey: true })));
            let mob = level.mob_mut(zombie).expect("a zombie villager");
            set_villager_data(mob, villager_data);
            let zombie_villager = zombie_villager_mut(mob);
            zombie_villager.gossips = Some(gossips);
            zombie_villager.trade_offers = Some(trade_offers);
            zombie_villager.villager_xp = villager_xp;
            if !is_silent {
                level.level_event(None, crate::server::level::events::SOUND_ZOMBIE_INFECTED, zombie_pos, 0);
            }
        });
        zombie_villager.is_some()
    }
}
