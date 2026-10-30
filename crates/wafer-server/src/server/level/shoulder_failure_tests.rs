use wafer_core::Vec3;
use wafer_data::packets::play::serverbound;
use wafer_protocol::nbt::Nbt;
use wafer_protocol::Writer;

use super::{ServerLevel, testing};
use crate::commands::perform_prefixed_command;
use crate::commands::source::CommandSourceStack;
use crate::crash_report::{ReportType, ReportedException};
use crate::network::connection::Packet;
use crate::network::login::{Profile, offline_uuid};
use crate::server::{entity_tick_frames, main_thread_frames};
use crate::world::InteractionHand;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;

const ORIGIN: Vec3 = Vec3::new(0.5, -60.0, 0.5);

const SHOULDER_LOAD_HEAD: [&str; 8] = [
    "net.minecraft.world.entity.Entity.load(Entity.java:2481)",
    "net.minecraft.world.entity.EntityType.lambda$create$1(EntityType.java:299)",
    "net.minecraft.util.Util.ifElse(Util.java:690)",
    "net.minecraft.world.entity.EntityType.create(EntityType.java:298)",
    "net.minecraft.server.level.ServerPlayer.respawnEntityOnShoulder(ServerPlayer.java:911)",
    "net.minecraft.server.level.ServerPlayer.removeEntitiesOnShoulder(ServerPlayer.java:900)",
    "net.minecraft.world.entity.player.Player.hurtServer(Player.java:739)",
    "net.minecraft.server.level.ServerPlayer.hurtServer(ServerPlayer.java:1078)",
];

const PACKET_TAIL: [&str; 6] = [
    "net.minecraft.network.PacketProcessor$ListenerAndPacket.handle(PacketProcessor.java:55)",
    "net.minecraft.network.PacketProcessor.processQueuedPackets(PacketProcessor.java:38)",
    "net.minecraft.server.MinecraftServer.processPacketsAndTick(MinecraftServer.java:1011)",
    "net.minecraft.server.MinecraftServer.runServer(MinecraftServer.java:752)",
    "net.minecraft.server.MinecraftServer.lambda$spin$0(MinecraftServer.java:310)",
    "java.base/java.lang.Thread.run(Thread.java:1474)",
];

const TICK_TAIL: [&str; 6] = [
    "net.minecraft.server.MinecraftServer.tickServer(MinecraftServer.java:980)",
    "net.minecraft.server.dedicated.DedicatedServer.tickServer(DedicatedServer.java:278)",
    "net.minecraft.server.MinecraftServer.processPacketsAndTick(MinecraftServer.java:1013)",
    "net.minecraft.server.MinecraftServer.runServer(MinecraftServer.java:752)",
    "net.minecraft.server.MinecraftServer.lambda$spin$0(MinecraftServer.java:310)",
    "java.base/java.lang.Thread.run(Thread.java:1474)",
];

const ENTITY_TICK_TAIL: [&str; 6] = [
    "net.minecraft.server.level.ServerLevel.tickNonPassenger(ServerLevel.java:860)",
    "net.minecraft.world.level.Level.guardEntityTick(Level.java:484)",
    "net.minecraft.server.level.ServerLevel.lambda$tick$0(ServerLevel.java:457)",
    "net.minecraft.world.level.entity.EntityTickList.forEach(EntityTickList.java:53)",
    "net.minecraft.server.level.ServerLevel.tick(ServerLevel.java:427)",
    "net.minecraft.server.MinecraftServer.tickChildren(MinecraftServer.java:1116)",
];

const MELEE: [&str; 2] = [
    "net.minecraft.world.entity.ai.goal.MeleeAttackGoal.checkAndPerformAttack(MeleeAttackGoal.java:151)",
    "net.minecraft.world.entity.ai.goal.MeleeAttackGoal.tick(MeleeAttackGoal.java:144)",
];

const ZOMBIE_GOAL: [&str; 1] = ["net.minecraft.world.entity.ai.goal.ZombieAttackGoal.tick(ZombieAttackGoal.java:28)"];

const EVEN_SELECTOR: [&str; 4] = [
    "net.minecraft.world.entity.ai.goal.WrappedGoal.tick(WrappedGoal.java:66)",
    "net.minecraft.world.entity.ai.goal.GoalSelector.tickRunningGoals(GoalSelector.java:105)",
    "net.minecraft.world.entity.ai.goal.GoalSelector.tick(GoalSelector.java:96)",
    "net.minecraft.world.entity.Mob.serverAiStep(Mob.java:800)",
];

