use crate::commands::brigadier::{CommandContext, CommandSyntaxException};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::Component;
use crate::server::Server;
use crate::server::command_origin::CommandOrigin;

const COMMAND_EXECUTION: [&str; 7] = [
    "com.mojang.brigadier.context.ContextChain.runExecutable(ContextChain.java:73)",
    "net.minecraft.commands.execution.tasks.ExecuteCommand.execute(ExecuteCommand.java:29)",
    "net.minecraft.commands.execution.tasks.ExecuteCommand.execute(ExecuteCommand.java:13)",
    "net.minecraft.commands.execution.UnboundEntryAction.lambda$bind$0(UnboundEntryAction.java:8)",
    "net.minecraft.commands.execution.CommandQueueEntry.execute(CommandQueueEntry.java:8)",
    "net.minecraft.commands.execution.ExecutionContext.runCommandQueue(ExecutionContext.java:106)",
    "net.minecraft.commands.Commands.executeCommandInContext(Commands.java:438)",
];

const PERFORM_COMMAND: [&str; 2] = [
    "net.minecraft.commands.Commands.performCommand(Commands.java:370)",
    "net.minecraft.commands.Commands.performPrefixedCommand(Commands.java:356)",
];

const FUNCTION_EXECUTE: &str = "net.minecraft.server.ServerFunctionManager.execute(ServerFunctionManager.java:71)";

const TICK_LOOP: [&str; 6] = [
    "net.minecraft.server.MinecraftServer.tickServer(MinecraftServer.java:980)",
    "net.minecraft.server.dedicated.DedicatedServer.tickServer(DedicatedServer.java:278)",
    "net.minecraft.server.MinecraftServer.processPacketsAndTick(MinecraftServer.java:1013)",
    "net.minecraft.server.MinecraftServer.runServer(MinecraftServer.java:752)",
    "net.minecraft.server.MinecraftServer.lambda$spin$0(MinecraftServer.java:310)",
    "java.base/java.lang.Thread.run(Thread.java:1474)",
];

const TICK_CHILDREN_LEVELS: &str = "net.minecraft.server.MinecraftServer.tickChildren(MinecraftServer.java:1116)";

const CONSOLE_SOURCE: [&str; 3] = [
    "net.minecraft.server.dedicated.DedicatedServer.handleConsoleInputs(DedicatedServer.java:412)",
    "net.minecraft.server.dedicated.DedicatedServer.tickConnection(DedicatedServer.java:402)",
    "net.minecraft.server.MinecraftServer.tickChildren(MinecraftServer.java:1127)",
];

const BLOCK_TICK: [&str; 6] = [
    "net.minecraft.world.level.block.CommandBlock.tick(CommandBlock.java:104)",
    "net.minecraft.world.level.block.state.BlockBehaviour$BlockStateBase.tick(BlockBehaviour.java:1153)",
    "net.minecraft.server.level.ServerLevel.tickBlock(ServerLevel.java:851)",
    "net.minecraft.world.ticks.LevelTicks.runCollectedTicks(LevelTicks.java:213)",
    "net.minecraft.world.ticks.LevelTicks.tick(LevelTicks.java:96)",
    "net.minecraft.server.level.ServerLevel.tick(ServerLevel.java:392)",
];

const BASE_COMMAND_BLOCK: &str = "net.minecraft.world.level.BaseCommandBlock.performCommand(BaseCommandBlock.java:114)";

const ENTITY_TICK: [&str; 5] = [
    "net.minecraft.server.level.ServerLevel.tickNonPassenger(ServerLevel.java:860)",
    "net.minecraft.world.level.Level.guardEntityTick(Level.java:484)",
    "net.minecraft.server.level.ServerLevel.lambda$tick$0(ServerLevel.java:457)",
    "net.minecraft.world.level.entity.EntityTickList.forEach(EntityTickList.java:53)",
    "net.minecraft.server.level.ServerLevel.tick(ServerLevel.java:427)",
];

const TIMER_TICK: [&str; 3] = [
    "net.minecraft.world.level.timers.TimerQueue.tick(TimerQueue.java:102)",
    "net.minecraft.server.level.ServerLevel.tickTime(ServerLevel.java:495)",
    "net.minecraft.server.level.ServerLevel.tick(ServerLevel.java:385)",
];

