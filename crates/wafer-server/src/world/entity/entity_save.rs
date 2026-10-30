use std::cell::RefCell;

use wafer_core::hash_map::java_hash_map_order;
use wafer_core::random::{RandomSource, java_string_hash};
use wafer_core::{AABB, Axis, BlockPos, Vec3, mth};
use wafer_protocol::nbt::Nbt;

use super::data::{DataValue, ids};
use super::entity_spawn_reason::EntitySpawnReason;
use crate::world::level::server_level_accessor::ServerLevelAccessor;
use super::item::ItemEntity;
use super::primed_tnt::PrimedTnt;
use super::projectile::throwableitemprojectile;
use super::{Entity, EntityId, LevelEntity, MAX_ENTITY_TAG_COUNT, RemovalReason};
use crate::network::chat::component_serialization;
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DecodeResult};
use crate::util::extra_codecs;
use crate::util::java_throwable::JavaThrowable;
use crate::util::problem_reporter::PathElement;
use crate::world::item::ItemStack;
use crate::world::level::block::entity::int_value;
use crate::world::level::chunk::serialization::state_to_nbt;
use crate::world::level::collision::{Collider, shapes_collide};
use crate::world::level::storage::tag_value_input;
use crate::world::level::storage::{format_uuid, uuid_ints};
use crate::world::scores::scoreboard::Scoreboard;

const ENTITY_TYPE_LOGGER: &str = "net.minecraft.world.entity.EntityType";
const TYPED_ENTITY_DATA_LOGGER: &str = "net.minecraft.world.item.component.TypedEntityData";
pub const NBT_PREDICATE_LOGGER: &str = "net.minecraft.advancements.predicates.NbtPredicate";
pub const ENTITY_SELECTOR_OPTIONS_LOGGER: &str = "net.minecraft.commands.arguments.selector.options.EntitySelectorOptions";

pub fn uuid_from_nbt(nbt: &Nbt) -> Option<u128> {
    let ints: [i32; 4] = match nbt {
        Nbt::IntArray(v) => v.as_slice().try_into().ok()?,
        Nbt::ByteArray(v) => <[i8; 4]>::try_from(v.as_slice()).ok()?.map(i32::from),
        Nbt::LongArray(v) => <[i64; 4]>::try_from(v.as_slice()).ok()?.map(|l| l as i32),
        Nbt::List(v) => {
            let numbers: [&Nbt; 4] = v.iter().collect::<Vec<_>>().try_into().ok()?;
            let mut ints = [0; 4];
            for (int, number) in ints.iter_mut().zip(numbers) {
                *int = match number {
                    Nbt::Byte(n) => *n as i32,
                    Nbt::Short(n) => *n as i32,
                    Nbt::Int(n) => *n,
                    Nbt::Long(n) => *n as i32,
                    Nbt::Float(n) => *n as i32,
                    Nbt::Double(n) => *n as i32,
                    _ => return None,
                };
            }
            ints
        }
        _ => return None,
    };
    Some(ints.iter().fold(0u128, |uuid, &i| uuid << 32 | i as u32 as u128))
}

pub fn decode_uuid(tag: &Nbt) -> DecodeResult<u128> {
    if let Nbt::IntArray(ints) = tag
        && let [a, b, c, d] = ints.as_slice()
    {
        return DecodeResult::success([a, b, c, d].iter().fold(0u128, |uuid, &&i| uuid << 32 | i as u32 as u128));
    }
    extra_codecs::fixed_size_ints(tag, 4).map(|ints| ints.iter().fold(0u128, |uuid, &i| uuid << 32 | i as u32 as u128))
}

pub fn decode_custom_data(tag: &Nbt) -> Result<Nbt, String> {
    let flattened_error = match tag {
        Nbt::Compound(_) => return Ok(tag.clone()),
        Nbt::String(snbt) => match crate::nbt::parse_fully(snbt) {
            Ok(parsed @ Nbt::Compound(_)) => return Ok(parsed),
            Ok(other) => format!("Expected compound tag, got {}", crate::nbt::to_string(&other)),
            Err(error) => error.get_message(),
        },
        _ => "Not a string".to_string(),
    };
    Err(format!("Failed to parse either. First: Not a compound tag: {}; Second: {flattened_error}", crate::nbt::to_string(tag)))
}

fn number(nbt: &Nbt, key: &str) -> Option<f64> {
    tag_value_input::get_f64(nbt, key)
}

fn flag(nbt: &Nbt, key: &str) -> bool {
    tag_value_input::get_boolean(nbt, key).unwrap_or(false)
}

impl Entity {
    pub fn save_without_id(&self, out: &mut Vec<(String, Nbt)>) {
        let pos = self.position();
        let d = self.delta_movement;
        out.push(("Pos".into(), Nbt::List(vec![Nbt::Double(pos.x), Nbt::Double(pos.y), Nbt::Double(pos.z)])));
        out.push(("Motion".into(), Nbt::List(vec![Nbt::Double(d.x), Nbt::Double(d.y), Nbt::Double(d.z)])));
        out.push(("Rotation".into(), Nbt::List(vec![Nbt::Float(self.y_rot), Nbt::Float(self.x_rot)])));
        out.push(("fall_distance".into(), Nbt::Double(self.fall_distance)));
        out.push(("Fire".into(), Nbt::Short(self.remaining_fire_ticks as i16)));
        out.push(("Air".into(), Nbt::Short(self.air_supply() as i16)));
        out.push(("OnGround".into(), Nbt::Byte(self.on_ground as i8)));
        out.push(("Invulnerable".into(), Nbt::Byte(self.invulnerable as i8)));
        out.push(("PortalCooldown".into(), Nbt::Int(self.portal_cooldown)));
        if self.invulnerable_time > 0 {
            out.push(("invulnerable_time".into(), Nbt::Int(self.invulnerable_time)));
        }
        out.push(("UUID".into(), uuid_ints(self.uuid)));
        if let DataValue::OptionalComponent(Some(name)) = self.data.get(ids::CUSTOM_NAME) {
            out.push(("CustomName".into(), name.clone()));
        }
        if matches!(self.data.get(ids::CUSTOM_NAME_VISIBLE), DataValue::Boolean(true)) {
            out.push(("CustomNameVisible".into(), Nbt::Byte(1)));
        }
        if self.is_silent() {
            out.push(("Silent".into(), Nbt::Byte(1)));
        }
        if self.is_no_gravity() {
            out.push(("NoGravity".into(), Nbt::Byte(1)));
        }
        if self.has_glowing_tag {
            out.push(("Glowing".into(), Nbt::Byte(1)));
        }
        if self.ticks_frozen() > 0 {
            out.push(("TicksFrozen".into(), Nbt::Int(self.ticks_frozen())));
        }
        if self.has_visual_fire {
            out.push(("HasVisualFire".into(), Nbt::Byte(1)));
        }
        if !self.tags.is_empty() {
            let hashes: Vec<i32> = self.tags.iter().map(|t| java_string_hash(t)).collect();
            let tags = java_hash_map_order(&hashes).into_iter().map(|i| Nbt::String(self.tags[i].clone())).collect();
            out.push(("Tags".into(), Nbt::List(tags)));
        }
        if let Some(data) = self.custom_data.as_ref().filter(|d| !matches!(d, Nbt::Compound(f) if f.is_empty())) {
            out.push(("data".into(), data.clone()));
        }
    }

    pub fn load(&mut self, nbt: &Nbt) -> Result<(), JavaThrowable> {
        let pos = tag_value_input::read(nbt, "Pos", extra_codecs::vec3).unwrap_or(Vec3::ZERO);
        let motion = tag_value_input::read(nbt, "Motion", extra_codecs::vec3).unwrap_or(Vec3::ZERO);
        let (y_rot, x_rot) = tag_value_input::read(nbt, "Rotation", extra_codecs::vec2).unwrap_or((0.0, 0.0));
        let clamp_motion = |v: f64| if v.abs() > 10.0 { 0.0 } else { v };
        self.set_delta_movement(Vec3::new(clamp_motion(motion.x), clamp_motion(motion.y), clamp_motion(motion.z)));
        self.needs_sync = true;
        let pos = Vec3::new(pos.x.clamp(-3.0000512e7, 3.0000512e7), pos.y.clamp(-2.0e7, 2.0e7), pos.z.clamp(-3.0000512e7, 3.0000512e7));
        if !pos.x.is_finite() || !pos.y.is_finite() || !pos.z.is_finite() {
            return Err(JavaThrowable::new(ILLEGAL_STATE_EXCEPTION_INVALID_POSITION, vec![ENTITY_LOAD_INVALID_POSITION]));
        }
        self.set_pos(pos);
        self.set_y_rot(y_rot % 360.0);
        self.set_x_rot(x_rot);
        self.y_head_rot = self.y_rot;
        self.set_old_pos_and_rot();
        self.fall_distance = number(nbt, "fall_distance").unwrap_or(0.0);
        self.remaining_fire_ticks = number(nbt, "Fire").map_or(0, |v| v as i16 as i32);
        let max_air_supply = self.get_max_air_supply();
        self.set_air_supply(number(nbt, "Air").map_or(max_air_supply, |v| v as i32));
        self.on_ground = flag(nbt, "OnGround");
        self.invulnerable = flag(nbt, "Invulnerable");
        self.invulnerable_time = number(nbt, "invulnerable_time").map_or(0, |v| v as i32);
        self.portal_cooldown = number(nbt, "PortalCooldown").map_or(0, |v| v as i32);
        if let Some(uuid) = tag_value_input::read(nbt, "UUID", decode_uuid) {
            self.uuid = uuid;
        }
        self.set_custom_name(tag_value_input::read(nbt, "CustomName", component_serialization::decode_custom_name));
        self.data.set(ids::CUSTOM_NAME_VISIBLE, DataValue::Boolean(flag(nbt, "CustomNameVisible")));
        self.data.set(ids::SILENT, DataValue::Boolean(flag(nbt, "Silent")));
        self.data.set(ids::NO_GRAVITY, DataValue::Boolean(flag(nbt, "NoGravity")));
        self.has_glowing_tag = flag(nbt, "Glowing");
        self.set_shared_flag(super::flags::GLOWING, self.has_glowing_tag);
        self.set_ticks_frozen(number(nbt, "TicksFrozen").map_or(0, |v| v as i32));
        self.has_visual_fire = flag(nbt, "HasVisualFire");
        self.custom_data = tag_value_input::read_or_error(nbt, "data", decode_custom_data);
        self.tags.clear();
        let tags = tag_value_input::read(nbt, "Tags", |tag| codec::sized_list_partial(tag, MAX_ENTITY_TAG_COUNT, |element| codec::string(element).map(str::to_string).into()));
        for tag in tags.unwrap_or_default() {
            if !self.tags.contains(&tag) {
                self.tags.push(tag);
            }
        }
        Ok(())
    }