const ODD_SELECTOR: [&str; 3] = [
    "net.minecraft.world.entity.ai.goal.WrappedGoal.tick(WrappedGoal.java:66)",
    "net.minecraft.world.entity.ai.goal.GoalSelector.tickRunningGoals(GoalSelector.java:105)",
    "net.minecraft.world.entity.Mob.serverAiStep(Mob.java:808)",
];

const MONSTER_STEP: [&str; 3] = [
    "net.minecraft.world.entity.LivingEntity.aiStep(LivingEntity.java:3187)",
    "net.minecraft.world.entity.Mob.aiStep(Mob.java:513)",
    "net.minecraft.world.entity.monster.Monster.aiStep(Monster.java:44)",
];

const MOB_TICK: [&str; 2] = ["net.minecraft.world.entity.LivingEntity.tick(LivingEntity.java:2886)", "net.minecraft.world.entity.Mob.tick(Mob.java:379)"];

const MOB_DO_HURT_TARGET: &str = "net.minecraft.world.entity.Mob.doHurtTarget(Mob.java:1494)";
const ZOMBIE_DO_HURT_TARGET: &str = "net.minecraft.world.entity.monster.zombie.Zombie.doHurtTarget(Zombie.java:317)";
const ZOMBIE_TICK: &str = "net.minecraft.world.entity.monster.zombie.Zombie.tick(Zombie.java:234)";

fn doubles(values: [f64; 3]) -> Nbt {
    Nbt::List(values.into_iter().map(Nbt::Double).collect())
}

fn bad_shoulder() -> Nbt {
    Nbt::Compound(vec![("id".into(), Nbt::String("minecraft:arrow".into())), ("pickup".into(), Nbt::Byte(3))].into())
}

fn thrown_frames(thrown: Box<dyn std::any::Any + Send>) -> Vec<&'static str> {
    let reported = thrown.downcast::<ReportedException>().expect("a reported exception");
    assert_eq!(reported.0.title(), "Loading entity NBT");
    reported.0.reported_exception().stack_trace.clone()
}

fn expected(parts: &[&[&'static str]]) -> Vec<&'static str> {
    parts.iter().flat_map(|part| part.iter().copied()).collect()
}

fn shoulder_level() -> (ServerLevel, EntityId) {
    let mut level = testing::level();
    let player = testing::player(&mut level, ORIGIN, 0.0, 0.0);
    level.handle_player_loaded(player);
    level.game_time = 100;
    level.player_mut(player).set_shoulder_entity_left(bad_shoulder());
    (level, player)
}

fn packet(id: i32, write: impl FnOnce(&mut Writer)) -> Packet {
    let mut w = Writer::new();
    write(&mut w);
    Packet { id, data: w.into_inner() }
}

#[test]
fn a_shoulder_entity_failing_during_a_player_attack_matches_vanilla() {
    let (mut level, victim) = shoulder_level();
    let attacker = testing::player_with_profile(&mut level, Profile { uuid: 2, name: "Attacker".into(), properties: Vec::new() }, ORIGIN, 0.0, 0.0);
    level.handle_player_loaded(attacker);
    let attack = packet(serverbound::ATTACK, |w| w.varint(victim));
    let thrown = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| level.handle_game_packet(attacker, &attack))).unwrap_err();
    assert_eq!(
        thrown_frames(thrown),
        expected(&[
            &SHOULDER_LOAD_HEAD,
            &[
                "net.minecraft.world.entity.Entity.hurtOrSimulate(Entity.java:2163)",
                "net.minecraft.world.entity.player.Player.attack(Player.java:1042)",
                "net.minecraft.server.network.ServerGamePacketListenerImpl.handleAttack(ServerGamePacketListenerImpl.java:2032)",
                "net.minecraft.network.protocol.game.ServerboundAttackPacket.handle(ServerboundAttackPacket.java:24)",
                "net.minecraft.network.protocol.game.ServerboundAttackPacket.handle(ServerboundAttackPacket.java:9)",
            ],
            &PACKET_TAIL,
        ])
    );
    assert_eq!(level.player(victim).shoulder_entity_left, bad_shoulder());
}

#[test]
fn a_shoulder_entity_failing_on_a_riptide_matches_vanilla() {
    let mut level = testing::level();
    let player = testing::player(&mut level, ORIGIN, 0.0, -10.0);
    level.handle_player_loaded(player);
    let mut stack = ItemStack::new(wafer_data::items::TRIDENT, 1);
    crate::world::item::enchantment::enchantment_helper::enchant(&mut stack, "minecraft:riptide", 3);
    level.player_mut(player).set_item_in_hand(InteractionHand::MainHand, Some(stack.clone()));
    level.entity_mut(player).was_touching_water = true;
    crate::world::item::use_item(&mut level, player, InteractionHand::MainHand, stack);
    for _ in 0..12 {
        level.tick();
        level.server_player_do_tick(player);
    }
    level.entity_mut(player).was_touching_water = true;
    level.game_time = 100;
    level.player_mut(player).set_shoulder_entity_left(bad_shoulder());
    let release = packet(serverbound::PLAYER_ACTION, |w| {
        w.varint(6);
        w.position(0, -60, 0);
        w.u8(1);
        w.varint(0);
    });
    let thrown = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| level.handle_game_packet(player, &release))).unwrap_err();
    assert_eq!(
        thrown_frames(thrown),
        expected(&[
            &SHOULDER_LOAD_HEAD[..6],
            &[
                "net.minecraft.world.entity.player.Player.startAutoSpinAttack(Player.java:553)",
                "net.minecraft.world.item.TridentItem.releaseUsing(TridentItem.java:119)",
                "net.minecraft.world.item.ItemStack.releaseUsing(ItemStack.java:756)",
                "net.minecraft.world.entity.LivingEntity.releaseUsingItem(LivingEntity.java:3746)",
                "net.minecraft.server.network.ServerGamePacketListenerImpl.handlePlayerAction(ServerGamePacketListenerImpl.java:1470)",
                "net.minecraft.network.protocol.game.ServerboundPlayerActionPacket.handle(ServerboundPlayerActionPacket.java:50)",
                "net.minecraft.network.protocol.game.ServerboundPlayerActionPacket.handle(ServerboundPlayerActionPacket.java:10)",
            ],
            &PACKET_TAIL,
        ])
    );
}