const TASK_QUEUE: [&str; 11] = [
    "net.minecraft.server.TickTask.run(TickTask.java:18)",
    "net.minecraft.util.thread.BlockableEventLoop.doRunTask(BlockableEventLoop.java:173)",
    "net.minecraft.util.thread.ReentrantBlockableEventLoop.doRunTask(ReentrantBlockableEventLoop.java:23)",
    "net.minecraft.server.MinecraftServer.doRunTask(MinecraftServer.java:912)",
    "net.minecraft.server.MinecraftServer.doRunTask(MinecraftServer.java:193)",
    "net.minecraft.util.thread.BlockableEventLoop.pollTask(BlockableEventLoop.java:147)",
    "net.minecraft.server.MinecraftServer.pollTaskInternal(MinecraftServer.java:894)",
    "net.minecraft.server.MinecraftServer.pollTask(MinecraftServer.java:888)",
    "net.minecraft.util.thread.BlockableEventLoop.managedBlock(BlockableEventLoop.java:156)",
    "net.minecraft.server.MinecraftServer.waitUntilNextTick(MinecraftServer.java:859)",
    "net.minecraft.server.MinecraftServer.runServer(MinecraftServer.java:758)",
];

const SPIN: [&str; 2] = ["net.minecraft.server.MinecraftServer.lambda$spin$0(MinecraftServer.java:310)", "java.base/java.lang.Thread.run(Thread.java:1474)"];

pub const RELOAD_COMMAND_FRAME: &str = "net.minecraft.server.commands.ReloadCommand.lambda$register$0(ReloadCommand.java:53)";

fn origin_frames(origin: CommandOrigin) -> Vec<&'static str> {
    let mut frames = Vec::new();
    match origin {
        CommandOrigin::Console => {
            frames.extend(PERFORM_COMMAND);
            frames.extend(CONSOLE_SOURCE);
            frames.extend(TICK_LOOP);
        }
        CommandOrigin::PlayerChat => {
            frames.push(PERFORM_COMMAND[0]);
            frames.push("net.minecraft.server.network.ServerGamePacketListenerImpl.performUnsignedChatCommand(ServerGamePacketListenerImpl.java:1717)");
            frames.push("net.minecraft.server.network.ServerGamePacketListenerImpl.lambda$handleChatCommand$0(ServerGamePacketListenerImpl.java:1705)");
            frames.extend(TASK_QUEUE);
            frames.extend(SPIN);
        }
        CommandOrigin::Rcon => {
            frames.extend(PERFORM_COMMAND);
            frames.push("net.minecraft.server.dedicated.DedicatedServer.lambda$runCommand$0(DedicatedServer.java:698)");
            frames.push("net.minecraft.util.thread.BlockableEventLoop.lambda$submitAsync$0(BlockableEventLoop.java:75)");
            frames.push("java.base/java.util.concurrent.CompletableFuture$AsyncSupply.run(CompletableFuture.java:1789)");
            frames.extend(TASK_QUEUE);
            frames.extend(SPIN);
        }
        CommandOrigin::CommandBlock | CommandOrigin::ChainCommandBlock => {
            frames.extend(PERFORM_COMMAND);
            frames.push(BASE_COMMAND_BLOCK);
            if origin == CommandOrigin::ChainCommandBlock {
                frames.push("net.minecraft.world.level.block.CommandBlock.executeChain(CommandBlock.java:215)");
                frames.push("net.minecraft.world.level.block.CommandBlock.execute(CommandBlock.java:121)");
            } else {
                frames.push("net.minecraft.world.level.block.CommandBlock.execute(CommandBlock.java:116)");
            }
            frames.extend(BLOCK_TICK);
            frames.push(TICK_CHILDREN_LEVELS);
            frames.extend(TICK_LOOP);
        }
        CommandOrigin::MinecartCommandBlock => {
            frames.extend(PERFORM_COMMAND);
            frames.push(BASE_COMMAND_BLOCK);
            frames.push("net.minecraft.world.entity.vehicle.minecart.MinecartCommandBlock.activateMinecart(MinecartCommandBlock.java:85)");
            frames.push("net.minecraft.world.entity.vehicle.minecart.OldMinecartBehavior.tick(OldMinecartBehavior.java:76)");
            frames.push("net.minecraft.world.entity.vehicle.minecart.AbstractMinecart.tick(AbstractMinecart.java:280)");
            frames.extend(ENTITY_TICK);
            frames.push(TICK_CHILDREN_LEVELS);
            frames.extend(TICK_LOOP);
        }
        CommandOrigin::ScheduledFunction => {
            frames.push(FUNCTION_EXECUTE);
            frames.push("net.minecraft.world.level.timers.FunctionCallback.lambda$handle$0(FunctionCallback.java:17)");
            frames.push("java.base/java.util.Optional.ifPresent(Optional.java:178)");
            frames.push("net.minecraft.world.level.timers.FunctionCallback.handle(FunctionCallback.java:17)");
            frames.push("net.minecraft.world.level.timers.FunctionCallback.handle(FunctionCallback.java:9)");
            frames.extend(TIMER_TICK);
            frames.push(TICK_CHILDREN_LEVELS);
            frames.extend(TICK_LOOP);
        }
        CommandOrigin::ScheduledFunctionTag => {
            frames.push(FUNCTION_EXECUTE);
            frames.push("net.minecraft.world.level.timers.FunctionTagCallback.handle(FunctionTagCallback.java:23)");
            frames.push("net.minecraft.world.level.timers.FunctionTagCallback.handle(FunctionTagCallback.java:13)");
            frames.extend(TIMER_TICK);
            frames.push(TICK_CHILDREN_LEVELS);
            frames.extend(TICK_LOOP);
        }
        CommandOrigin::TickFunctions | CommandOrigin::LoadFunctions => {
            frames.push(FUNCTION_EXECUTE);
            frames.push("net.minecraft.server.ServerFunctionManager.executeTagFunctions(ServerFunctionManager.java:61)");
            frames.push(if origin == CommandOrigin::TickFunctions {
                "net.minecraft.server.ServerFunctionManager.tick(ServerFunctionManager.java:55)"
            } else {
                "net.minecraft.server.ServerFunctionManager.tick(ServerFunctionManager.java:53)"
            });
            frames.push("net.minecraft.server.MinecraftServer.tickChildren(MinecraftServer.java:1094)");
            frames.extend(TICK_LOOP);
        }
        CommandOrigin::AdvancementReward => {
            frames.push(FUNCTION_EXECUTE);
            frames.push("net.minecraft.advancements.AdvancementRewards.lambda$grant$1(AdvancementRewards.java:76)");
            frames.push("java.base/java.util.Optional.ifPresent(Optional.java:178)");
            frames.push("net.minecraft.advancements.AdvancementRewards.grant(AdvancementRewards.java:76)");
            frames.push("net.minecraft.server.PlayerAdvancements.award(PlayerAdvancements.java:178)");
            frames.push("net.minecraft.advancements.triggers.SimpleCriterionTrigger.trigger(SimpleCriterionTrigger.java:53)");
            frames.push("net.minecraft.advancements.triggers.PlayerTrigger.trigger(PlayerTrigger.java:28)");
            frames.push("net.minecraft.server.level.ServerPlayer.tick(ServerPlayer.java:694)");
            frames.extend(ENTITY_TICK);
            frames.push(TICK_CHILDREN_LEVELS);
            frames.extend(TICK_LOOP);
        }
    }
    frames
}

