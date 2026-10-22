use wafer_core::{BlockPos, Vec3};
use wafer_protocol::nbt::Nbt;

use crate::world::level::is_in_spawnable_bounds;
use crate::commands::brigadier::{CommandContext, CommandSyntaxException, DynamicCommandExceptionType, SimpleCommandExceptionType, tr};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::{Arg, Component, HoverEvent};
use crate::server::Server;
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::level::server_level_accessor::ServerLevelAccessor;
use crate::world::entity::{EntityId, LevelEntity};

static ERROR_FAILED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.summon.failed");
static ERROR_FAILED_PEACEFUL: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.summon.failed.peaceful");
static ERROR_DUPLICATE_UUID: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.summon.failed.uuid");
static INVALID_POSITION: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.summon.invalidPosition");
static ERROR_NOT_SUMMONABLE_ENTITY: DynamicCommandExceptionType = DynamicCommandExceptionType(|value| tr("entity.not_summonable", vec![value]));

pub(crate) const MODELLED: &[&str] = &[
    "minecraft:item",
    "minecraft:tnt",
    "minecraft:falling_block",
    "minecraft:lightning_bolt",
    "minecraft:experience_orb",
    "minecraft:chicken",
    "minecraft:cow",
    "minecraft:mooshroom",
    "minecraft:pig",
    "minecraft:sheep",
    "minecraft:goat",
    "minecraft:armadillo",
    "minecraft:sniffer",
    "minecraft:camel",
    "minecraft:camel_husk",
    "minecraft:nautilus",
    "minecraft:zombie_nautilus",
    "minecraft:happy_ghast",
    "minecraft:creaking",
    "minecraft:axolotl",
    "minecraft:frog",
    "minecraft:tadpole",
    "minecraft:end_crystal",
    "minecraft:eye_of_ender",
    "minecraft:evoker_fangs",
    "minecraft:villager",
    "minecraft:zombie",
    "minecraft:husk",
    "minecraft:giant",
    "minecraft:zombie_villager",
    "minecraft:creeper",
    "minecraft:spider",
    "minecraft:cave_spider",
    "minecraft:skeleton",
    "minecraft:stray",
    "minecraft:wither_skeleton",
    "minecraft:bogged",
    "minecraft:parched",
    "minecraft:hoglin",
    "minecraft:panda",
    "minecraft:zoglin",
    "minecraft:drowned",
    "minecraft:silverfish",
    "minecraft:endermite",
    "minecraft:enderman",
    "minecraft:witch",
    "minecraft:vindicator",
    "minecraft:pillager",
    "minecraft:evoker",
    "minecraft:illusioner",
    "minecraft:ravager",
    "minecraft:polar_bear",
    "minecraft:llama",
    "minecraft:trader_llama",
    "minecraft:wandering_trader",
    "minecraft:llama_spit",
    "minecraft:skeleton_horse",
    "minecraft:zombie_horse",
    "minecraft:vex",
    "minecraft:phantom",
    "minecraft:marker",
    "minecraft:interaction",
    "minecraft:painting",
    "minecraft:item_frame",
    "minecraft:glow_item_frame",
    "minecraft:block_display",
    "minecraft:item_display",
    "minecraft:text_display",
    "minecraft:chest_minecart",
    "minecraft:furnace_minecart",
    "minecraft:hopper_minecart",
    "minecraft:tnt_minecart",
    "minecraft:spawner_minecart",
    "minecraft:command_block_minecart",
    "minecraft:guardian",
    "minecraft:elder_guardian",
    "minecraft:slime",
    "minecraft:magma_cube",
    "minecraft:wolf",
    "minecraft:bat",
    "minecraft:squid",
    "minecraft:glow_squid",
    "minecraft:iron_golem",
    "minecraft:snow_golem",
    "minecraft:cod",
    "minecraft:salmon",
    "minecraft:pufferfish",
    "minecraft:tropical_fish",
    "minecraft:cat",
    "minecraft:ocelot",
    "minecraft:horse",
    "minecraft:donkey",
    "minecraft:mule",
    "minecraft:fox",
    "minecraft:rabbit",
    "minecraft:turtle",
    "minecraft:snowball",
    "minecraft:egg",
    "minecraft:ender_pearl",
    "minecraft:experience_bottle",
    "minecraft:splash_potion",
    "minecraft:lingering_potion",
    "minecraft:arrow",
    "minecraft:spectral_arrow",
    "minecraft:trident",
    "minecraft:firework_rocket",
    "minecraft:small_fireball",
    "minecraft:fireball",
    "minecraft:wind_charge",
    "minecraft:breeze_wind_charge",
    "minecraft:blaze",
    "minecraft:ghast",
    "minecraft:zombified_piglin",
    "minecraft:piglin",
    "minecraft:piglin_brute",
    "minecraft:breeze",
    "minecraft:strider",
    "minecraft:dolphin",
    "minecraft:parrot",
    "minecraft:bee",
    "minecraft:shulker",
    "minecraft:shulker_bullet",
    "minecraft:wither",
    "minecraft:wither_skull",
    "minecraft:dragon_fireball",
    "minecraft:ominous_item_spawner",
    "minecraft:armor_stand",
    "minecraft:cushion",
    "minecraft:mannequin",
    "minecraft:area_effect_cloud",
    "minecraft:minecart",
    "minecraft:acacia_boat",
    "minecraft:birch_boat",
    "minecraft:cherry_boat",
    "minecraft:dark_oak_boat",
    "minecraft:jungle_boat",
    "minecraft:mangrove_boat",
    "minecraft:oak_boat",
    "minecraft:pale_oak_boat",
    "minecraft:poplar_boat",
    "minecraft:spruce_boat",
    "minecraft:bamboo_raft",
    "minecraft:acacia_chest_boat",
    "minecraft:birch_chest_boat",
    "minecraft:cherry_chest_boat",
    "minecraft:dark_oak_chest_boat",
    "minecraft:jungle_chest_boat",
    "minecraft:mangrove_chest_boat",
    "minecraft:oak_chest_boat",
    "minecraft:pale_oak_chest_boat",
    "minecraft:poplar_chest_boat",
    "minecraft:spruce_chest_boat",
    "minecraft:bamboo_chest_raft",
];