#[test]
fn a_shoulder_entity_failing_when_the_game_mode_packet_selects_spectator_matches_vanilla() {
    let mut server = testing::server();
    let profile = Profile { uuid: offline_uuid("Shoulder"), name: "Shoulder".into(), properties: Vec::new() };
    let player = testing::server_player_with_profile(&mut server, profile, ORIGIN, 0.0, 0.0);
    server.name_to_id_cache.add(crate::server::players::NameAndId { id: offline_uuid("Shoulder"), name: "Shoulder".into() });
    let source = CommandSourceStack::server(&server);
    perform_prefixed_command(&mut server, &source, "op Shoulder");
    let level = &mut server.levels[0];
    level.handle_player_loaded(player);
    level.game_time = 100;
    level.player_mut(player).set_shoulder_entity_left(bad_shoulder());
    let change = packet(serverbound::CHANGE_GAME_MODE, |w| w.varint(3));
    let thrown = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| server.handle_change_game_mode(player, &change))).unwrap_err();
    assert_eq!(
        thrown_frames(thrown),
        expected(&[
            &SHOULDER_LOAD_HEAD[..6],
            &[
                "net.minecraft.server.level.ServerPlayer.setGameMode(ServerPlayer.java:1914)",
                "net.minecraft.server.commands.GameModeCommand.setGameMode(GameModeCommand.java:70)",
                "net.minecraft.server.commands.GameModeCommand.setGameMode(GameModeCommand.java:66)",
                "net.minecraft.server.network.ServerGamePacketListenerImpl.handleChangeGameMode(ServerGamePacketListenerImpl.java:2347)",
                "net.minecraft.network.protocol.game.ServerboundChangeGameModePacket.handle(ServerboundChangeGameModePacket.java:24)",
                "net.minecraft.network.protocol.game.ServerboundChangeGameModePacket.handle(ServerboundChangeGameModePacket.java:9)",
            ],
            &PACKET_TAIL,
        ])
    );
}

fn spawn_mob(level: &mut ServerLevel, kind: &str, odd_tick: bool) -> EntityId {
    let tag = Nbt::Compound(vec![("id".into(), Nbt::String(kind.into())), ("Pos".into(), doubles([0.5, -60.0, 1.5]))].into());
    let mob = level.load_entity(&tag).expect("a mob");
    let id = mob.entity().id;
    assert!(level.add_entity(mob));
    level.mob_mut(id).unwrap().entity.tick_count = if (id % 2 != 0) == odd_tick { 10 } else { 11 };
    id
}