const RELOAD_PACKS: &str = "net.minecraft.server.commands.ReloadCommand.reloadPacks(ReloadCommand.java:22)";

fn reload_resources_frames(origin: CommandOrigin) -> Vec<&'static str> {
    if matches!(origin, CommandOrigin::PlayerChat | CommandOrigin::Rcon) {
        let mut frames = vec![
            "java.base/java.util.concurrent.CompletableFuture$UniCompose.tryFire(CompletableFuture.java:1171)",
            "java.base/java.util.concurrent.CompletableFuture.postComplete(CompletableFuture.java:531)",
            "java.base/java.util.concurrent.CompletableFuture$AsyncSupply.run(CompletableFuture.java:1794)",
        ];
        frames.extend(&TASK_QUEUE[..9]);
        frames.push("net.minecraft.server.MinecraftServer.reloadResources(MinecraftServer.java:1550)");
        frames.push(RELOAD_PACKS);
        frames
    } else {
        vec![
            "java.base/java.util.concurrent.CompletableFuture.uniComposeStage(CompletableFuture.java:1208)",
            "java.base/java.util.concurrent.CompletableFuture.thenCompose(CompletableFuture.java:2368)",
            "net.minecraft.server.MinecraftServer.reloadResources(MinecraftServer.java:1518)",
            RELOAD_PACKS,
        ]
    }
}

