use wafer_core::random::{LegacyRandomSource, RandomSource};
use wafer_core::{BlockPos, SectionPos, Vec3, mth};
use wafer_data::packets::play::clientbound;
use wafer_data::{EquipmentSlot, blocks, mob_effects};
use wafer_protocol::nbt::Nbt;

use crate::network::chat::Component;
use crate::util::codec::{self, DecodeResult, Record};
use crate::util::extra_codecs;
use crate::server::level::server_boss_event::{BossBarColor, BossBarOverlay, ServerBossEvent};
use crate::server::level::{ServerLevel, SoundSource};
use crate::util::java_collections::JavaObjectHashSet;
use crate::world::difficulty::Difficulty;
use crate::world::effect::MobEffectInstance;
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::entity_types;
use crate::world::entity::mob::Mob;
use crate::world::entity::spawn_placements;
use crate::world::entity::{EntityId, create_insecure_uuid, sound_event};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::level::BlockGetter;
use crate::world::level::chunk::HeightmapType;
use crate::world::level::server_level_accessor::ServerLevelAccessor;
use crate::world::level::storage::uuid_ints;

const ALLOW_SPAWNING_WITHIN_VILLAGE_SECONDS_THRESHOLD: i32 = 7;
const SECTION_RADIUS_FOR_FINDING_NEW_VILLAGE_CENTER: i32 = 2;
const RAID_TIMEOUT_TICKS: i64 = 48000;
const NUM_SPAWN_ATTEMPTS: i32 = 5;
const POST_RAID_TICK_LIMIT: i32 = 40;
const DEFAULT_PRE_RAID_TICKS: i32 = 300;
pub const MAX_NO_ACTION_TIME: i32 = 2400;
pub const MAX_CELEBRATION_TICKS: i32 = 600;
const OUTSIDE_RAID_BOUNDS_TIMEOUT: i32 = 30;
pub const DEFAULT_MAX_RAID_OMEN_LEVEL: i32 = 5;
const LOW_MOB_THRESHOLD: i32 = 2;
const HERO_OF_THE_VILLAGE_DURATION: i32 = 48000;
pub const VALID_RAID_RADIUS_SQR: i32 = 9216;
pub const RAID_REMOVAL_THRESHOLD_SQR: f64 = 12544.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RaidStatus {
    Ongoing,
    Victory,
    Loss,
    Stopped,
}