fn melee_attack_frames(kind: &str, odd_tick: bool) -> Vec<&'static str> {
    let (mut level, player) = shoulder_level();
    let mob = spawn_mob(&mut level, kind, odd_tick);
    let _tick = main_thread_frames::tick();
    let _scope = entity_tick_frames::melee_attack(&level, mob, player);
    let thrown = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| level.do_hurt_target(mob, player))).unwrap_err();
    thrown_frames(thrown)
}

#[test]
fn a_shoulder_entity_failing_on_a_zombie_attack_matches_vanilla() {
    let hurt = [MOB_DO_HURT_TARGET, ZOMBIE_DO_HURT_TARGET];
    let even = expected(&[&SHOULDER_LOAD_HEAD, &hurt, &MELEE, &ZOMBIE_GOAL, &EVEN_SELECTOR, &MONSTER_STEP, &MOB_TICK, &[ZOMBIE_TICK], &ENTITY_TICK_TAIL, &TICK_TAIL]);
    assert_eq!(melee_attack_frames("minecraft:zombie", false), even);
    let odd = expected(&[&SHOULDER_LOAD_HEAD, &hurt, &MELEE, &ZOMBIE_GOAL, &ODD_SELECTOR, &MONSTER_STEP, &MOB_TICK, &[ZOMBIE_TICK], &ENTITY_TICK_TAIL, &TICK_TAIL]);
    assert_eq!(melee_attack_frames("minecraft:zombie", true), odd);
}

#[test]
fn a_shoulder_entity_failing_on_other_melee_attacks_matches_vanilla() {
    let husk = expected(&[
        &SHOULDER_LOAD_HEAD,
        &[MOB_DO_HURT_TARGET, ZOMBIE_DO_HURT_TARGET, "net.minecraft.world.entity.monster.zombie.Husk.doHurtTarget(Husk.java:70)"],
        &MELEE,
        &ZOMBIE_GOAL,
        &EVEN_SELECTOR,
        &MONSTER_STEP,
        &MOB_TICK,
        &[ZOMBIE_TICK],
        &ENTITY_TICK_TAIL,
        &TICK_TAIL,
    ]);
    assert_eq!(melee_attack_frames("minecraft:husk", false), husk);
    let villager = expected(&[
        &SHOULDER_LOAD_HEAD,
        &[MOB_DO_HURT_TARGET, ZOMBIE_DO_HURT_TARGET],
        &MELEE,
        &ZOMBIE_GOAL,
        &EVEN_SELECTOR,
        &MONSTER_STEP,
        &MOB_TICK,
        &[ZOMBIE_TICK, "net.minecraft.world.entity.monster.zombie.ZombieVillager.tick(ZombieVillager.java:176)"],
        &ENTITY_TICK_TAIL,
        &TICK_TAIL,
    ]);
    assert_eq!(melee_attack_frames("minecraft:zombie_villager", false), villager);
    let vindicator = expected(&[
        &SHOULDER_LOAD_HEAD,
        &[MOB_DO_HURT_TARGET],
        &MELEE,
        &ODD_SELECTOR,
        &MONSTER_STEP,
        &["net.minecraft.world.entity.raid.Raider.aiStep(Raider.java:117)"],
        &MOB_TICK,
        &ENTITY_TICK_TAIL,
        &TICK_TAIL,
    ]);
    assert_eq!(melee_attack_frames("minecraft:vindicator", true), vindicator);
    let wither_skeleton = expected(&[
        &SHOULDER_LOAD_HEAD,
        &[MOB_DO_HURT_TARGET, "net.minecraft.world.entity.monster.skeleton.WitherSkeleton.doHurtTarget(WitherSkeleton.java:97)"],
        &MELEE,
        &ODD_SELECTOR,
        &MONSTER_STEP,
        &MOB_TICK,
        &ENTITY_TICK_TAIL,
        &TICK_TAIL,
    ]);
    assert_eq!(melee_attack_frames("minecraft:wither_skeleton", true), wither_skeleton);
    let skeleton = expected(&[
        &SHOULDER_LOAD_HEAD,
        &[MOB_DO_HURT_TARGET],
        &MELEE,
        &EVEN_SELECTOR,
        &MONSTER_STEP,
        &MOB_TICK,
        &["net.minecraft.world.entity.monster.skeleton.Skeleton.tick(Skeleton.java:54)"],
        &ENTITY_TICK_TAIL,
        &TICK_TAIL,
    ]);
    assert_eq!(melee_attack_frames("minecraft:skeleton", false), skeleton);
}