pub fn summonable_entity_type(context: &CommandContext, name: &str) -> Result<String, CommandSyntaxException> {
    let id = context.get_identifier(name).to_string();
    if id == "minecraft:player" || id == "minecraft:fishing_bobber" {
        return Err(ERROR_NOT_SUMMONABLE_ENTITY.create(id));
    }
    Ok(id)
}

pub fn create_entity(server: &mut Server, source: &CommandSourceStack, entity_type: &str, pos: Vec3, nbt: &Nbt, finalize: bool) -> Result<(EntityId, Component), CommandSyntaxException> {
    if !is_in_spawnable_bounds(BlockPos::containing(pos.x, pos.y, pos.z)) {
        return Err(INVALID_POSITION.create());
    }
    if server.levels[source.level].difficulty == crate::world::difficulty::Difficulty::Peaceful
        && crate::world::entity::entity_type::EntityType::by_name(entity_type).is_some_and(|ty| !ty.is_allowed_in_peaceful())
    {
        return Err(ERROR_FAILED_PEACEFUL.create());
    }
    if !MODELLED.contains(&entity_type) {
        let hover = Component::literal(format!("{entity_type} entities are not implemented in Wafer yet"));
        return Err(CommandSyntaxException::new(Component::translatable("command.failed").with_hover(HoverEvent::ShowText(Box::new(hover)))));
    }
    let mut tag = match nbt {
        Nbt::Compound(fields) => fields.clone(),
        _ => Vec::new(),
    };
    tag.retain(|(k, _)| k != "id");
    tag.push(("id".into(), Nbt::String(entity_type.into())));
    let level = &mut server.levels[source.level];
    let mut loaded = level.load_entity_recursive(&Nbt::Compound(tag), &mut |entity| {
        let e = entity.entity_mut();
        let (y_rot, x_rot) = (e.y_rot, e.x_rot);
        e.snap_to(pos.x, pos.y, pos.z, y_rot, x_rot);
    });
    if loaded.is_empty() {
        return Err(ERROR_FAILED.create());
    }
    if finalize && matches!(loaded[0], LevelEntity::Mob(_)) {
        let entity = loaded.remove(0);
        let difficulty = level.current_difficulty_at(entity.entity().block_position());
        let (entity, _) = level.finalize_spawn_detached(entity, &difficulty, EntitySpawnReason::Command, None);
        loaded.insert(0, entity);
    }
    if loaded.iter().any(|e| level.entity_manager.is_loaded(e.entity().uuid)) {
        return Err(ERROR_DUPLICATE_UUID.create());
    }
    let id = loaded[0].entity().id;
    let (entity, name) = level.display_name_detached(loaded.remove(0));
    loaded.insert(0, entity);
    for entity in loaded {
        let entity = server.resolve_detached_display_text(source.level, entity);
        server.levels[source.level].add_fresh_entity_with_passengers(entity);
    }
    Ok((id, name))
}

fn spawn_entity(server: &mut Server, source: &CommandSourceStack, entity_type: &str, pos: Vec3, nbt: &Nbt, finalize: bool) -> Result<i32, CommandSyntaxException> {
    let (_, name) = create_entity(server, source, entity_type, pos, nbt, finalize)?;
    source.send_success(server, || Component::translatable_with("commands.summon.success", vec![Arg::Component(name)]), true);
    Ok(1)
}

pub fn summon(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let entity_type = summonable_entity_type(context, "entity")?;
    spawn_entity(server, source, &entity_type, source.position, &Nbt::Compound(Vec::new()), true)
}

pub fn summon_at(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let entity_type = summonable_entity_type(context, "entity")?;
    let pos = context.get_coordinates("pos").position(server, source);
    spawn_entity(server, source, &entity_type, pos, &Nbt::Compound(Vec::new()), true)
}

pub fn summon_with_nbt(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let entity_type = summonable_entity_type(context, "entity")?;
    let pos = context.get_coordinates("pos").position(server, source);
    let nbt = context.get_nbt("nbt").clone();
    spawn_entity(server, source, &entity_type, pos, &nbt, false)
}