    pub fn load_team(&self, nbt: &Nbt, scoreboard: &RefCell<Scoreboard>) {
        if let Some(team_name) = tag_value_input::get_string(nbt, "Team") {
            add_loaded_entity_to_team(scoreboard, self.uuid, team_name);
        }
    }
}

pub const LOADING_ENTITY_NBT: &str = "Loading entity NBT";
const REPORTED_EXCEPTION_LOADING_ENTITY_NBT: &str = "net.minecraft.ReportedException: Loading entity NBT";
const ILLEGAL_STATE_EXCEPTION_INVALID_POSITION: &str = "java.lang.IllegalStateException: Entity has invalid position";
pub const ENTITY_LOAD_INVALID_POSITION: &str = "net.minecraft.world.entity.Entity.load(Entity.java:2440)";
pub const ENTITY_LOAD_READ_ADDITIONAL_SAVE_DATA: &str = "net.minecraft.world.entity.Entity.load(Entity.java:2472)";
const ENTITY_LOAD_REPORT: &str = "net.minecraft.world.entity.Entity.load(Entity.java:2481)";
const SPECTRAL_ARROW_READ_ADDITIONAL_SAVE_DATA: &str = "net.minecraft.world.entity.projectile.arrow.SpectralArrow.readAdditionalSaveData(SpectralArrow.java:55)";
const THROWN_TRIDENT_READ_ADDITIONAL_SAVE_DATA: &str = "net.minecraft.world.entity.projectile.arrow.ThrownTrident.readAdditionalSaveData(ThrownTrident.java:200)";

const ENTITY_TYPE_CREATE: [&str; 5] = [
    "net.minecraft.world.entity.EntityType.lambda$create$1(EntityType.java:299)",
    "net.minecraft.util.Util.ifElse(Util.java:690)",
    "net.minecraft.world.entity.EntityType.create(EntityType.java:298)",
    "net.minecraft.world.entity.EntityType.loadStaticEntity(EntityType.java:397)",
    "net.minecraft.world.entity.EntityType.loadEntityRecursive(EntityType.java:362)",
];

const ENTITY_TYPE_LOAD_PASSENGERS: [&str; 4] = [
    "net.minecraft.world.entity.EntityType.loadPassengersRecursive(EntityType.java:377)",
    "net.minecraft.world.entity.EntityType.lambda$loadEntityRecursive$0(EntityType.java:364)",
    "java.base/java.util.Optional.map(Optional.java:260)",
    "net.minecraft.world.entity.EntityType.loadEntityRecursive(EntityType.java:364)",
];

const PLAYER_LOAD: [&str; 12] = [
    "java.base/java.util.Optional.ifPresent(Optional.java:178)",
    "net.minecraft.server.network.config.PrepareSpawnTask$Ready.spawn(PrepareSpawnTask.java:194)",
    "net.minecraft.server.network.config.PrepareSpawnTask.spawnPlayer(PrepareSpawnTask.java:91)",
    "net.minecraft.server.network.ServerConfigurationPacketListenerImpl.handleConfigurationFinished(ServerConfigurationPacketListenerImpl.java:178)",
    "net.minecraft.network.protocol.configuration.ServerboundFinishConfigurationPacket.handle(ServerboundFinishConfigurationPacket.java:22)",
    "net.minecraft.network.protocol.configuration.ServerboundFinishConfigurationPacket.handle(ServerboundFinishConfigurationPacket.java:8)",
    "net.minecraft.network.PacketProcessor$ListenerAndPacket.handle(PacketProcessor.java:55)",
    "net.minecraft.network.PacketProcessor.processQueuedPackets(PacketProcessor.java:38)",
    "net.minecraft.server.MinecraftServer.processPacketsAndTick(MinecraftServer.java:1011)",
    "net.minecraft.server.MinecraftServer.runServer(MinecraftServer.java:752)",
    "net.minecraft.server.MinecraftServer.lambda$spin$0(MinecraftServer.java:310)",
    "java.base/java.lang.Thread.run(Thread.java:1474)",
];

const ENTITY_TYPE_CREATE_TYPED: [&str; 5] = [
    "net.minecraft.world.entity.EntityType.lambda$create$3(EntityType.java:306)",
    "java.base/java.util.Optional.ifPresent(Optional.java:178)",
    "net.minecraft.world.entity.EntityType.create(EntityType.java:306)",
    "net.minecraft.world.entity.EntityType.loadStaticEntity(EntityType.java:406)",
    "net.minecraft.world.entity.EntityType.loadEntityRecursive(EntityType.java:369)",
];

pub const LOAD_ENTITY_RECURSIVE_REASON: &str = "net.minecraft.world.entity.EntityType.loadEntityRecursive(EntityType.java:358)";
pub const LOAD_ENTITY_RECURSIVE_TAG: &str = "net.minecraft.world.entity.EntityType.loadEntityRecursive(EntityType.java:347)";
pub const LOAD_ENTITY_RECURSIVE_TYPED: &str = "net.minecraft.world.entity.EntityType.loadEntityRecursive(EntityType.java:353)";

const BLOCK_ENTITY_TICK: [&str; 4] = [
    "net.minecraft.world.level.chunk.LevelChunk$BoundTickingBlockEntity.tick(LevelChunk.java:773)",
    "net.minecraft.world.level.chunk.LevelChunk$RebindableTickingBlockEntityWrapper.tick(LevelChunk.java:827)",
    "net.minecraft.world.level.Level.tickBlockEntities(Level.java:475)",
    "net.minecraft.server.level.ServerLevel.tick(ServerLevel.java:461)",
];