pub fn invoker_frames(command: &[&'static str], server: &Server) -> &'static [&'static str] {
    let mut frames = reload_resources_frames(server.command_origin);
    frames.extend(command);
    frames.extend(COMMAND_EXECUTION);
    frames.extend(origin_frames(server.command_origin));
    frames.leak()
}

pub fn reload_packs(server: &mut Server, selected_packs: &[String], invoker: &'static [&'static str], source: &CommandSourceStack) {
    if let Err(exception) = server.reload_resources(selected_packs, invoker) {
        warn!("Failed to execute reload\n{}", crate::resources::registry_load_task::completion_exception(exception).print_stack_trace().trim_end());
        source.send_failure(server, Component::translatable("commands.reload.failure"));
    }
}

fn discover_new_packs(server: &mut Server, current_packs: Vec<String>) -> Vec<String> {
    server.pack_repository.reload();
    let mut selected = current_packs;
    let disabled = &server.data_configuration.data_packs.disabled;
    for pack in server.pack_repository.available_ids() {
        if !disabled.iter().any(|d| d == pack) && !selected.iter().any(|s| s == pack) {
            selected.push(pack.to_string());
        }
    }
    selected
}

pub fn reload(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let current_packs = server.pack_repository.selected_ids().into_iter().map(str::to_string).collect();
    let new_selected_packs = discover_new_packs(server, current_packs);
    source.send_success(server, || Component::translatable("commands.reload.success"), true);
    reload_packs(server, &new_selected_packs, invoker_frames(&[RELOAD_COMMAND_FRAME], server), source);
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORIGINS: [CommandOrigin; 11] = [
        CommandOrigin::Console,
        CommandOrigin::PlayerChat,
        CommandOrigin::Rcon,
        CommandOrigin::CommandBlock,
        CommandOrigin::ChainCommandBlock,
        CommandOrigin::MinecartCommandBlock,
        CommandOrigin::ScheduledFunction,
        CommandOrigin::ScheduledFunctionTag,
        CommandOrigin::TickFunctions,
        CommandOrigin::LoadFunctions,
        CommandOrigin::AdvancementReward,
    ];

    fn contains(frames: &[&str], needle: &str) -> bool {
        frames.iter().any(|frame| frame.contains(needle))
    }

    #[test]
    fn every_origin_ends_in_the_thread_that_runs_it() {
        for origin in ORIGINS {
            let frames = origin_frames(origin);
            assert_eq!(frames.last(), Some(&"java.base/java.lang.Thread.run(Thread.java:1474)"), "{origin:?}");
            assert!(contains(&frames, "MinecraftServer.lambda$spin$0"), "{origin:?}");
        }
    }

    #[test]
    fn only_chat_and_rcon_reload_through_the_deferred_task() {
        for origin in ORIGINS {
            let frames = reload_resources_frames(origin);
            let deferred = matches!(origin, CommandOrigin::PlayerChat | CommandOrigin::Rcon);
            assert_eq!(contains(&frames, "reloadResources(MinecraftServer.java:1550)"), deferred, "{origin:?}");
            assert_eq!(contains(&frames, "reloadResources(MinecraftServer.java:1518)"), !deferred, "{origin:?}");
            assert_eq!(frames.last(), Some(&RELOAD_PACKS), "{origin:?}");
        }
    }

    #[test]
    fn each_origin_shows_the_frames_of_its_own_entry_point() {
        assert!(contains(&origin_frames(CommandOrigin::Console), "DedicatedServer.handleConsoleInputs"));
        assert!(contains(&origin_frames(CommandOrigin::PlayerChat), "ServerGamePacketListenerImpl.performUnsignedChatCommand"));
        assert!(contains(&origin_frames(CommandOrigin::Rcon), "DedicatedServer.lambda$runCommand$0"));
        assert!(contains(&origin_frames(CommandOrigin::CommandBlock), "CommandBlock.execute(CommandBlock.java:116)"));
        assert!(contains(&origin_frames(CommandOrigin::ChainCommandBlock), "CommandBlock.executeChain"));
        assert!(contains(&origin_frames(CommandOrigin::MinecartCommandBlock), "MinecartCommandBlock.activateMinecart"));
        assert!(contains(&origin_frames(CommandOrigin::ScheduledFunction), "FunctionCallback.handle"));
        assert!(contains(&origin_frames(CommandOrigin::ScheduledFunctionTag), "FunctionTagCallback.handle"));
        assert!(contains(&origin_frames(CommandOrigin::TickFunctions), "ServerFunctionManager.tick(ServerFunctionManager.java:55)"));
        assert!(contains(&origin_frames(CommandOrigin::LoadFunctions), "ServerFunctionManager.tick(ServerFunctionManager.java:53)"));
        assert!(contains(&origin_frames(CommandOrigin::AdvancementReward), "AdvancementRewards.grant"));
    }
}