impl RaidStatus {
    const NAMES: [(&'static str, RaidStatus); 4] = [("ongoing", RaidStatus::Ongoing), ("victory", RaidStatus::Victory), ("loss", RaidStatus::Loss), ("stopped", RaidStatus::Stopped)];

    fn get_serialized_name(self) -> &'static str {
        match self {
            RaidStatus::Ongoing => "ongoing",
            RaidStatus::Victory => "victory",
            RaidStatus::Loss => "loss",
            RaidStatus::Stopped => "stopped",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum RaiderType {
    Vindicator,
    Evoker,
    Pillager,
    Witch,
    Ravager,
}

impl RaiderType {
    const VALUES: [RaiderType; 5] = [RaiderType::Vindicator, RaiderType::Evoker, RaiderType::Pillager, RaiderType::Witch, RaiderType::Ravager];

    fn entity_type(self) -> &'static str {
        match self {
            RaiderType::Vindicator => "minecraft:vindicator",
            RaiderType::Evoker => "minecraft:evoker",
            RaiderType::Pillager => "minecraft:pillager",
            RaiderType::Witch => "minecraft:witch",
            RaiderType::Ravager => "minecraft:ravager",
        }
    }

    fn spawns_per_wave_before_bonus(self) -> [i32; 8] {
        match self {
            RaiderType::Vindicator => [0, 0, 2, 0, 1, 4, 2, 5],
            RaiderType::Evoker => [0, 0, 0, 0, 0, 1, 1, 2],
            RaiderType::Pillager => [0, 4, 3, 3, 4, 4, 4, 2],
            RaiderType::Witch => [0, 0, 0, 0, 3, 0, 0, 1],
            RaiderType::Ravager => [0, 0, 0, 1, 0, 1, 0, 2],
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
struct RaidMember {
    entity: EntityId,
    uuid: u128,
    health: f32,
}

pub struct Raid {
    group_to_leader_map: Vec<(i32, EntityId)>,
    group_raider_map: Vec<(i32, Vec<RaidMember>)>,
    heroes_of_the_village: JavaObjectHashSet<u128>,
    ticks_active: i64,
    center: BlockPos,
    started: bool,
    total_health: f32,
    raid_omen_level: i32,
    active: bool,
    groups_spawned: i32,
    post_raid_ticks: i32,
    raid_cooldown_ticks: i32,
    random: LegacyRandomSource,
    raid_event: ServerBossEvent,
    num_groups: i32,
    status: RaidStatus,
    celebration_ticks: i32,
    wave_spawn_pos: Option<BlockPos>,
}

fn raid_name_component() -> Component {
    Component::translatable("event.minecraft.raid")
}

pub fn get_num_groups(difficulty: Difficulty) -> i32 {
    match difficulty {
        Difficulty::Peaceful => 0,
        Difficulty::Easy => 3,
        Difficulty::Normal => 5,
        Difficulty::Hard => 7,
    }
}

impl Raid {
    fn with_fields(center: BlockPos, num_groups: i32, status: RaidStatus) -> Raid {
        let mut random = LegacyRandomSource::create();
        let raid_event = ServerBossEvent::new(create_insecure_uuid(&mut random), raid_name_component(), BossBarColor::Red, BossBarOverlay::Notched10);
        Raid {
            group_to_leader_map: Vec::new(),
            group_raider_map: Vec::new(),
            heroes_of_the_village: JavaObjectHashSet::default(),
            ticks_active: 0,
            center,
            started: false,
            total_health: 0.0,
            raid_omen_level: 0,
            active: false,
            groups_spawned: 0,
            post_raid_ticks: 0,
            raid_cooldown_ticks: 0,
            random,
            raid_event,
            num_groups,
            status,
            celebration_ticks: 0,
            wave_spawn_pos: None,
        }
    }

    pub fn new(center: BlockPos, difficulty: Difficulty) -> Raid {
        let mut raid = Raid::with_fields(center, get_num_groups(difficulty), RaidStatus::Ongoing);
        raid.active = true;
        raid.raid_cooldown_ticks = DEFAULT_PRE_RAID_TICKS;
        raid.raid_event.progress = 0.0;
        raid
    }

    pub fn decode(input: &Nbt) -> DecodeResult<Raid> {
        let mut record = Record::default();
        let started = record.required(input, "started", codec::bool_value);
        let active = record.required(input, "active", codec::bool_value);
        let ticks_active = record.required(input, "ticks_active", codec::long);
        let raid_omen_level = record.required(input, "raid_omen_level", codec::int);
        let groups_spawned = record.required(input, "groups_spawned", codec::int);
        let raid_cooldown_ticks = record.required(input, "cooldown_ticks", codec::int);
        let post_raid_ticks = record.required(input, "post_raid_ticks", codec::int);
        let total_health = record.required(input, "total_health", codec::float);
        let num_groups = record.required(input, "group_count", codec::int);
        let status = record.required(input, "status", |status| codec::string_enum(status, &RaidStatus::NAMES));
        let center = record.required(input, "center", extra_codecs::block_pos);
        let heroes = record.required(input, "heroes_of_the_village", |heroes| codec::list_partial(heroes, extra_codecs::uuid));
        record.build(|| {
            let mut raid = Raid::with_fields(center?, num_groups?, status?);
            raid.started = started?;
            raid.active = active?;
            raid.ticks_active = ticks_active?;
            raid.raid_omen_level = raid_omen_level?;
            raid.groups_spawned = groups_spawned?;
            raid.raid_cooldown_ticks = raid_cooldown_ticks?;
            raid.post_raid_ticks = post_raid_ticks?;
            raid.total_health = total_health?;
            for hero in heroes? {
                raid.heroes_of_the_village.insert(hero);
            }
            Some(raid)
        })
    }

    pub fn save(&self, output: &mut Vec<(String, Nbt)>) {
        output.push(("started".into(), Nbt::Byte(self.started as i8)));
        output.push(("active".into(), Nbt::Byte(self.active as i8)));
        output.push(("ticks_active".into(), Nbt::Long(self.ticks_active)));
        output.push(("raid_omen_level".into(), Nbt::Int(self.raid_omen_level)));
        output.push(("groups_spawned".into(), Nbt::Int(self.groups_spawned)));
        output.push(("cooldown_ticks".into(), Nbt::Int(self.raid_cooldown_ticks)));
        output.push(("post_raid_ticks".into(), Nbt::Int(self.post_raid_ticks)));
        output.push(("total_health".into(), Nbt::Float(self.total_health)));
        output.push(("group_count".into(), Nbt::Int(self.num_groups)));
        output.push(("status".into(), Nbt::String(self.status.get_serialized_name().into())));
        output.push(("center".into(), Nbt::IntArray(vec![self.center.x, self.center.y, self.center.z])));
        output.push(("heroes_of_the_village".into(), Nbt::List(self.heroes_of_the_village.iter().map(uuid_ints).collect())));
    }

    pub fn is_over(&self) -> bool {
        self.is_victory() || self.is_loss()
    }

    pub fn is_between_waves(&self) -> bool {
        self.has_first_wave_spawned() && self.get_total_raiders_alive() == 0 && self.raid_cooldown_ticks > 0
    }

    pub fn has_first_wave_spawned(&self) -> bool {
        self.groups_spawned > 0
    }

    pub fn is_stopped(&self) -> bool {
        self.status == RaidStatus::Stopped
    }

    pub fn is_victory(&self) -> bool {
        self.status == RaidStatus::Victory
    }

    pub fn is_loss(&self) -> bool {
        self.status == RaidStatus::Loss
    }

    pub fn get_total_health(&self) -> f32 {
        self.total_health
    }

    pub fn get_all_raiders(&self) -> Vec<EntityId> {
        self.group_raider_map.iter().flat_map(|(_, raiders)| raiders.iter().map(|member| member.entity)).collect()
    }

    pub fn is_started(&self) -> bool {
        self.started
    }

    pub fn get_groups_spawned(&self) -> i32 {
        self.groups_spawned
    }

    pub fn get_max_raid_omen_level(&self) -> i32 {
        DEFAULT_MAX_RAID_OMEN_LEVEL
    }

    pub fn get_raid_omen_level(&self) -> i32 {
        self.raid_omen_level
    }

    pub fn set_raid_omen_level(&mut self, raid_omen_level: i32) {
        self.raid_omen_level = raid_omen_level;
    }

    fn has_more_waves(&self) -> bool {
        if self.has_bonus_wave() { !self.has_spawned_bonus_wave() } else { !self.is_final_wave() }
    }

    fn is_final_wave(&self) -> bool {
        self.get_groups_spawned() == self.num_groups
    }

    fn has_bonus_wave(&self) -> bool {
        self.raid_omen_level > 1
    }

    fn has_spawned_bonus_wave(&self) -> bool {
        self.get_groups_spawned() > self.num_groups
    }

    fn should_spawn_bonus_group(&self) -> bool {
        self.is_final_wave() && self.get_total_raiders_alive() == 0 && self.has_bonus_wave()
    }

    fn should_spawn_group(&self) -> bool {
        self.raid_cooldown_ticks == 0 && (self.groups_spawned < self.num_groups || self.should_spawn_bonus_group()) && self.get_total_raiders_alive() == 0
    }

    pub fn get_total_raiders_alive(&self) -> i32 {
        self.group_raider_map.iter().map(|(_, raiders)| raiders.len() as i32).sum()
    }

    pub fn get_leader(&self, wave: i32) -> Option<EntityId> {
        self.group_to_leader_map.iter().find(|(group, _)| *group == wave).map(|&(_, leader)| leader)
    }

    pub fn remove_leader(&mut self, wave: i32) {
        self.group_to_leader_map.retain(|(group, _)| *group != wave);
    }

    pub fn get_center(&self) -> BlockPos {
        self.center
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn get_enchant_odds(&self) -> f32 {
        match self.get_raid_omen_level() {
            2 => 0.1,
            3 => 0.25,
            4 => 0.5,
            5 => 0.75,
            _ => 0.0,
        }
    }

    pub fn add_hero_of_the_village(&mut self, killer_uuid: u128) {
        self.heroes_of_the_village.insert(killer_uuid);
    }

    fn get_default_num_spawns(&self, raider_type: RaiderType, wave: i32, is_bonus_wave: bool) -> i32 {
        let spawns = raider_type.spawns_per_wave_before_bonus();
        if is_bonus_wave { spawns[self.num_groups as usize] } else { spawns[wave as usize] }
    }

    fn get_potential_bonus_spawns(&mut self, raider_type: RaiderType, wave: i32, difficulty: Difficulty, is_bonus_wave: bool) -> i32 {
        let is_easy = difficulty == Difficulty::Easy;
        let is_normal = difficulty == Difficulty::Normal;
        let bonus_spawns = match raider_type {
            RaiderType::Vindicator | RaiderType::Pillager => {
                if is_easy {
                    self.random.next_int_bounded(2)
                } else if is_normal {
                    1
                } else {
                    2
                }
            }
            RaiderType::Evoker => return 0,
            RaiderType::Witch => {
                if is_easy || wave <= 2 || wave == 4 {
                    return 0;
                }
                1
            }
            RaiderType::Ravager => (!is_easy && is_bonus_wave) as i32,
        };
        if bonus_spawns > 0 { self.random.next_int_bounded(bonus_spawns + 1) } else { 0 }
    }

    fn members_mut(&mut self, wave: i32) -> &mut Vec<RaidMember> {
        if let Some(index) = self.group_raider_map.iter().position(|(group, _)| *group == wave) {
            return &mut self.group_raider_map[index].1;
        }
        let index = self.group_raider_map.iter().position(|(group, _)| *group > wave).unwrap_or(self.group_raider_map.len());
        self.group_raider_map.insert(index, (wave, Vec::new()));
        &mut self.group_raider_map[index].1
    }
}

fn banner_pattern_layer(pattern: &str, color: &str) -> Nbt {
    Nbt::Compound(vec![("pattern".into(), Nbt::String(pattern.into())), ("color".into(), Nbt::String(color.into()))].into())
}

pub fn get_ominous_banner_instance() -> ItemStack {
    let patterns = Nbt::List(vec![
        banner_pattern_layer("minecraft:rhombus", "cyan"),
        banner_pattern_layer("minecraft:stripe_bottom", "light_gray"),
        banner_pattern_layer("minecraft:stripe_center", "gray"),
        banner_pattern_layer("minecraft:border", "light_gray"),
        banner_pattern_layer("minecraft:stripe_middle", "black"),
        banner_pattern_layer("minecraft:half_horizontal", "light_gray"),
        banner_pattern_layer("minecraft:circle", "light_gray"),
        banner_pattern_layer("minecraft:border", "black"),
    ]);
    let tooltip_display = Nbt::Compound(vec![("hidden_components".into(), Nbt::List(vec![Nbt::String(dc::BANNER_PATTERNS.name().into())]))].into());
    let item_name = Nbt::Compound(vec![("translate".into(), Nbt::String("block.minecraft.ominous_banner".into()))].into());
    let mut banner = ItemStack::new(wafer_data::items::WHITE_BANNER, 1);
    banner.set(dc::BANNER_PATTERNS, Some(dc::BANNER_PATTERNS.decode(&patterns).unwrap_or(patterns)));
    banner.set(dc::TOOLTIP_DISPLAY, Some(dc::TOOLTIP_DISPLAY.decode(&tooltip_display).unwrap_or(tooltip_display)));
    banner.set(dc::ITEM_NAME, Some(dc::ITEM_NAME.decode(&item_name).unwrap_or(item_name)));
    banner.set(dc::RARITY, Some(Nbt::String("uncommon".into())));
    banner
}

impl ServerLevel {
    pub fn raid(&self, raid_id: i32) -> Option<&Raid> {
        self.raids.get_any(raid_id)
    }

    fn raid_mut(&mut self, raid_id: i32) -> Option<&mut Raid> {
        self.raids.get_any_mut(raid_id)
    }

    fn with_raid_event(&mut self, raid_id: i32, f: impl FnOnce(&mut ServerBossEvent, &ServerLevel)) {
        let Some(raid) = self.raid_mut(raid_id) else { return };
        let placeholder = ServerBossEvent::new(0, Component::empty(), BossBarColor::Red, BossBarOverlay::Notched10);
        let mut raid_event = std::mem::replace(&mut raid.raid_event, placeholder);
        f(&mut raid_event, self);
        if let Some(raid) = self.raid_mut(raid_id) {
            raid.raid_event = raid_event;
        }
    }

    fn raid_valid_player(&self, raid_id: i32, player: EntityId) -> bool {
        let pos = self.entity(player).block_position();
        self.is_alive(player) && self.raids.get_id_at(pos) == Some(raid_id)
    }

    fn raid_update_players(&mut self, raid_id: i32) {
        let current_players_in_raid = self.raid(raid_id).map(|raid| raid.raid_event.players.clone()).unwrap_or_default();
        let new_players_in_raid: Vec<EntityId> = self.players.keys().copied().filter(|&player| self.raid_valid_player(raid_id, player)).collect();
        self.with_raid_event(raid_id, |raid_event, level| {
            for &player in &new_players_in_raid {
                if !current_players_in_raid.contains(&player) {
                    raid_event.add_player(std::slice::from_ref(level), player);
                }
            }
            for &player in &current_players_in_raid {
                if !new_players_in_raid.contains(&player) {
                    raid_event.remove_player(player);
                }
            }
        });
    }

    pub fn raid_absorb_raid_omen(&mut self, raid_id: i32, player: EntityId) -> bool {
        let Some(amplifier) = self.living(player).effect(mob_effects::RAID_OMEN).map(|effect| effect.amplifier) else { return false };
        let Some(raid) = self.raid_mut(raid_id) else { return false };
        raid.raid_omen_level += amplifier + 1;
        raid.raid_omen_level = mth::clamp_i(raid.raid_omen_level, 0, raid.get_max_raid_omen_level());
        if !raid.has_first_wave_spawned() {
            self.award_stat(player, crate::stats::stats::RAID_TRIGGER, 1);
            crate::advancements::triggers::criteria_triggers::RAID_OMEN.trigger(self, player);
        }
        true
    }

    pub fn raid_stop(&mut self, raid_id: i32) {
        let Some(raid) = self.raid_mut(raid_id) else { return };
        raid.active = false;
        raid.status = RaidStatus::Stopped;
        for player in raid.raid_event.players.clone() {
            raid.raid_event.remove_player(player);
        }
    }

    fn raid_set_name(&mut self, raid_id: i32, name: Component) {
        if let Some(raid) = self.raid_mut(raid_id) {
            raid.raid_event.set_name(name);
        }
    }

    fn raid_set_progress(&mut self, raid_id: i32, progress: f32) {
        if let Some(raid) = self.raid_mut(raid_id) {
            raid.raid_event.set_progress(progress);
        }
    }

    fn raid_set_visible(&mut self, raid_id: i32, visible: bool) {
        if let Some(raid) = self.raid_mut(raid_id) {
            raid.raid_event.set_visible(visible);
        }
    }

    pub fn raid_tick(&mut self, raid_id: i32) {
        let Some(raid) = self.raid(raid_id) else { return };
        if raid.is_stopped() {
            return;
        }
        if raid.status == RaidStatus::Ongoing {
            let old_active = raid.active;
            let center = raid.center;
            let active = self.has_chunk_at(center);
            self.raid_mut(raid_id).expect("a raid").active = active;
            if self.difficulty == Difficulty::Peaceful {
                self.raid_stop(raid_id);
                return;
            }
            if old_active != active {
                self.raid_set_visible(raid_id, active);
            }
            if !active {
                return;
            }
            if !self.is_village(center) {
                self.raid_move_raid_center_to_nearby_village_section(raid_id);
            }
            let center = self.raid(raid_id).expect("a raid").center;
            if !self.is_village(center) {
                if self.raid(raid_id).expect("a raid").groups_spawned > 0 {
                    self.raid_mut(raid_id).expect("a raid").status = RaidStatus::Loss;
                } else {
                    self.raid_stop(raid_id);
                }
            }
            let raid = self.raid_mut(raid_id).expect("a raid");
            raid.ticks_active += 1;
            if raid.ticks_active >= RAID_TIMEOUT_TICKS {
                self.raid_stop(raid_id);
                return;
            }
            let raiders_alive = raid.get_total_raiders_alive();
            if raiders_alive == 0 && raid.has_more_waves() {
                if raid.raid_cooldown_ticks <= 0 {
                    if raid.raid_cooldown_ticks == 0 && raid.groups_spawned > 0 {
                        raid.raid_cooldown_ticks = DEFAULT_PRE_RAID_TICKS;
                        self.raid_set_name(raid_id, raid_name_component());
                        return;
                    }
                } else {
                    let has_cached_wave_spawn_pos = raid.wave_spawn_pos.is_some();
                    let mut should_try_to_find_spawn_pos = !has_cached_wave_spawn_pos && raid.raid_cooldown_ticks % 5 == 0;
                    if let Some(wave_spawn_pos) = raid.wave_spawn_pos
                        && !self.is_position_entity_ticking(wave_spawn_pos)
                    {
                        should_try_to_find_spawn_pos = true;
                    }
                    if should_try_to_find_spawn_pos {
                        let wave_spawn_pos = self.raid_find_random_spawn_pos(raid_id, 8);
                        self.raid_mut(raid_id).expect("a raid").wave_spawn_pos = wave_spawn_pos;
                    }
                    let cooldown = self.raid(raid_id).expect("a raid").raid_cooldown_ticks;
                    if cooldown == DEFAULT_PRE_RAID_TICKS || cooldown % 20 == 0 {
                        self.raid_update_players(raid_id);
                    }
                    let raid = self.raid_mut(raid_id).expect("a raid");
                    raid.raid_cooldown_ticks -= 1;
                    let progress = mth::clamp_f((DEFAULT_PRE_RAID_TICKS - raid.raid_cooldown_ticks) as f32 / DEFAULT_PRE_RAID_TICKS as f32, 0.0, 1.0);
                    self.raid_set_progress(raid_id, progress);
                }
            }
            if self.raid(raid_id).expect("a raid").ticks_active % 20 == 0 {
                self.raid_update_players(raid_id);
                self.raid_update_raiders(raid_id);
                if raiders_alive > 0 && raiders_alive <= LOW_MOB_THRESHOLD {
                    let name = raid_name_component().append(Component::literal(" - ")).append(Component::translatable_with("event.minecraft.raid.raiders_remaining", vec![crate::network::chat::Arg::Int(raiders_alive)]));
                    self.raid_set_name(raid_id, name);
                } else {
                    self.raid_set_name(raid_id, raid_name_component());
                }
            }
            let mut sound_played = false;
            let mut attempt = 0;
            while self.raid(raid_id).is_some_and(Raid::should_spawn_group) {
                let spawn_pos = match self.raid(raid_id).expect("a raid").wave_spawn_pos {
                    Some(pos) => Some(pos),
                    None => self.raid_find_random_spawn_pos(raid_id, 20),
                };
                match spawn_pos {
                    Some(spawn_pos) => {
                        self.raid_mut(raid_id).expect("a raid").started = true;
                        self.raid_spawn_group(raid_id, spawn_pos);
                        if !sound_played {
                            self.raid_play_sound(raid_id, spawn_pos);
                            sound_played = true;
                        }
                    }
                    None => attempt += 1,
                }
                if attempt > NUM_SPAWN_ATTEMPTS {
                    self.raid_stop(raid_id);
                    break;
                }
            }
            let raid = self.raid_mut(raid_id).expect("a raid");
            if raid.is_started() && !raid.has_more_waves() && raiders_alive == 0 {
                if raid.post_raid_ticks < POST_RAID_TICK_LIMIT {
                    raid.post_raid_ticks += 1;
                } else {
                    raid.status = RaidStatus::Victory;
                    let raid_omen_level = raid.raid_omen_level;
                    let heroes: Vec<u128> = raid.heroes_of_the_village.iter().collect();
                    for hero_uuid in heroes {
                        let Some(hero) = self.entity_manager.visible_entity_storage.get(hero_uuid) else { continue };
                        if self.is_living(hero) && !self.is_spectator(hero) {
                            self.add_effect(hero, MobEffectInstance::with(mob_effects::HERO_OF_THE_VILLAGE, HERO_OF_THE_VILLAGE_DURATION, raid_omen_level - 1, false, false, true), None);
                            if self.is_player(hero) {
                                self.award_stat(hero, crate::stats::stats::RAID_WIN, 1);
                                crate::advancements::triggers::criteria_triggers::RAID_WIN.trigger(self, hero);
                            }
                        }
                    }
                }
            }
            self.raids.set_dirty();
        } else if raid.is_over() {
            let raid = self.raid_mut(raid_id).expect("a raid");
            raid.celebration_ticks += 1;
            if raid.celebration_ticks >= MAX_CELEBRATION_TICKS {
                self.raid_stop(raid_id);
                return;
            }
            if raid.celebration_ticks % 20 == 0 {
                let victory = raid.is_victory();
                self.raid_update_players(raid_id);
                self.raid_set_visible(raid_id, true);
                if victory {
                    self.raid_set_progress(raid_id, 0.0);
                    self.raid_set_name(raid_id, Component::translatable("event.minecraft.raid.victory.full"));
                } else {
                    self.raid_set_name(raid_id, Component::translatable("event.minecraft.raid.defeat.full"));
                }
            }
        }
    }

    fn raid_move_raid_center_to_nearby_village_section(&mut self, raid_id: i32) {
        let center = self.raid(raid_id).expect("a raid").center;
        let (origin_x, origin_y, origin_z) = (SectionPos::block_to_section_coord(center.x), SectionPos::block_to_section_coord(center.y), SectionPos::block_to_section_coord(center.z));
        let mut best: Option<(f64, BlockPos)> = None;
        let radius = SECTION_RADIUS_FOR_FINDING_NEW_VILLAGE_CENTER;
        for z in origin_z - radius..=origin_z + radius {
            for y in origin_y - radius..=origin_y + radius {
                for x in origin_x - radius..=origin_x + radius {
                    let section_center = BlockPos::new(SectionPos::section_to_block_coord(x) + 8, SectionPos::section_to_block_coord(y) + 8, SectionPos::section_to_block_coord(z) + 8);
                    if !self.is_village(section_center) {
                        continue;
                    }
                    let distance = section_center.dist_sqr(center);
                    if best.is_none_or(|(best_distance, _)| distance < best_distance) {
                        best = Some((distance, section_center));
                    }
                }
            }
        }
        if let Some((_, pos)) = best {
            self.raid_mut(raid_id).expect("a raid").center = pos;
        }
    }

    fn raid_update_raiders(&mut self, raid_id: i32) {
        let center = self.raid(raid_id).expect("a raid").center;
        let mut to_remove: Vec<EntityId> = Vec::new();
        for raider in self.raid(raid_id).expect("a raid").get_all_raiders() {
            let Some(e) = self.entity_ref(raider).filter(|e| !e.is_removed()) else {
                to_remove.push(raider);
                continue;
            };
            let raider_pos = e.block_position();
            if center.dist_sqr(raider_pos) >= RAID_REMOVAL_THRESHOLD_SQR {
                to_remove.push(raider);
            } else if e.tick_count > 600 {
                if self.entity_manager.visible_entity_storage.get(e.uuid).is_none() {
                    to_remove.push(raider);
                }
                if !self.is_village(raider_pos) && self.living(raider).no_action_time > MAX_NO_ACTION_TIME {
                    let ticks_outside_raid = self.raider(raider).get_ticks_outside_raid();
                    self.raider_mut(raider).set_ticks_outside_raid(ticks_outside_raid + 1);
                }
                if self.raider(raider).get_ticks_outside_raid() >= OUTSIDE_RAID_BOUNDS_TIMEOUT {
                    to_remove.push(raider);
                }
            }
        }
        to_remove.dedup();
        for raider in to_remove {
            let wave = self.raid_member_wave(raid_id, raider);
            self.raid_remove_from_raid(raid_id, raider, true);
            if self.entity_ref(raider).is_some()
                && self.raider(raider).patrolling_monster.is_patrol_leader()
                && let Some(wave) = wave
            {
                self.raid_mut(raid_id).expect("a raid").remove_leader(wave);
            }
        }
    }

    fn raid_member_wave(&self, raid_id: i32, raider: EntityId) -> Option<i32> {
        self.raid(raid_id)?.group_raider_map.iter().find(|(_, members)| members.iter().any(|member| member.entity == raider)).map(|&(wave, _)| wave)
    }

    fn raid_play_sound(&mut self, raid_id: i32, sound_origin: BlockPos) {
        let raid = self.raid_mut(raid_id).expect("a raid");
        let players_in_raid = raid.raid_event.players.clone();
        let seed = raid.random.next_long();
        let raid_loc = Vec3::at_center_of(sound_origin);
        let sound = sound_event("minecraft:event.raid.horn");
        for (&id, player) in &self.players {
            let player_loc = player.entity.position();
            let dist_btwn = ((raid_loc.x - player_loc.x) * (raid_loc.x - player_loc.x) + (raid_loc.z - player_loc.z) * (raid_loc.z - player_loc.z)).sqrt();
            let x3 = player_loc.x + 13.0 / dist_btwn * (raid_loc.x - player_loc.x);
            let z3 = player_loc.z + 13.0 / dist_btwn * (raid_loc.z - player_loc.z);
            if dist_btwn <= 64.0 || players_in_raid.contains(&id) {
                player.connection.send(clientbound::SOUND, |p| {
                    p.varint(sound + 1);
                    p.varint(SoundSource::Neutral as i32);
                    p.i32((x3 * 8.0) as i32);
                    p.i32((player_loc.y * 8.0) as i32);
                    p.i32((z3 * 8.0) as i32);
                    p.f32(64.0);
                    p.f32(1.0);
                    p.i64(seed);
                });
            }
        }
    }

    fn raid_spawn_group(&mut self, raid_id: i32, pos: BlockPos) {
        let mut leader_set = false;
        let raid = self.raid_mut(raid_id).expect("a raid");
        let group_number = raid.groups_spawned + 1;
        raid.total_health = 0.0;
        let difficulty = self.get_current_difficulty_at(pos);
        let is_bonus_group = self.raid(raid_id).expect("a raid").should_spawn_bonus_group();
        for raider_type in RaiderType::VALUES {
            let raid = self.raid_mut(raid_id).expect("a raid");
            let num_spawns = raid.get_default_num_spawns(raider_type, group_number, is_bonus_group) + raid.get_potential_bonus_spawns(raider_type, group_number, difficulty.get_difficulty(), is_bonus_group);
            let mut ravagers_spawned = 0;
            for _ in 0..num_spawns {
                let Some(raider) = ServerLevel::create_entity(raider_type.entity_type()) else { break };
                let raider_id = raider.entity().id;
                self.entities.insert(raider_id, raider);
                if !leader_set && self.mob(raider_id).is_some_and(Mob::can_be_leader) {
                    self.raider_mut(raider_id).patrolling_monster.set_patrol_leader(true);
                    self.raid_set_leader(raid_id, group_number, raider_id);
                    leader_set = true;
                }
                self.raid_join_raid_detached(raid_id, group_number, raider_id, pos);
                if raider_type == RaiderType::Ravager {
                    let riding_raider = if group_number == get_num_groups(Difficulty::Normal) {
                        ServerLevel::create_entity("minecraft:pillager")
                    } else if group_number >= get_num_groups(Difficulty::Hard) {
                        if ravagers_spawned == 0 { ServerLevel::create_entity("minecraft:evoker") } else { ServerLevel::create_entity("minecraft:vindicator") }
                    } else {
                        None
                    };
                    ravagers_spawned += 1;
                    if let Some(riding_raider) = riding_raider {
                        let riding_id = riding_raider.entity().id;
                        self.entities.insert(riding_id, riding_raider);
                        self.raid_join_raid_detached(raid_id, group_number, riding_id, pos);
                        if self.entity_ref(riding_id).is_some() {
                            self.entity_mut(riding_id).snap_to(pos.x as f64 + 0.5, pos.y as f64, pos.z as f64 + 0.5, 0.0, 0.0);
                            self.start_riding(riding_id, raider_id, false, false);
                        }
                    }
                }
            }
        }
        let raid = self.raid_mut(raid_id).expect("a raid");
        raid.wave_spawn_pos = None;
        raid.groups_spawned += 1;
        self.raid_update_bossbar(raid_id);
        self.raids.set_dirty();
    }

    fn raid_join_raid_detached(&mut self, raid_id: i32, group_number: i32, raider: EntityId, pos: BlockPos) {
        self.raid_add_wave_mob(raid_id, group_number, raider, true);
        self.raid_set_raider_fields(raid_id, group_number, raider);
        self.entity_mut(raider).set_pos(Vec3::new(pos.x as f64 + 0.5, pos.y as f64 + 1.0, pos.z as f64 + 0.5));
        let difficulty = self.get_current_difficulty_at(pos);
        self.finalize_spawn(raider, &difficulty, EntitySpawnReason::Event, None);
        self.raider_apply_raid_buffs(raider, group_number, false);
        self.set_on_ground(raider, true);
        let entity = self.entities.shift_remove(&raider).expect("the joining raider");
        self.add_fresh_entity_with_passengers(entity);
    }

    pub fn raid_join_raid_existing(&mut self, raid_id: i32, group_number: i32, raider: EntityId) {
        self.raid_add_wave_mob(raid_id, group_number, raider, true);
        self.raid_set_raider_fields(raid_id, group_number, raider);
    }

    fn raid_set_raider_fields(&mut self, raid_id: i32, group_number: i32, raider: EntityId) {
        let raider = self.raider_mut(raider);
        raider.set_current_raid(Some(raid_id));
        raider.set_wave(group_number);
        raider.set_can_join_raid(true);
        raider.set_ticks_outside_raid(0);
    }

    pub fn raid_update_bossbar(&mut self, raid_id: i32) {
        let health = self.raid_get_health_of_living_raiders(raid_id);
        let total_health = self.raid(raid_id).expect("a raid").total_health;
        self.raid_set_progress(raid_id, mth::clamp_f(health / total_health, 0.0, 1.0));
    }

    pub fn raid_get_health_of_living_raiders(&mut self, raid_id: i32) -> f32 {
        let healths: Vec<(EntityId, f32)> =
            self.raid(raid_id).expect("a raid").group_raider_map.iter().flat_map(|(_, members)| members.iter()).map(|member| (member.entity, member.health)).collect();
        let mut health = 0.0;
        let mut refreshed = Vec::with_capacity(healths.len());
        for (entity, cached) in healths {
            let current = if self.entity_ref(entity).is_some() { self.health(entity) } else { cached };
            refreshed.push((entity, current));
            health += current;
        }
        let raid = self.raid_mut(raid_id).expect("a raid");
        for (_, members) in &mut raid.group_raider_map {
            for member in members {
                if let Some(&(_, current)) = refreshed.iter().find(|(entity, _)| *entity == member.entity) {
                    member.health = current;
                }
            }
        }
        health
    }

    pub fn raid_remove_from_raid(&mut self, raid_id: i32, raider: EntityId, remove_from_total_health: bool) {
        let Some(wave) = self.raid_member_wave(raid_id, raider) else { return };
        let health = if self.entity_ref(raider).is_some() { self.health(raider) } else { self.raid(raid_id).and_then(|raid| raid.group_raider_map.iter().flat_map(|(_, members)| members.iter()).find(|member| member.entity == raider)).map_or(0.0, |member| member.health) };
        let raid = self.raid_mut(raid_id).expect("a raid");
        let members = raid.members_mut(wave);
        let Some(index) = members.iter().position(|member| member.entity == raider) else { return };
        members.remove(index);
        if remove_from_total_health {
            raid.total_health -= health;
        }
        if self.entity_ref(raider).is_some() && self.mob(raider).and_then(Mob::raider).is_some() {
            self.raider_mut(raider).set_current_raid(None);
        }
        self.raid_update_bossbar(raid_id);
        self.raids.set_dirty();
    }

    fn raid_find_random_spawn_pos(&mut self, raid_id: i32, max_tries: i32) -> Option<BlockPos> {
        let raid = self.raid_mut(raid_id).expect("a raid");
        let seconds_remaining = raid.raid_cooldown_ticks / 20;
        let how_far = 0.22 * seconds_remaining as f32 - 0.24;
        let start_angle = raid.random.next_float() * std::f32::consts::TAU;
        let center = raid.center;
        for i in 0..max_tries {
            let angle = start_angle + std::f32::consts::PI * i as f32 / 8.0;
            let raid = self.raid_mut(raid_id).expect("a raid");
            let spawn_x = center.x + mth::floor_f(mth::cos(angle as f64) * 32.0 * how_far) + raid.random.next_int_bounded(3) * mth::floor_f(how_far);
            let spawn_z = center.z + mth::floor_f(mth::sin(angle as f64) * 32.0 * how_far) + raid.random.next_int_bounded(3) * mth::floor_f(how_far);
            let spawn_y = self.get_height(HeightmapType::WorldSurface, spawn_x, spawn_z);
            if (spawn_y - center.y).abs() > 96 {
                continue;
            }
            let spawn_pos = BlockPos::new(spawn_x, spawn_y, spawn_z);
            if self.is_village(spawn_pos) && seconds_remaining > ALLOW_SPAWNING_WITHIN_VILLAGE_SECONDS_THRESHOLD {
                continue;
            }
            if self.has_chunks_at(spawn_pos.x - 10, spawn_pos.z - 10, spawn_pos.x + 10, spawn_pos.z + 10)
                && self.is_position_entity_ticking(spawn_pos)
                && (spawn_placements::get_placement_type(&entity_types::RAVAGER).is_spawn_position_ok(self, spawn_pos, Some(&entity_types::RAVAGER))
                    || self.block_state(spawn_pos.below()).is_block(blocks::SNOW) && self.block_state(spawn_pos).is_air())
            {
                return Some(spawn_pos);
            }
        }
        None
    }

    pub fn raid_add_wave_mob(&mut self, raid_id: i32, wave: i32, raider: EntityId, update_health: bool) -> bool {
        let uuid = self.entity(raider).uuid;
        let health = self.health(raider);
        let raid = self.raid_mut(raid_id).expect("a raid");
        let members = raid.members_mut(wave);
        if let Some(index) = members.iter().position(|member| member.uuid == uuid) {
            members.remove(index);
            members.push(RaidMember { entity: raider, uuid, health });
        }
        if !members.iter().any(|member| member.entity == raider) {
            members.push(RaidMember { entity: raider, uuid, health });
        }
        if update_health {
            raid.total_health += health;
        }
        self.raid_update_bossbar(raid_id);
        self.raids.set_dirty();
        true
    }

    pub fn raid_set_leader(&mut self, raid_id: i32, wave: i32, raider: EntityId) {
        let raid = self.raid_mut(raid_id).expect("a raid");
        raid.group_to_leader_map.retain(|(group, _)| *group != wave);
        raid.group_to_leader_map.push((wave, raider));
        self.set_item_slot(raider, EquipmentSlot::Head, get_ominous_banner_instance());
        self.mob_mut(raider).expect("a raider").set_drop_chance(EquipmentSlot::Head, 2.0);
    }

    pub fn is_position_entity_ticking(&self, pos: BlockPos) -> bool {
        self.entity_manager.is_ticking(pos.chunk()) && self.in_entity_ticking_range(pos.chunk())
    }
}