pub fn block_entity_tick_site(caller_frames: &[&'static str]) -> Vec<&'static str> {
    let mut site = caller_frames.to_vec();
    site.extend(BLOCK_ENTITY_TICK);
    site.push(crate::server::command_frames::TICK_CHILDREN_LEVELS);
    site.extend(crate::server::main_thread_frames::current());
    site
}

pub fn player_spawn_site(caller_frames: &[&'static str]) -> Vec<&'static str> {
    let mut site = caller_frames.to_vec();
    site.extend_from_slice(&PLAYER_LOAD[2..]);
    site
}

pub fn loading_entity_exception(cause: JavaThrowable, passenger_depth: usize, site: &[&'static str]) -> JavaThrowable {
    loading_entity_exception_with(cause, passenger_depth, site, false)
}

pub fn loading_entity_exception_with(cause: JavaThrowable, passenger_depth: usize, site: &[&'static str], typed: bool) -> JavaThrowable {
    let mut shared = if typed { ENTITY_TYPE_CREATE_TYPED.to_vec() } else { ENTITY_TYPE_CREATE.to_vec() };
    for _ in 0..passenger_depth {
        shared.extend(ENTITY_TYPE_LOAD_PASSENGERS);
    }
    shared.extend_from_slice(site);
    reported_loading_entity_nbt(cause, &shared)
}

pub fn player_loading_exception(cause: JavaThrowable) -> JavaThrowable {
    reported_loading_entity_nbt(cause, &PLAYER_LOAD)
}

fn reported_loading_entity_nbt(cause: JavaThrowable, shared: &[&'static str]) -> JavaThrowable {
    let mut reported_frames = vec![ENTITY_LOAD_REPORT];
    reported_frames.extend_from_slice(shared);
    let mut cause_frames = cause.stack_trace;
    cause_frames.extend_from_slice(shared);
    JavaThrowable::new(REPORTED_EXCEPTION_LOADING_ENTITY_NBT, reported_frames).with_cause(JavaThrowable::new(cause.description, cause_frames))
}

pub fn loading_entity_report(cause: &JavaThrowable) -> String {
    format!("{REPORTED_EXCEPTION_LOADING_ENTITY_NBT}\nCaused by: {}", cause.description)
}

pub fn add_loaded_entity_to_team(scoreboard: &RefCell<Scoreboard>, uuid: u128, team_name: &str) {
    let mut scoreboard = scoreboard.borrow_mut();
    let success = scoreboard.get_player_team(team_name).is_some() && scoreboard.add_player_to_team(&format_uuid(uuid), team_name);
    if !success {
        warn!("Unable to add entity to team \"{team_name}\" (that team probably doesn't exist)");
    }
}

impl LevelEntity {
    pub fn encode_id(&self) -> Option<&'static str> {
        match self {
            LevelEntity::Item(_) => Some("minecraft:item"),
            LevelEntity::PrimedTnt(_) => Some("minecraft:tnt"),
            LevelEntity::FallingBlock(_) => Some("minecraft:falling_block"),
            LevelEntity::ExperienceOrb(_) => Some("minecraft:experience_orb"),
            LevelEntity::LightningBolt(_) => None,
            LevelEntity::EyeOfEnder(_) => Some("minecraft:eye_of_ender"),
            LevelEntity::EvokerFangs(_) => Some("minecraft:evoker_fangs"),
            LevelEntity::LeashKnot(_) => None,
            LevelEntity::Painting(_) => Some("minecraft:painting"),
            LevelEntity::Cushion(_) => Some("minecraft:cushion"),
            LevelEntity::ItemFrame(frame) => Some(if frame.glow { "minecraft:glow_item_frame" } else { "minecraft:item_frame" }),
            LevelEntity::Marker(_) => Some("minecraft:marker"),
            LevelEntity::Interaction(_) => Some("minecraft:interaction"),
            LevelEntity::OminousItemSpawner(_) => Some("minecraft:ominous_item_spawner"),
            LevelEntity::ArmorStand(_) => Some("minecraft:armor_stand"),
            LevelEntity::Mannequin(_) => Some("minecraft:mannequin"),
            LevelEntity::Display(display) => Some(match display.kind {
                super::display::DisplayKind::BlockDisplay => "minecraft:block_display",
                super::display::DisplayKind::ItemDisplay => "minecraft:item_display",
                super::display::DisplayKind::TextDisplay { .. } => "minecraft:text_display",
            }),
            LevelEntity::EndCrystal(_) => Some("minecraft:end_crystal"),
            LevelEntity::Projectile(projectile) => Some(wafer_data::registry::ENTITY_TYPE[projectile.entity.entity_type as usize]),
            LevelEntity::AreaEffectCloud(_) => Some("minecraft:area_effect_cloud"),
            LevelEntity::Mob(_) | LevelEntity::Minecart(_) | LevelEntity::Boat(_) => Some(wafer_data::registry::ENTITY_TYPE[self.entity().entity_type as usize]),
        }
    }

    pub fn save(&self) -> Option<Nbt> {
        if self.entity().removal.is_some_and(|r| !r.should_save()) {
            return None;
        }
        let id = self.encode_id()?;
        let mut out = vec![("id".to_string(), Nbt::String(id.into()))];
        self.save_without_id(&mut out);
        Some(Nbt::Compound(out))
    }

    pub fn save_without_id(&self, out: &mut Vec<(String, Nbt)>) {
        self.entity().save_without_id(out);
        match self {
            LevelEntity::Item(item) => {
                out.push(("Health".into(), Nbt::Short(item.health as i16)));
                out.push(("Age".into(), Nbt::Short(item.age as i16)));
                out.push(("PickupDelay".into(), Nbt::Short(item.pickup_delay as i16)));
                if let Some(thrower) = item.thrower {
                    out.push(("Thrower".into(), uuid_ints(thrower)));
                }
                if let Some(owner) = item.target {
                    out.push(("Owner".into(), uuid_ints(owner)));
                }
                if !item.item.is_empty() {
                    out.push(("Item".into(), item.item.to_nbt()));
                }
            }
            LevelEntity::PrimedTnt(tnt) => {
                out.push(("fuse".into(), Nbt::Short(tnt.fuse() as i16)));
                out.push(("block_state".into(), state_to_nbt(tnt.block_state())));
                if tnt.explosion_power != 4.0 {
                    out.push(("explosion_power".into(), Nbt::Float(tnt.explosion_power)));
                }
                if let Some(owner) = tnt.owner {
                    out.push(("owner".into(), uuid_ints(owner)));
                }
            }
            LevelEntity::ExperienceOrb(orb) => {
                out.push(("Health".into(), Nbt::Short(orb.health as i16)));
                out.push(("Age".into(), Nbt::Short(orb.age as i16)));
                out.push(("Value".into(), Nbt::Short(orb.value() as i16)));
                out.push(("Count".into(), Nbt::Int(orb.count)));
            }
            LevelEntity::FallingBlock(falling) => falling.add_additional_save_data(out),
            LevelEntity::LightningBolt(_) => {}
            LevelEntity::EyeOfEnder(eye) => out.push(("Item".into(), eye.get_item().to_nbt())),
            LevelEntity::EvokerFangs(fangs) => fangs.add_additional_save_data(out),
            LevelEntity::LeashKnot(_) | LevelEntity::Marker(_) => {}
            LevelEntity::Interaction(interaction) => interaction.add_additional_save_data(out),
            LevelEntity::OminousItemSpawner(spawner) => spawner.add_additional_save_data(out),
            LevelEntity::ArmorStand(stand) => stand.add_additional_save_data(out),
            LevelEntity::Mannequin(mannequin) => mannequin.add_additional_save_data(out),
            LevelEntity::Display(display) => display.add_additional_save_data(out),
            LevelEntity::Painting(painting) => painting.add_additional_save_data(out),
            LevelEntity::Cushion(cushion) => cushion.add_additional_save_data(out),
            LevelEntity::ItemFrame(frame) => frame.add_additional_save_data(out),
            LevelEntity::EndCrystal(crystal) => {
                if let Some(target) = crystal.get_beam_target() {
                    out.push(("beam_target".into(), Nbt::IntArray(vec![target.x, target.y, target.z])));
                }
                out.push(("ShowBottom".into(), Nbt::Byte(crystal.shows_bottom() as i8)));
            }
            LevelEntity::Projectile(projectile) => {
                use super::projectile::projectile::ProjectileKind;
                projectile.add_additional_save_data(out);
                match projectile.kind {
                    ProjectileKind::Arrow(_) => projectile.abstract_arrow_add_additional_save_data(out),
                    ProjectileKind::SpectralArrow(_) => {
                        projectile.abstract_arrow_add_additional_save_data(out);
                        projectile.spectral_arrow_add_additional_save_data(out);
                    }
                    ProjectileKind::ThrownTrident(_) => {
                        projectile.abstract_arrow_add_additional_save_data(out);
                        projectile.thrown_trident_add_additional_save_data(out);
                    }
                    ProjectileKind::FireworkRocket(_) => projectile.firework_add_additional_save_data(out),
                    ProjectileKind::SmallFireball(_) => projectile.fireball_add_additional_save_data(out),
                    ProjectileKind::LargeFireball(_) => projectile.large_fireball_add_additional_save_data(out),
                    ProjectileKind::WindCharge(_) | ProjectileKind::BreezeWindCharge(_) => projectile.abstract_hurting_projectile_add_additional_save_data(out),
                    ProjectileKind::FishingHook(_) => {}
                    ProjectileKind::ShulkerBullet(_) => super::projectile::shulker_bullet::ShulkerBullet::add_additional_save_data(projectile, out),
                    ProjectileKind::WitherSkull(_) => projectile.wither_skull_add_additional_save_data(out),
                    ProjectileKind::DragonFireball(_) => projectile.abstract_hurting_projectile_add_additional_save_data(out),
                    _ => projectile.throwable_item_projectile_add_additional_save_data(out),
                }
            }
            LevelEntity::AreaEffectCloud(cloud) => cloud.add_additional_save_data(out),
            LevelEntity::Mob(mob) => mob.save(out),
            LevelEntity::Minecart(minecart) => minecart.add_additional_save_data(out),
            LevelEntity::Boat(boat) => {
                crate::world::entity::leashable::LeashData::write(boat.leash_data.as_ref(), out);
                if let Some(container) = &boat.container {
                    container.add_chest_vehicle_save_data(out);
                }
            }
        }
    }

    #[cfg(test)]
    pub fn load(&mut self, nbt: &Nbt) -> Result<(), JavaThrowable> {
        self.load_with(nbt, |_, _| {})
    }

    pub fn load_in_level(&mut self, nbt: &Nbt, scoreboard: &RefCell<Scoreboard>) -> Result<(), JavaThrowable> {
        self.load_with(nbt, |entity, nbt| entity.load_team(nbt, scoreboard))
    }

    pub fn load_with(&mut self, nbt: &Nbt, load_team: impl FnOnce(&Entity, &Nbt)) -> Result<(), JavaThrowable> {
        self.entity_mut().load(nbt)?;
        load_team(self.entity(), nbt);
        match self {
            LevelEntity::Item(item) => {
                item.health = number(nbt, "Health").map_or(5, |v| v as i16 as i32);
                item.age = number(nbt, "Age").map_or(0, |v| v as i16 as i32);
                item.pickup_delay = number(nbt, "PickupDelay").map_or(0, |v| v as i16 as i32);
                item.target = tag_value_input::read(nbt, "Owner", decode_uuid);
                item.thrower = tag_value_input::read(nbt, "Thrower", decode_uuid);
                let stack = tag_value_input::read(nbt, "Item", ItemStack::from_nbt_partial).unwrap_or(ItemStack::new(wafer_data::items::AIR, 0));
                item.set_item(stack);
                if item.item.is_empty() {
                    item.entity.discard();
                }
            }
            LevelEntity::PrimedTnt(tnt) => {
                tnt.set_fuse(number(nbt, "fuse").map_or(super::primed_tnt::DEFAULT_FUSE_TIME, |v| v as i16 as i32));
                tnt.set_block_state(tag_value_input::read_or_error(nbt, "block_state", crate::world::level::chunk::serialization::decode_block_state).unwrap_or(wafer_data::blocks::TNT.default_state()));
                tnt.explosion_power = number(nbt, "explosion_power").map_or(4.0, |v| v as f32).clamp(0.0, 128.0);
                tnt.owner = tag_value_input::read(nbt, "owner", decode_uuid);
            }
            LevelEntity::ExperienceOrb(orb) => {
                let short = |key: &str, default: i16| tag_value_input::get_numeric(nbt, key).and_then(int_value).map_or(default, |v| v as i16) as i32;
                orb.health = short("Health", 5);
                orb.age = short("Age", 0);
                orb.set_value(short("Value", 0));
                orb.count = tag_value_input::read_or_error(nbt, "Count", |tag| match codec::int(tag)? {
                    count if count > 0 => Ok(count),
                    count => Err(format!("Value must be positive: {count}")),
                })
                .unwrap_or(1);
            }
            LevelEntity::FallingBlock(falling) => falling.read_additional_save_data(nbt),
            LevelEntity::LightningBolt(_) => {}
            LevelEntity::EvokerFangs(fangs) => fangs.read_additional_save_data(nbt),
            LevelEntity::LeashKnot(_) | LevelEntity::Marker(_) => {}
            LevelEntity::Interaction(interaction) => interaction.read_additional_save_data(nbt),
            LevelEntity::OminousItemSpawner(spawner) => spawner.read_additional_save_data(nbt),
            LevelEntity::ArmorStand(stand) => stand.read_additional_save_data(nbt),
            LevelEntity::Mannequin(mannequin) => mannequin.read_additional_save_data(nbt),
            LevelEntity::Display(display) => display.read_additional_save_data(nbt),
            LevelEntity::Painting(painting) => painting.read_additional_save_data(nbt),
            LevelEntity::Cushion(cushion) => cushion.read_additional_save_data(nbt),
            LevelEntity::ItemFrame(frame) => frame.read_additional_save_data(nbt),
            LevelEntity::EyeOfEnder(eye) => {
                let item = tag_value_input::read(nbt, "Item", ItemStack::from_nbt_partial).unwrap_or_else(ItemStack::empty);
                eye.set_item(&item);
            }
            LevelEntity::EndCrystal(crystal) => {
                let target = tag_value_input::read(nbt, "beam_target", crate::util::extra_codecs::block_pos);
                crystal.set_beam_target(target);
                crystal.set_show_bottom(tag_value_input::get_numeric(nbt, "ShowBottom").and_then(int_value).map_or(true, |v| v != 0));
            }
            LevelEntity::Projectile(projectile) => {
                use super::projectile::projectile::ProjectileKind;
                projectile.read_additional_save_data(nbt);
                match projectile.kind {
                    ProjectileKind::Arrow(_) => {
                        projectile.abstract_arrow_read_additional_save_data(nbt).map_err(|cause| cause.with_frame(ENTITY_LOAD_READ_ADDITIONAL_SAVE_DATA))?;
                    }
                    ProjectileKind::SpectralArrow(_) => {
                        projectile
                            .abstract_arrow_read_additional_save_data(nbt)
                            .map_err(|cause| cause.with_frame(SPECTRAL_ARROW_READ_ADDITIONAL_SAVE_DATA).with_frame(ENTITY_LOAD_READ_ADDITIONAL_SAVE_DATA))?;
                        projectile.spectral_arrow_read_additional_save_data(nbt);
                    }
                    ProjectileKind::ThrownTrident(_) => {
                        projectile
                            .abstract_arrow_read_additional_save_data(nbt)
                            .map_err(|cause| cause.with_frame(THROWN_TRIDENT_READ_ADDITIONAL_SAVE_DATA).with_frame(ENTITY_LOAD_READ_ADDITIONAL_SAVE_DATA))?;
                        projectile.thrown_trident_read_additional_save_data(nbt);
                    }
                    ProjectileKind::FireworkRocket(_) => projectile.firework_read_additional_save_data(nbt),
                    ProjectileKind::SmallFireball(_) => projectile.fireball_read_additional_save_data(nbt),
                    ProjectileKind::LargeFireball(_) => projectile.large_fireball_read_additional_save_data(nbt),
                    ProjectileKind::WindCharge(_) | ProjectileKind::BreezeWindCharge(_) => projectile.abstract_hurting_projectile_read_additional_save_data(nbt),
                    ProjectileKind::FishingHook(_) => {}
                    ProjectileKind::ShulkerBullet(_) => super::projectile::shulker_bullet::ShulkerBullet::read_additional_save_data(projectile, nbt),
                    ProjectileKind::WitherSkull(_) => projectile.wither_skull_read_additional_save_data(nbt),
                    ProjectileKind::DragonFireball(_) => projectile.abstract_hurting_projectile_read_additional_save_data(nbt),
                    _ => projectile.throwable_item_projectile_read_additional_save_data(nbt),
                }
            }
            LevelEntity::AreaEffectCloud(cloud) => cloud.read_additional_save_data(nbt),
            LevelEntity::Mob(mob) => mob.load(nbt),
            LevelEntity::Minecart(minecart) => minecart.read_additional_save_data(nbt),
            LevelEntity::Boat(boat) => {
                boat.leash_data = crate::world::entity::leashable::LeashData::read(nbt);
                if let Some(container) = &mut boat.container {
                    container.read_chest_vehicle_save_data(nbt);
                }
            }
        }
        Ok(())
    }
}

pub fn apply_default_stack_config(level: &ServerLevel, entity: &mut LevelEntity, item_stack: &ItemStack) -> Option<Nbt> {
    match &mut *entity {
        LevelEntity::Mob(mob) => mob.apply_components_from_item_stack(item_stack),
        LevelEntity::Painting(painting) => painting.apply_components_from_item_stack(item_stack),
        LevelEntity::Cushion(cushion) => cushion.apply_components_from_item_stack(item_stack),
        LevelEntity::Mannequin(mannequin) => mannequin.apply_components_from_item_stack(item_stack),
        other => other.entity_mut().apply_components_from_item_stack(item_stack),
    }
    update_custom_entity_tag(level, entity, item_stack.get(crate::world::item::component::ENTITY_DATA)?)
}

fn start_riding_while_loading(loaded: &mut [LevelEntity], passenger: usize, vehicle: usize) {
    let vehicle_id = loaded[vehicle].entity().id;
    let p = loaded[passenger].entity_mut();
    p.set_pose(super::data::Pose::Standing);
    p.vehicle = Some(vehicle_id);
    let passenger_id = p.id;
    loaded[vehicle].entity_mut().passengers.push(passenger_id);
}

fn only_op_can_set_nbt(entity_type: &str) -> bool {
    matches!(entity_type, "minecraft:falling_block" | "minecraft:command_block_minecart" | "minecraft:spawner_minecart")
}

fn update_custom_entity_tag(level: &ServerLevel, entity: &mut LevelEntity, entity_data: &Nbt) -> Option<Nbt> {
    let Nbt::Compound(fields) = entity_data else { return None };
    let entity_type = wafer_data::registry::ENTITY_TYPE[entity.entity().entity_type as usize];
    if fields.iter().find(|(k, _)| k == "id").and_then(|(_, v)| v.as_str()) != Some(entity_type) {
        return None;
    }
    if only_op_can_set_nbt(entity_type) {
        return None;
    }
    let mut collector = tag_value_input::ScopedCollector::new_empty(TYPED_ENTITY_DATA_LOGGER);
    let unsafe_data: Vec<(String, Nbt)> = fields.iter().filter(|(k, _)| k != "id").cloned().collect();
    let mut tag = Vec::new();
    entity.save_without_id(&mut tag);
    let uuid = entity.entity().uuid;
    crate::nbt::merge(&mut tag, &unsafe_data);
    let tag = Nbt::Compound(tag);
    let converted = level.with_old_owner_converted(entity, &tag);
    if let Err(e) = entity.load_in_level(&converted, &level.scoreboard) {
        panic!("{}", loading_entity_report(&e));
    }
    entity.entity_mut().uuid = uuid;
    collector.set_root(PathElement::Text(level.detached_entity_to_string(entity)));
    Some(tag)
}

impl ServerLevel {
    pub fn create_entity(entity_type: &str) -> Option<LevelEntity> {
        Some(match entity_type {
            "minecraft:item" => LevelEntity::Item(ItemEntity::with_movement(Vec3::ZERO, ItemStack::new(wafer_data::items::AIR, 0), Vec3::ZERO)),
            "minecraft:tnt" => LevelEntity::PrimedTnt(PrimedTnt::of_type()),
            "minecraft:falling_block" => LevelEntity::FallingBlock(super::item::falling_block_entity::FallingBlockEntity::of_type()),
            "minecraft:experience_orb" => LevelEntity::ExperienceOrb(super::experience_orb::ExperienceOrb::of_type()),
            "minecraft:lightning_bolt" => LevelEntity::LightningBolt(super::lightning_bolt::LightningBolt::new(Vec3::ZERO)),
            "minecraft:eye_of_ender" => LevelEntity::EyeOfEnder(super::projectile::eye_of_ender::EyeOfEnder::of_type()),
            "minecraft:painting" => LevelEntity::Painting(super::decoration::painting::Painting::create_empty()),
            "minecraft:leash_knot" => LevelEntity::LeashKnot(super::decoration::leash_fence_knot_entity::LeashFenceKnotEntity::new(wafer_core::BlockPos::ZERO)),
            "minecraft:cushion" => LevelEntity::Cushion(super::decoration::cushion::Cushion::of_type()),
            "minecraft:item_frame" => LevelEntity::ItemFrame(super::decoration::item_frame::ItemFrame::create_empty(false)),
            "minecraft:glow_item_frame" => LevelEntity::ItemFrame(super::decoration::item_frame::ItemFrame::create_empty(true)),
            "minecraft:block_display" => LevelEntity::Display(super::display::Display::create_block_display()),
            "minecraft:item_display" => LevelEntity::Display(super::display::Display::create_item_display()),
            "minecraft:text_display" => LevelEntity::Display(super::display::Display::create_text_display()),
            "minecraft:interaction" => LevelEntity::Interaction(super::interaction::Interaction::create()),
            "minecraft:armor_stand" => LevelEntity::ArmorStand(Box::new(super::decoration::armor_stand::ArmorStand::of_type())),
            "minecraft:mannequin" => LevelEntity::Mannequin(Box::new(super::decoration::mannequin::Mannequin::of_type())),
            "minecraft:ominous_item_spawner" => LevelEntity::OminousItemSpawner(Box::new(super::ominous_item_spawner::OminousItemSpawner::of_type())),
            "minecraft:marker" => LevelEntity::Marker(super::marker::Marker::create()),
            "minecraft:evoker_fangs" => LevelEntity::EvokerFangs(super::projectile::evoker_fangs::EvokerFangs::of_type()),
            "minecraft:end_crystal" => LevelEntity::EndCrystal(super::boss::enderdragon::end_crystal::EndCrystal::of_type()),
            "minecraft:chicken" => LevelEntity::Mob(Box::new(super::animal::chicken::Chicken::create())),
            "minecraft:cow" => LevelEntity::Mob(Box::new(super::animal::cow::cow::Cow::create())),
            "minecraft:mooshroom" => LevelEntity::Mob(Box::new(super::animal::cow::mushroom_cow::MushroomCow::create())),
            "minecraft:pig" => LevelEntity::Mob(Box::new(super::animal::pig::pig::Pig::create())),
            "minecraft:sheep" => LevelEntity::Mob(Box::new(super::animal::sheep::sheep::Sheep::create())),
            "minecraft:goat" => LevelEntity::Mob(Box::new(super::animal::goat::goat::Goat::create())),
            "minecraft:armadillo" => LevelEntity::Mob(Box::new(super::animal::armadillo::armadillo::Armadillo::create())),
            "minecraft:sniffer" => LevelEntity::Mob(Box::new(super::animal::sniffer::sniffer::Sniffer::create())),
            "minecraft:axolotl" => LevelEntity::Mob(Box::new(super::animal::axolotl::axolotl::Axolotl::create())),
            "minecraft:frog" => LevelEntity::Mob(Box::new(super::animal::frog::frog::Frog::create())),
            "minecraft:tadpole" => LevelEntity::Mob(Box::new(super::animal::frog::tadpole::Tadpole::create())),
            "minecraft:villager" => LevelEntity::Mob(Box::new(super::npc::villager::villager::Villager::create())),
            "minecraft:zombie" => LevelEntity::Mob(Box::new(super::monster::zombie::zombie::Zombie::create())),
            "minecraft:husk" => LevelEntity::Mob(Box::new(super::monster::zombie::husk::Husk::create())),
            "minecraft:vindicator" => LevelEntity::Mob(Box::new(super::monster::illager::vindicator::Vindicator::create())),
            "minecraft:pillager" => LevelEntity::Mob(Box::new(super::monster::illager::pillager::Pillager::create())),
            "minecraft:evoker" => LevelEntity::Mob(Box::new(super::monster::illager::evoker::Evoker::create())),
            "minecraft:illusioner" => LevelEntity::Mob(Box::new(super::monster::illager::illusioner::Illusioner::create())),
            "minecraft:vex" => LevelEntity::Mob(Box::new(super::monster::vex::Vex::create())),
            "minecraft:guardian" => LevelEntity::Mob(Box::new(super::monster::guardian::Guardian::create())),
            "minecraft:elder_guardian" => LevelEntity::Mob(Box::new(super::monster::elder_guardian::ElderGuardian::create())),
            "minecraft:phantom" => LevelEntity::Mob(Box::new(super::monster::phantom::Phantom::create())),
            "minecraft:giant" => LevelEntity::Mob(Box::new(super::monster::giant::Giant::create())),
            "minecraft:wither_skeleton" => LevelEntity::Mob(Box::new(super::monster::skeleton::wither_skeleton::WitherSkeleton::create())),
            "minecraft:zombie_villager" => LevelEntity::Mob(Box::new(super::monster::zombie::zombie_villager::ZombieVillager::create())),
            "minecraft:creeper" => LevelEntity::Mob(Box::new(super::monster::creeper::Creeper::create())),
            "minecraft:spider" => LevelEntity::Mob(Box::new(super::monster::spider::spider::Spider::create())),
            "minecraft:cave_spider" => LevelEntity::Mob(Box::new(super::monster::spider::cave_spider::CaveSpider::create())),
            "minecraft:skeleton" => LevelEntity::Mob(Box::new(super::monster::skeleton::skeleton::Skeleton::create())),
            "minecraft:stray" => LevelEntity::Mob(Box::new(super::monster::skeleton::stray::Stray::create())),
            "minecraft:panda" => LevelEntity::Mob(Box::new(super::animal::panda::panda::Panda::create())),
            "minecraft:hoglin" => LevelEntity::Mob(Box::new(super::monster::hoglin::hoglin::Hoglin::create())),
            "minecraft:zoglin" => LevelEntity::Mob(Box::new(super::monster::zoglin::Zoglin::create())),
            "minecraft:breeze" => LevelEntity::Mob(Box::new(super::monster::breeze::breeze::Breeze::create())),
            "minecraft:strider" => LevelEntity::Mob(Box::new(super::monster::strider::Strider::create())),
            "minecraft:dolphin" => LevelEntity::Mob(Box::new(super::animal::dolphin::dolphin::Dolphin::create())),
            "minecraft:parrot" => LevelEntity::Mob(Box::new(super::animal::parrot::parrot::Parrot::create())),
            "minecraft:bee" => LevelEntity::Mob(Box::new(super::animal::bee::bee::Bee::create())),
            "minecraft:piglin" => LevelEntity::Mob(Box::new(super::monster::piglin::piglin::Piglin::create())),
            "minecraft:piglin_brute" => LevelEntity::Mob(Box::new(super::monster::piglin::piglin_brute::PiglinBrute::create())),
            "minecraft:parched" => LevelEntity::Mob(Box::new(super::monster::skeleton::parched::Parched::create())),
            "minecraft:bogged" => LevelEntity::Mob(Box::new(super::monster::skeleton::bogged::Bogged::create())),
            "minecraft:drowned" => LevelEntity::Mob(Box::new(super::monster::zombie::drowned::Drowned::create())),
            "minecraft:silverfish" => LevelEntity::Mob(Box::new(super::monster::silverfish::Silverfish::create())),
            "minecraft:endermite" => LevelEntity::Mob(Box::new(super::monster::endermite::Endermite::create())),
            "minecraft:enderman" => LevelEntity::Mob(Box::new(super::monster::enderman::Enderman::create())),
            "minecraft:witch" => LevelEntity::Mob(Box::new(super::monster::witch::Witch::create())),
            "minecraft:ravager" => LevelEntity::Mob(Box::new(super::monster::ravager::Ravager::create())),
            "minecraft:polar_bear" => LevelEntity::Mob(Box::new(super::animal::polarbear::polar_bear::PolarBear::create())),
            "minecraft:llama" => LevelEntity::Mob(Box::new(super::animal::equine::llama::Llama::create())),
            "minecraft:trader_llama" => LevelEntity::Mob(Box::new(super::animal::equine::trader_llama::TraderLlama::create())),
            "minecraft:camel" => LevelEntity::Mob(Box::new(super::animal::camel::camel::Camel::create())),
            "minecraft:camel_husk" => LevelEntity::Mob(Box::new(super::animal::camel::camel_husk::CamelHusk::create())),
            "minecraft:nautilus" => LevelEntity::Mob(Box::new(super::animal::nautilus::nautilus::Nautilus::create())),
            "minecraft:zombie_nautilus" => LevelEntity::Mob(Box::new(super::animal::nautilus::zombie_nautilus::ZombieNautilus::create())),
            "minecraft:happy_ghast" => LevelEntity::Mob(Box::new(super::animal::happyghast::happy_ghast::HappyGhast::create())),
            "minecraft:creaking" => LevelEntity::Mob(Box::new(super::monster::creaking::creaking::Creaking::create())),
            "minecraft:warden" => LevelEntity::Mob(Box::new(super::monster::warden::warden::Warden::create())),
            "minecraft:wandering_trader" => LevelEntity::Mob(Box::new(super::npc::wanderingtrader::wandering_trader::WanderingTrader::create())),
            "minecraft:dragon_fireball" => LevelEntity::Projectile(Box::new(super::projectile::hurtingprojectile::dragon_fireball::DragonFireball::of_type())),
            "minecraft:wither_skull" => LevelEntity::Projectile(Box::new(super::projectile::hurtingprojectile::wither_skull::WitherSkull::of_type())),
            "minecraft:wither" => LevelEntity::Mob(Box::new(super::boss::wither::wither_boss::WitherBoss::create())),
            "minecraft:ender_dragon" => LevelEntity::Mob(Box::new(super::boss::enderdragon::ender_dragon::EnderDragon::create())),
            "minecraft:shulker_bullet" => LevelEntity::Projectile(Box::new(super::projectile::shulker_bullet::ShulkerBullet::of_type())),
            "minecraft:shulker" => LevelEntity::Mob(Box::new(super::monster::shulker::Shulker::create())),
            "minecraft:llama_spit" => LevelEntity::Projectile(Box::new(super::projectile::llama_spit::LlamaSpit::of_type())),
            "minecraft:skeleton_horse" => LevelEntity::Mob(Box::new(super::animal::equine::skeleton_horse::SkeletonHorse::create())),
            "minecraft:zombie_horse" => LevelEntity::Mob(Box::new(super::animal::equine::zombie_horse::ZombieHorse::create())),
            "minecraft:slime" => LevelEntity::Mob(Box::new(super::monster::cubemob::slime::Slime::create())),
            "minecraft:magma_cube" => LevelEntity::Mob(Box::new(super::monster::cubemob::magma_cube::MagmaCube::create())),
            "minecraft:sulfur_cube" => LevelEntity::Mob(Box::new(super::monster::cubemob::sulfur_cube::SulfurCube::create())),
            "minecraft:wolf" => LevelEntity::Mob(Box::new(super::animal::wolf::wolf::Wolf::create())),
            "minecraft:bat" => LevelEntity::Mob(Box::new(super::ambient::bat::Bat::create())),
            "minecraft:squid" => LevelEntity::Mob(Box::new(super::animal::squid::squid::Squid::create())),
            "minecraft:glow_squid" => LevelEntity::Mob(Box::new(super::animal::squid::glow_squid::GlowSquid::create())),
            "minecraft:iron_golem" => LevelEntity::Mob(Box::new(super::animal::golem::iron_golem::IronGolem::create())),
            "minecraft:allay" => LevelEntity::Mob(Box::new(super::animal::allay::allay::Allay::create())),
            "minecraft:copper_golem" => LevelEntity::Mob(Box::new(super::animal::golem::copper_golem::CopperGolem::create())),
            "minecraft:snow_golem" => LevelEntity::Mob(Box::new(super::animal::golem::snow_golem::SnowGolem::create())),
            "minecraft:cod" => LevelEntity::Mob(Box::new(super::animal::fish::cod::Cod::create())),
            "minecraft:salmon" => LevelEntity::Mob(Box::new(super::animal::fish::salmon::Salmon::create())),
            "minecraft:pufferfish" => LevelEntity::Mob(Box::new(super::animal::fish::pufferfish::Pufferfish::create())),
            "minecraft:tropical_fish" => LevelEntity::Mob(Box::new(super::animal::fish::tropical_fish::TropicalFish::create())),
            "minecraft:cat" => LevelEntity::Mob(Box::new(super::animal::feline::cat::Cat::create())),
            "minecraft:ocelot" => LevelEntity::Mob(Box::new(super::animal::feline::ocelot::Ocelot::create())),
            "minecraft:horse" => LevelEntity::Mob(Box::new(super::animal::equine::horse::Horse::create())),
            "minecraft:donkey" => LevelEntity::Mob(Box::new(super::animal::equine::donkey::Donkey::create())),
            "minecraft:mule" => LevelEntity::Mob(Box::new(super::animal::equine::mule::Mule::create())),
            "minecraft:fox" => LevelEntity::Mob(Box::new(super::animal::fox::fox::Fox::create())),
            "minecraft:rabbit" => LevelEntity::Mob(Box::new(super::animal::rabbit::rabbit::Rabbit::create())),
            "minecraft:turtle" => LevelEntity::Mob(Box::new(super::animal::turtle::turtle::Turtle::create())),
            "minecraft:snowball" => LevelEntity::Projectile(Box::new(throwableitemprojectile::snowball::Snowball::of_type())),
            "minecraft:egg" => LevelEntity::Projectile(Box::new(throwableitemprojectile::thrown_egg::ThrownEgg::of_type())),
            "minecraft:ender_pearl" => LevelEntity::Projectile(Box::new(throwableitemprojectile::thrown_enderpearl::ThrownEnderpearl::of_type())),
            "minecraft:experience_bottle" => LevelEntity::Projectile(Box::new(throwableitemprojectile::thrown_experience_bottle::ThrownExperienceBottle::of_type())),
            "minecraft:splash_potion" => LevelEntity::Projectile(Box::new(throwableitemprojectile::thrown_splash_potion::ThrownSplashPotion::of_type())),
            "minecraft:lingering_potion" => LevelEntity::Projectile(Box::new(throwableitemprojectile::thrown_lingering_potion::ThrownLingeringPotion::of_type())),
            "minecraft:arrow" => LevelEntity::Projectile(Box::new(super::projectile::arrow::arrow::Arrow::of_type())),
            "minecraft:spectral_arrow" => LevelEntity::Projectile(Box::new(super::projectile::arrow::spectral_arrow::SpectralArrow::of_type())),
            "minecraft:trident" => LevelEntity::Projectile(Box::new(super::projectile::arrow::thrown_trident::ThrownTrident::of_type())),
            "minecraft:firework_rocket" => LevelEntity::Projectile(Box::new(super::projectile::firework_rocket_entity::FireworkRocketEntity::of_type())),
            "minecraft:small_fireball" => LevelEntity::Projectile(Box::new(super::projectile::hurtingprojectile::small_fireball::SmallFireball::of_type())),
            "minecraft:wind_charge" => LevelEntity::Projectile(Box::new(super::projectile::hurtingprojectile::windcharge::wind_charge::WindCharge::of_type())),
            "minecraft:breeze_wind_charge" => LevelEntity::Projectile(Box::new(super::projectile::hurtingprojectile::windcharge::breeze_wind_charge::BreezeWindCharge::of_type())),
            "minecraft:fireball" => LevelEntity::Projectile(Box::new(super::projectile::hurtingprojectile::large_fireball::LargeFireball::of_type())),
            "minecraft:blaze" => LevelEntity::Mob(Box::new(super::monster::blaze::Blaze::create())),
            "minecraft:ghast" => LevelEntity::Mob(Box::new(super::monster::ghast::Ghast::create())),
            "minecraft:zombified_piglin" => LevelEntity::Mob(Box::new(super::monster::zombie::zombified_piglin::ZombifiedPiglin::create())),
            "minecraft:area_effect_cloud" => LevelEntity::AreaEffectCloud(Box::new(super::area_effect_cloud::AreaEffectCloud::of_type())),
            "minecraft:minecart" => LevelEntity::Minecart(Box::new(super::vehicle::minecart::minecart::Minecart::create())),
            "minecraft:chest_minecart" => LevelEntity::Minecart(Box::new(super::vehicle::minecart::minecart_chest::MinecartChest::create())),
            "minecraft:furnace_minecart" => LevelEntity::Minecart(Box::new(super::vehicle::minecart::minecart_furnace::MinecartFurnace::create())),
            "minecraft:hopper_minecart" => LevelEntity::Minecart(Box::new(super::vehicle::minecart::minecart_hopper::MinecartHopper::create())),
            "minecraft:tnt_minecart" => LevelEntity::Minecart(Box::new(super::vehicle::minecart::minecart_tnt::MinecartTnt::create())),
            "minecraft:command_block_minecart" => LevelEntity::Minecart(Box::new(super::vehicle::minecart::minecart_command_block::MinecartCommandBlock::create())),
            "minecraft:spawner_minecart" => LevelEntity::Minecart(Box::new(super::vehicle::minecart::minecart_spawner::MinecartSpawner::create())),
            _ => {
                let boat = super::vehicle::boat::boat::create(entity_type)
                    .or_else(|| super::vehicle::boat::raft::create(entity_type))
                    .or_else(|| super::vehicle::boat::chest_boat::create(entity_type))
                    .or_else(|| super::vehicle::boat::chest_raft::create(entity_type))?;
                LevelEntity::Boat(Box::new(boat))
            }
        })
    }

    fn get_y_offset(&self, spawn_pos: BlockPos, moved_up: bool, entity_box: &AABB) -> f64 {
        let mut aabb = AABB::of_block(spawn_pos);
        if moved_up {
            aabb = aabb.expand_towards(0.0, -1.0, 0.0);
        }
        let shapes: Vec<Collider> = self.block_collisions(&aabb, &crate::world::phys::shapes::collision_context::CollisionContext::EMPTY, false).into_iter().map(|(_, collider)| collider).collect();
        1.0 + shapes_collide(Axis::Y, entity_box, &shapes, if moved_up { -2.0 } else { -1.0 })
    }

    pub fn spawn_entity_type(&mut self, entity_type: &str, spawn_pos: BlockPos, spawn_reason: EntitySpawnReason) -> Option<EntityId> {
        self.spawn_entity_type_from_stack(entity_type, None, spawn_pos, spawn_reason, false, false)
    }

    pub fn spawn_entity_type_from_stack(
        &mut self,
        entity_type: &str,
        item_stack: Option<&ItemStack>,
        spawn_pos: BlockPos,
        spawn_reason: EntitySpawnReason,
        try_move_down: bool,
        moved_up: bool,
    ) -> Option<EntityId> {
        let entity = self.create_entity_type_from_stack(entity_type, item_stack, spawn_pos, spawn_reason, try_move_down, moved_up)?;
        let id = entity.entity().id;
        let is_mob = matches!(entity, LevelEntity::Mob(_));
        self.add_fresh_entity_with_passengers(entity);
        if is_mob {
            self.play_ambient_sound(id);
        }
        Some(id)
    }

    pub fn create_entity_type_from_stack(
        &mut self,
        entity_type: &str,
        item_stack: Option<&ItemStack>,
        spawn_pos: BlockPos,
        spawn_reason: EntitySpawnReason,
        try_move_down: bool,
        moved_up: bool,
    ) -> Option<LevelEntity> {
        let mut entity = ServerLevel::create_entity(entity_type)?;
        if let LevelEntity::Mob(mob) = &mut entity
            && let super::mob::MobKind::Villager(_) = mob.kind
        {
            self.villager_register_brain_goals_detached(mob, Vec3::ZERO);
        }
        let y_off = if try_move_down {
            entity.entity_mut().set_pos(Vec3::new(spawn_pos.x as f64 + 0.5, (spawn_pos.y + 1) as f64, spawn_pos.z as f64 + 0.5));
            self.get_y_offset(spawn_pos, moved_up, &entity.entity().bounding_box())
        } else {
            0.0
        };
        let y_rot = mth::wrap_degrees(self.random.next_float() * 360.0);
        entity.entity_mut().snap_to(spawn_pos.x as f64 + 0.5, spawn_pos.y as f64 + y_off, spawn_pos.z as f64 + 0.5, y_rot, 0.0);
        if let LevelEntity::Mob(mob) = &mut entity {
            mob.entity.y_head_rot = mob.entity.y_rot;
            mob.living.y_body_rot = mob.entity.y_rot;
        }
        if matches!(entity, LevelEntity::Mob(_)) {
            let difficulty = self.current_difficulty_at(entity.entity().block_position());
            entity = self.finalize_spawn_detached(entity, &difficulty, spawn_reason, None).0;
        }
        if let Some(item_stack) = item_stack
            && let Some(tag) = apply_default_stack_config(self, &mut entity, item_stack)
        {
            entity = self.reassess_weapon_goal_detached(entity);
            entity = self.read_level_dependent_save_data_detached(entity, &tag);
        }
        Some(entity)
    }

    pub fn save_entity(&self, id: EntityId) -> Option<Nbt> {
        if self.is_passenger(id) { None } else { self.save_as_passenger(id) }
    }

    pub fn save_as_passenger(&self, id: EntityId) -> Option<Nbt> {
        let entity = self.find_entity(id)?;
        if entity.entity().removal.is_some_and(|r| !r.should_save()) {
            return None;
        }
        let encode_id = entity.encode_id()?;
        let mut out = vec![("id".to_string(), Nbt::String(encode_id.into()))];
        self.save_without_id(id, &mut out);
        Some(Nbt::Compound(out))
    }

    pub fn save_without_id(&self, id: EntityId, out: &mut Vec<(String, Nbt)>) {
        let start = out.len();
        match self.find_player(id) {
            Some(player) => player.entity.save_without_id(out),
            None => match self.find_entity(id) {
                Some(entity) => entity.save_without_id(out),
                None => self.dragon_part_entity(id).expect("entity in this level").save_without_id(out),
            },
        }
        self.save_vehicle_position(id, &mut out[start..]);
        if self.is_vehicle(id) {
            let mut passengers: Vec<Nbt> = Vec::new();
            for &passenger in self.passengers(id) {
                let _scope = tag_value_input::enter_indexed_field("Passengers", passengers.len());
                passengers.extend(self.save_as_passenger(passenger));
            }
            if !passengers.is_empty() {
                out.push(("Passengers".into(), Nbt::List(passengers)));
            }
        }
    }

    pub fn save_vehicle_position(&self, id: EntityId, out: &mut [(String, Nbt)]) {
        if let Some(vehicle) = self.vehicle(id) {
            let (v, y) = (self.entity(vehicle), self.entity(id).y());
            if let Some(pos) = out.iter_mut().find(|(k, _)| k == "Pos") {
                pos.1 = Nbt::List(vec![Nbt::Double(v.x()), Nbt::Double(y), Nbt::Double(v.z())]);
            }
        }
    }

    pub fn should_be_saved(&self, id: EntityId) -> bool {
        if self.entity(id).removal.is_some_and(|r| !r.should_save()) {
            return false;
        }
        if self.is_passenger(id) { false } else { !self.is_vehicle(id) || !self.has_exactly_one_player_passenger(id) }
    }

    pub fn load_entity_recursive(&mut self, nbt: &Nbt, site: &[&'static str], post_load: &mut dyn FnMut(&mut LevelEntity)) -> Vec<LevelEntity> {
        let _collector = tag_value_input::ScopedCollector::new_empty(ENTITY_TYPE_LOGGER);
        self.load_entity_recursive_in_scope(nbt, site, post_load)
    }

    pub fn load_entity_recursive_in_scope(&mut self, nbt: &Nbt, site: &[&'static str], post_load: &mut dyn FnMut(&mut LevelEntity)) -> Vec<LevelEntity> {
        let mut loaded = Vec::new();
        self.load_entity_recursive_into(nbt, true, site, 0, post_load, &mut loaded);
        loaded
    }

    pub fn load_entity_recursive_for_spawn(&mut self, nbt: &Nbt, site: &[&'static str], post_load: &mut dyn FnMut(&mut LevelEntity)) -> Vec<LevelEntity> {
        let mut loaded = Vec::new();
        self.load_entity_recursive_into(nbt, false, site, 0, post_load, &mut loaded);
        loaded
    }

    fn load_entity_recursive_into(
        &mut self,
        nbt: &Nbt,
        ignore_checks: bool,
        site: &[&'static str],
        depth: usize,
        post_load: &mut dyn FnMut(&mut LevelEntity),
        loaded: &mut Vec<LevelEntity>,
    ) -> Option<usize> {
        if !ignore_checks
            && let Some(entity_type) = nbt.get("id").and_then(Nbt::as_str)
            && super::entity_type::EntityType::by_name(entity_type).is_some_and(|ty| !ty.can_spawn(self.difficulty))
        {
            warn!("Skipping Entity with id {entity_type}");
            return None;
        }
        let mut entity = self.load_entity_at(nbt, depth, site)?;
        post_load(&mut entity);
        let index = loaded.len();
        loaded.push(entity);
        tag_value_input::for_each_child(nbt, "Passengers", |passenger_tag| {
            if let Some(passenger) = self.load_entity_recursive_into(passenger_tag, ignore_checks, site, depth + 1, post_load, loaded) {
                start_riding_while_loading(loaded, passenger, index);
            }
        });
        Some(index)
    }

    pub fn try_add_fresh_entity_with_passengers(&mut self, entities: Vec<LevelEntity>) -> bool {
        if entities.iter().any(|entity| self.entity_manager.is_loaded(entity.entity().uuid)) {
            return false;
        }
        for entity in entities {
            self.add_fresh_entity_with_passengers(entity);
        }
        true
    }

    pub fn with_old_owner_converted<'a>(&self, entity: &LevelEntity, nbt: &'a Nbt) -> std::borrow::Cow<'a, Nbt> {
        let reads_owner = matches!(entity, LevelEntity::Mob(mob) if mob.tamable_animal().is_some() || mob.abstract_horse().is_some());
        let Nbt::Compound(fields) = nbt else { return std::borrow::Cow::Borrowed(nbt) };
        if !reads_owner || !matches!(nbt.get("Owner"), Some(Nbt::String(_))) {
            return std::borrow::Cow::Borrowed(nbt);
        }
        let owner = super::entity_reference::EntityReference::read_with_old_owner_conversion(nbt, "Owner", self);
        let mut fields = fields.clone();
        fields.retain(|(key, _)| key != "Owner");
        if let Some(owner) = owner {
            fields.push(("Owner".into(), uuid_ints(owner.get_uuid())));
        }
        std::borrow::Cow::Owned(Nbt::Compound(fields))
    }

    pub fn load_entity(&mut self, nbt: &Nbt) -> Option<LevelEntity> {
        self.load_entity_at(nbt, 0, &[])
    }

    pub fn load_entity_at(&mut self, nbt: &Nbt, passenger_depth: usize, site: &[&'static str]) -> Option<LevelEntity> {
        self.load_entity_with(nbt, passenger_depth, &|| site.to_vec(), false)
    }

    pub fn load_entity_typed(&mut self, nbt: &Nbt, site: &dyn Fn() -> Vec<&'static str>) -> Option<LevelEntity> {
        self.load_entity_with(nbt, 0, site, true)
    }

    fn load_entity_with(&mut self, nbt: &Nbt, passenger_depth: usize, site: &dyn Fn() -> Vec<&'static str>, typed: bool) -> Option<LevelEntity> {
        let entity_type = nbt.get("id").and_then(Nbt::as_str)?;
        let Some(mut entity) = ServerLevel::create_entity(entity_type) else {
            warn!("Skipping Entity with id {entity_type}");
            return None;
        };
        if let Err(cause) = entity.load_in_level(&self.with_old_owner_converted(&entity, nbt), &self.scoreboard) {
            warn!("Exception loading entity: \n{}", loading_entity_exception_with(cause, passenger_depth, &site(), typed).print_stack_trace().trim_end());
            return None;
        }
        let entity = self.reassess_weapon_goal_detached(entity);
        let entity = self.read_level_dependent_save_data_detached(entity, nbt);
        Some(self.resolve_display_text_detached(entity))
    }

    pub fn entity_tag_to_compare(&self, id: EntityId) -> Nbt {
        self.entity_tag_to_compare_with(id, &|pearl| self.other_levels.iter().find(|level| level.entities.contains_key(&pearl)))
    }

    pub fn entity_tag_to_compare_with<'a>(&'a self, id: EntityId, other_level_pearl: &dyn Fn(EntityId) -> Option<&'a ServerLevel>) -> Nbt {
        self.entity_tag_to_compare_logged(id, other_level_pearl, NBT_PREDICATE_LOGGER)
    }

    pub fn entity_tag_to_compare_logged<'a>(&'a self, id: EntityId, other_level_pearl: &dyn Fn(EntityId) -> Option<&'a ServerLevel>, logger: &'static str) -> Nbt {
        let mut collector = tag_value_input::ScopedCollector::new_empty(logger);
        let tag = self.entity_tag_to_compare_unreported(id, other_level_pearl);
        if collector.has_problems() {
            collector.set_root(PathElement::Text(self.entity_to_string(id)));
        }
        tag
    }

    fn entity_tag_to_compare_unreported<'a>(&'a self, id: EntityId, other_level_pearl: &dyn Fn(EntityId) -> Option<&'a ServerLevel>) -> Nbt {
        if let Some(player) = self.find_player(id) {
            let Nbt::Compound(mut fields) = self.save_player_with_ender_pearls(id, other_level_pearl) else { unreachable!() };
            if let Some(selected) = player.inventory.slots[player.inventory.selected_slot()].as_ref().filter(|s| !s.is_empty()) {
                fields.push(("SelectedItem".into(), selected.to_nbt()));
            }
            return Nbt::Compound(fields);
        }
        let mut fields = Vec::new();
        self.save_without_id(id, &mut fields);
        Nbt::Compound(fields)
    }
}

impl RemovalReason {
    pub fn should_save(self) -> bool {
        matches!(self, RemovalReason::UnloadedToChunk)
    }

    pub fn should_destroy(self) -> bool {
        matches!(self, RemovalReason::Killed | RemovalReason::Discarded)
    }
}

#[cfg(test)]
mod tests {
    use wafer_core::Vec3;
    use wafer_data::items;

    use super::*;
    use crate::server::level::testing;

    #[test]
    fn items_and_tnt_round_trip() {
        let mut level = testing::level();
        let mut item = ItemEntity::with_movement(Vec3::new(1.5, -60.0, 2.5), ItemStack::new(items::DIAMOND, 3), Vec3::new(0.1, 0.0, 0.0));
        item.pickup_delay = 10;
        item.entity.tags = vec!["b".into(), "a".into()];
        let saved = LevelEntity::Item(item).save().unwrap();
        assert_eq!(saved.get("id").and_then(Nbt::as_str), Some("minecraft:item"));
        let LevelEntity::Item(loaded) = level.load_entity(&saved).unwrap() else { panic!("not an item") };
        assert_eq!((loaded.item.item, loaded.item.count, loaded.pickup_delay), (items::DIAMOND, 3, 10));
        assert_eq!(loaded.entity.position(), Vec3::new(1.5, -60.0, 2.5));
        assert_eq!(LevelEntity::Item(loaded).save(), Some(saved));

        let mut tnt = PrimedTnt::new(&mut level, Vec3::new(0.5, -60.0, 0.5), Some(7));
        tnt.set_fuse(33);
        let saved = LevelEntity::PrimedTnt(tnt).save().unwrap();
        let LevelEntity::PrimedTnt(loaded) = level.load_entity(&saved).unwrap() else { panic!("not TNT") };
        assert_eq!((loaded.fuse(), loaded.owner), (33, Some(7)));
        let bolt = super::super::lightning_bolt::LightningBolt::new(Vec3::ZERO);
        assert_eq!(LevelEntity::LightningBolt(bolt).save(), None);
    }

    fn marker_tag(uuid: u128, team: Nbt) -> Nbt {
        Nbt::Compound(vec![("id".into(), Nbt::String("minecraft:marker".into())), ("UUID".into(), uuid_ints(uuid)), ("Team".into(), team)])
    }

    fn team_of(level: &ServerLevel, uuid: u128) -> Option<String> {
        level.scoreboard.borrow().get_players_team(&format_uuid(uuid)).map(|team| team.name().to_string())
    }

    #[test]
    fn team_field_adds_loaded_entities_to_the_team() {
        let mut level = testing::level();
        level.scoreboard.borrow_mut().add_player_team("red");
        level.load_entity(&marker_tag(1, Nbt::String("red".into()))).unwrap();
        level.load_entity(&marker_tag(2, Nbt::String("blue".into()))).unwrap();
        level.load_entity(&marker_tag(3, Nbt::Int(5))).unwrap();
        assert_eq!(team_of(&level, 1).as_deref(), Some("red"));
        assert_eq!(team_of(&level, 2), None);
        assert_eq!(team_of(&level, 3), None);
    }

    #[test]
    fn team_field_in_entity_data_adds_the_entity_to_the_team() {
        let mut level = testing::level();
        level.scoreboard.borrow_mut().add_player_team("red");
        let mut entity = ServerLevel::create_entity("minecraft:marker").unwrap();
        let uuid = entity.entity().uuid;
        let data = Nbt::Compound(vec![("id".into(), Nbt::String("minecraft:marker".into())), ("Team".into(), Nbt::String("red".into()))]);
        let mut stack = ItemStack::new(wafer_data::items::STONE, 1);
        stack.set(crate::world::item::component::ENTITY_DATA, Some(data));
        apply_default_stack_config(&level, &mut entity, &stack).unwrap();
        assert_eq!(team_of(&level, uuid).as_deref(), Some("red"));
    }

    fn positioned_marker_tag(pos: [f64; 3], rotation: [f32; 2]) -> Nbt {
        Nbt::Compound(vec![
            ("id".into(), Nbt::String("minecraft:marker".into())),
            ("Pos".into(), Nbt::List(pos.iter().map(|&v| Nbt::Double(v)).collect())),
            ("Rotation".into(), Nbt::List(rotation.iter().map(|&v| Nbt::Float(v)).collect())),
        ])
    }

    #[test]
    fn entities_with_a_non_finite_position_are_not_loaded() {
        let mut level = testing::level();
        assert!(level.load_entity(&positioned_marker_tag([f64::NAN, 0.0, 0.0], [0.0, 0.0])).is_none());
        let mut entity = ServerLevel::create_entity("minecraft:marker").unwrap();
        let cause = entity.load(&positioned_marker_tag([0.0, 0.0, f64::NAN], [0.0, 0.0])).unwrap_err();
        assert_eq!(cause.description, "java.lang.IllegalStateException: Entity has invalid position");
        assert!(level.load_entity(&positioned_marker_tag([1.0, 2.0, 3.0], [0.0, 0.0])).is_some());
        let trace = loading_entity_exception(cause, 1, &["Site.frame(Site.java:1)"]).print_stack_trace();
        assert!(trace.starts_with("net.minecraft.ReportedException: Loading entity NBT\n\tat net.minecraft.world.entity.Entity.load(Entity.java:2481)\n\tat net.minecraft.world.entity.EntityType.lambda$create$1(EntityType.java:299)\n"));
        assert!(trace.contains("\tat net.minecraft.world.entity.EntityType.loadPassengersRecursive(EntityType.java:377)\n"));
        assert!(trace.ends_with("\tat Site.frame(Site.java:1)\nCaused by: java.lang.IllegalStateException: Entity has invalid position\n\tat net.minecraft.world.entity.Entity.load(Entity.java:2440)\n\t... 10 more\n"));
    }

    fn arrow_tag(pickup: Nbt) -> Nbt {
        Nbt::Compound(vec![("id".into(), Nbt::String("minecraft:arrow".into())), ("inBlockState".into(), Nbt::String("minecraft:not_a_block".into())), ("pickup".into(), pickup)])
    }

    #[test]
    fn arrows_with_pickup_three_fail_to_load_like_an_out_of_bounds_ordinal() {
        let mut level = testing::level();
        for pickup in [Nbt::Byte(3), Nbt::Int(259), Nbt::Double(3.7), Nbt::Short(-253)] {
            let mut entity = ServerLevel::create_entity("minecraft:arrow").unwrap();
            let cause = entity.load(&arrow_tag(pickup.clone())).unwrap_err();
            assert_eq!(cause.description, "java.lang.ArrayIndexOutOfBoundsException: Index 3 out of bounds for length 3", "{pickup:?}");
            assert_eq!(cause.stack_trace.first(), Some(&"net.minecraft.world.entity.projectile.arrow.AbstractArrow$Pickup.byOrdinal(AbstractArrow.java:81)"));
            assert_eq!(cause.stack_trace.last(), Some(&ENTITY_LOAD_READ_ADDITIONAL_SAVE_DATA));
            assert!(level.load_entity(&arrow_tag(pickup)).is_none());
        }
        for pickup in [Nbt::Byte(0), Nbt::Byte(2), Nbt::Byte(4), Nbt::Byte(-3), Nbt::Int(127)] {
            assert!(level.load_entity(&arrow_tag(pickup)).is_some());
        }
    }

    #[test]
    fn passenger_arrows_with_pickup_three_are_skipped_but_their_vehicle_loads() {
        let mut level = testing::level();
        let cow = Nbt::Compound(vec![("id".into(), Nbt::String("minecraft:cow".into())), ("Passengers".into(), Nbt::List(vec![arrow_tag(Nbt::Byte(3))]))]);
        let loaded = level.load_entity_recursive(&cow, &[], &mut |_| {});
        assert_eq!(loaded.len(), 1);
    }

    #[test]
    fn typed_loads_use_the_typed_create_frames() {
        let cause = JavaThrowable::new("java.lang.IllegalStateException: boom", vec!["Cause.frame(Cause.java:1)"]);
        let trace = loading_entity_exception_with(cause, 0, &[LOAD_ENTITY_RECURSIVE_TYPED, "Site.frame(Site.java:2)"], true).print_stack_trace();
        let expected = "net.minecraft.ReportedException: Loading entity NBT\n\
            \tat net.minecraft.world.entity.Entity.load(Entity.java:2481)\n\
            \tat net.minecraft.world.entity.EntityType.lambda$create$3(EntityType.java:306)\n\
            \tat java.base/java.util.Optional.ifPresent(Optional.java:178)\n\
            \tat net.minecraft.world.entity.EntityType.create(EntityType.java:306)\n\
            \tat net.minecraft.world.entity.EntityType.loadStaticEntity(EntityType.java:406)\n\
            \tat net.minecraft.world.entity.EntityType.loadEntityRecursive(EntityType.java:369)\n\
            \tat net.minecraft.world.entity.EntityType.loadEntityRecursive(EntityType.java:353)\n\
            \tat Site.frame(Site.java:2)\n\
            Caused by: java.lang.IllegalStateException: boom\n\
            \tat Cause.frame(Cause.java:1)\n\
            \t... 7 more\n";
        assert_eq!(trace, expected);
    }

    #[test]
    fn block_entity_and_player_sites_append_vanilla_callers() {
        let site = block_entity_tick_site(&["Caller.frame(Caller.java:3)"]);
        assert_eq!(site[0], "Caller.frame(Caller.java:3)");
        assert_eq!(&site[1..5], &BLOCK_ENTITY_TICK);
        assert_eq!(site[5], crate::server::command_frames::TICK_CHILDREN_LEVELS);
        let site = player_spawn_site(&["Caller.frame(Caller.java:3)"]);
        assert_eq!(site[0], "Caller.frame(Caller.java:3)");
        assert_eq!(&site[1..], &PLAYER_LOAD[2..]);
    }

    #[test]
    fn non_finite_rotations_are_discarded_while_the_entity_loads() {
        let mut level = testing::level();
        let entity = level.load_entity(&positioned_marker_tag([1.0, 2.0, 3.0], [f32::NAN, 10.0])).expect("a marker");
        assert_eq!((entity.entity().y_rot, entity.entity().x_rot), (0.0, 10.0));
        let entity = level.load_entity(&positioned_marker_tag([1.0, 2.0, 3.0], [400.0, f32::INFINITY])).expect("a marker");
        assert_eq!((entity.entity().y_rot, entity.entity().x_rot), (40.0, 0.0));
    }
}