#[test]
fn a_shoulder_entity_failing_on_a_creeper_explosion_matches_vanilla() {
    let (mut level, _player) = shoulder_level();
    let tag = Nbt::Compound(vec![
        ("id".into(), Nbt::String("minecraft:creeper".into())),
        ("Pos".into(), doubles([0.5, -60.0, 1.5])),
        ("Fuse".into(), Nbt::Short(1)),
        ("ignited".into(), Nbt::Byte(1)),
    ].into());
    let creeper = level.load_entity(&tag).expect("a creeper");
    let id = creeper.entity().id;
    assert!(level.add_entity(creeper));
    let _tick = main_thread_frames::tick();
    let thrown = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        for _ in 0..4 {
            level.creeper_tick(id);
        }
    }))
    .unwrap_err();
    assert_eq!(
        thrown_frames(thrown),
        expected(&[
            &SHOULDER_LOAD_HEAD,
            &[
                "net.minecraft.world.level.ServerExplosion.hurtEntities(ServerExplosion.java:201)",
                "net.minecraft.world.level.ServerExplosion.explode(ServerExplosion.java:248)",
                "net.minecraft.server.level.ServerLevel.explode(ServerLevel.java:1218)",
                "net.minecraft.world.level.Level.explode(Level.java:529)",
                "net.minecraft.world.entity.monster.Creeper.explodeCreeper(Creeper.java:244)",
                "net.minecraft.world.entity.monster.Creeper.tick(Creeper.java:157)",
            ],
            &ENTITY_TICK_TAIL,
            &TICK_TAIL,
        ])
    );
}

#[test]
fn a_shoulder_entity_failing_during_an_entity_tick_crashes_with_the_vanilla_report_sections() {
    let (mut level, player) = shoulder_level();
    let zombie = spawn_mob(&mut level, "minecraft:zombie", false);
    let _tick = main_thread_frames::tick();
    let thrown = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        level.guard_entity_tick(zombie, |level| {
            let _scope = entity_tick_frames::melee_attack(level, zombie, player);
            level.do_hurt_target(zombie, player);
        })
    }))
    .unwrap_err();
    let report = thrown.downcast::<ReportedException>().expect("a reported exception").0;
    let text = report.get_friendly_report(&ReportType::CRASH).replace("\r\n", "\n");
    let sections: Vec<&str> = text.split("\n\n").filter(|section| section.starts_with("-- ")).collect();
    let titles: Vec<&str> = sections.iter().map(|section| section.lines().next().unwrap()).collect();
    assert_eq!(titles, ["-- Head --", "-- Entity being loaded --", "-- Entity being ticked --", "-- System Details --"]);
    let head: Vec<&str> = sections[0].lines().collect();
    assert_eq!(head[2], "Stacktrace:");
    assert_eq!(head[3], "\tat net.minecraft.world.entity.projectile.arrow.AbstractArrow$Pickup.byOrdinal(AbstractArrow.java:81)");
    assert_eq!(*head.last().unwrap(), "\tat net.minecraft.server.level.ServerLevel.tickNonPassenger(ServerLevel.java:860)");
    let loaded: Vec<&str> = sections[1].lines().collect();
    assert_eq!(loaded[2], "\tEntity Type: minecraft:arrow (net.minecraft.world.entity.projectile.arrow.Arrow)");
    assert_eq!(&loaded[4..6], ["\tEntity Name: Arrow", "\tEntity's Exact location: 0.00, 0.00, 0.00"]);
    assert_eq!(loaded[10], "Stacktrace:");
    assert_eq!(loaded[11], "\tat net.minecraft.world.entity.Entity.load(Entity.java:2472)");
    assert_eq!(*loaded.last().unwrap(), "\tat java.base/java.lang.Thread.run(Thread.java:1474)");
    let ticked: Vec<&str> = sections[2].lines().skip_while(|line| *line != "Stacktrace:").skip(1).map(|line| line.trim_start_matches("\tat ")).collect();
    assert_eq!(ticked, expected(&[&ENTITY_TICK_TAIL[1..], &TICK_TAIL]));
}
