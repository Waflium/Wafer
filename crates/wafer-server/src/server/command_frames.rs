use super::command_origin::CommandOrigin;

pub const COMMAND_EXECUTION: [&str; 7] = [
    "com.mojang.brigadier.context.ContextChain.runExecutable(ContextChain.java:73)",
    "net.minecraft.commands.execution.tasks.ExecuteCommand.execute(ExecuteCommand.java:29)",
    "net.minecraft.commands.execution.tasks.ExecuteCommand.execute(ExecuteCommand.java:13)",
    "net.minecraft.commands.execution.UnboundEntryAction.lambda$bind$0(UnboundEntryAction.java:8)",
    "net.minecraft.commands.execution.CommandQueueEntry.execute(CommandQueueEntry.java:8)",
    "net.minecraft.commands.execution.ExecutionContext.runCommandQueue(ExecutionContext.java:106)",
    "net.minecraft.commands.Commands.executeCommandInContext(Commands.java:438)",
];

pub const PERFORM_COMMAND: [&str; 2] = [
    "net.minecraft.commands.Commands.performCommand(Commands.java:370)",
    "net.minecraft.commands.Commands.performPrefixedCommand(Commands.java:356)",
];

pub const FUNCTION_EXECUTE: &str = "net.minecraft.server.ServerFunctionManager.execute(ServerFunctionManager.java:71)";

pub const TICK_LOOP: [&str; 6] = [
    "net.minecraft.server.MinecraftServer.tickServer(MinecraftServer.java:980)",
    "net.minecraft.server.dedicated.DedicatedServer.tickServer(DedicatedServer.java:278)",
    "net.minecraft.server.MinecraftServer.processPacketsAndTick(MinecraftServer.java:1013)",
    "net.minecraft.server.MinecraftServer.runServer(MinecraftServer.java:752)",
    "net.minecraft.server.MinecraftServer.lambda$spin$0(MinecraftServer.java:310)",
    "java.base/java.lang.Thread.run(Thread.java:1474)",
];

pub const TICK_CHILDREN_LEVELS: &str = "net.minecraft.server.MinecraftServer.tickChildren(MinecraftServer.java:1116)";

pub const CONSOLE_SOURCE: [&str; 3] = [
    "net.minecraft.server.dedicated.DedicatedServer.handleConsoleInputs(DedicatedServer.java:412)",
    "net.minecraft.server.dedicated.DedicatedServer.tickConnection(DedicatedServer.java:402)",
    "net.minecraft.server.MinecraftServer.tickChildren(MinecraftServer.java:1127)",
];

pub const BLOCK_TICK: [&str; 6] = [
    "net.minecraft.world.level.block.CommandBlock.tick(CommandBlock.java:104)",
    "net.minecraft.world.level.block.state.BlockBehaviour$BlockStateBase.tick(BlockBehaviour.java:1153)",
    "net.minecraft.server.level.ServerLevel.tickBlock(ServerLevel.java:851)",
    "net.minecraft.world.ticks.LevelTicks.runCollectedTicks(LevelTicks.java:213)",
    "net.minecraft.world.ticks.LevelTicks.tick(LevelTicks.java:96)",
    "net.minecraft.server.level.ServerLevel.tick(ServerLevel.java:392)",
];

pub const BASE_COMMAND_BLOCK: &str = "net.minecraft.world.level.BaseCommandBlock.performCommand(BaseCommandBlock.java:114)";

pub const ENTITY_TICK: [&str; 5] = [
    "net.minecraft.server.level.ServerLevel.tickNonPassenger(ServerLevel.java:860)",
    "net.minecraft.world.level.Level.guardEntityTick(Level.java:484)",
    "net.minecraft.server.level.ServerLevel.lambda$tick$0(ServerLevel.java:457)",
    "net.minecraft.world.level.entity.EntityTickList.forEach(EntityTickList.java:53)",
    "net.minecraft.server.level.ServerLevel.tick(ServerLevel.java:427)",
];

pub const TICK_NON_PASSENGER: [&str; 6] = [
    "net.minecraft.server.level.ServerLevel.tickNonPassenger(ServerLevel.java:860)",
    "net.minecraft.world.level.Level.guardEntityTick(Level.java:484)",
    "net.minecraft.server.level.ServerLevel.lambda$tick$0(ServerLevel.java:457)",
    "net.minecraft.world.level.entity.EntityTickList.forEach(EntityTickList.java:53)",
    "net.minecraft.server.level.ServerLevel.tick(ServerLevel.java:427)",
    TICK_CHILDREN_LEVELS,
];

pub const TICK_NON_PASSENGER_PASSENGERS: [&str; 6] = [
    "net.minecraft.server.level.ServerLevel.tickNonPassenger(ServerLevel.java:864)",
    "net.minecraft.world.level.Level.guardEntityTick(Level.java:484)",
    "net.minecraft.server.level.ServerLevel.lambda$tick$0(ServerLevel.java:457)",
    "net.minecraft.world.level.entity.EntityTickList.forEach(EntityTickList.java:53)",
    "net.minecraft.server.level.ServerLevel.tick(ServerLevel.java:427)",
    TICK_CHILDREN_LEVELS,
];

pub const TIMER_TICK: [&str; 3] = [
    "net.minecraft.world.level.timers.TimerQueue.tick(TimerQueue.java:102)",
    "net.minecraft.server.level.ServerLevel.tickTime(ServerLevel.java:495)",
    "net.minecraft.server.level.ServerLevel.tick(ServerLevel.java:385)",
];

pub const TASK_QUEUE: [&str; 11] = [
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

pub const SPIN: [&str; 2] = ["net.minecraft.server.MinecraftServer.lambda$spin$0(MinecraftServer.java:310)", "java.base/java.lang.Thread.run(Thread.java:1474)"];

pub const PLAYER_CONNECTION_TICK: [&str; 5] = [
    "net.minecraft.server.network.ServerGamePacketListenerImpl.tickPlayer(ServerGamePacketListenerImpl.java:345)",
    "net.minecraft.server.network.ServerGamePacketListenerImpl.tick(ServerGamePacketListenerImpl.java:320)",
    "net.minecraft.network.Connection.tick(Connection.java:368)",
    "net.minecraft.server.network.ServerConnectionListener.tick(ServerConnectionListener.java:175)",
    "net.minecraft.server.MinecraftServer.tickConnection(MinecraftServer.java:1158)",
];

pub const TICK_CHILDREN_CONNECTIONS: &str = "net.minecraft.server.dedicated.DedicatedServer.tickConnection(DedicatedServer.java:401)";

pub const PREPARE_LEVELS: [&str; 4] = [
    "net.minecraft.server.MinecraftServer.prepareLevels(MinecraftServer.java:578)",
    "net.minecraft.server.MinecraftServer.loadLevel(MinecraftServer.java:411)",
    "net.minecraft.server.dedicated.DedicatedServer.initServer(DedicatedServer.java:221)",
    "net.minecraft.server.MinecraftServer.runServer(MinecraftServer.java:720)",
];

thread_local! {
    static ORIGIN_BASE: std::cell::RefCell<Option<Vec<&'static str>>> = const { std::cell::RefCell::new(None) };
}

pub fn replace_origin_base(base: Option<Vec<&'static str>>) -> Option<Vec<&'static str>> {
    ORIGIN_BASE.with(|origin_base| origin_base.replace(base))
}

fn origin_base() -> Vec<&'static str> {
    ORIGIN_BASE.with(|origin_base| origin_base.borrow().clone()).unwrap_or_else(super::main_thread_frames::current)
}

pub fn enter_command(origin: CommandOrigin) -> super::main_thread_frames::FrameScope {
    static CACHED: [std::sync::OnceLock<&'static [&'static str]>; 13] = [const { std::sync::OnceLock::new() }; 13];
    match origin {
        CommandOrigin::EnchantmentEffect | CommandOrigin::TestEnvironmentFunction | CommandOrigin::AdvancementReward => super::main_thread_frames::enter_absolute(command_frames(origin)),
        _ => super::main_thread_frames::enter_absolute_static(CACHED[origin as usize].get_or_init(|| command_frames(origin).leak())),
    }
}

pub fn command_frames(origin: CommandOrigin) -> Vec<&'static str> {
    let mut frames = COMMAND_EXECUTION.to_vec();
    frames.extend(origin_frames(origin));
    frames
}

pub fn origin_frames(origin: CommandOrigin) -> Vec<&'static str> {
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
        CommandOrigin::EnchantmentEffect => {
            frames.push(FUNCTION_EXECUTE);
            frames.push("net.minecraft.world.item.enchantment.effects.RunFunction.apply(RunFunction.java:42)");
            frames.extend(origin_base());
        }
        CommandOrigin::TestEnvironmentFunction => {
            frames.push(FUNCTION_EXECUTE);
            frames.extend(origin_base());
        }
        CommandOrigin::AdvancementReward => {
            frames.push(FUNCTION_EXECUTE);
            frames.push("net.minecraft.advancements.AdvancementRewards.lambda$grant$1(AdvancementRewards.java:76)");
            frames.push("java.base/java.util.Optional.ifPresent(Optional.java:178)");
            frames.push("net.minecraft.advancements.AdvancementRewards.grant(AdvancementRewards.java:76)");
            frames.push("net.minecraft.server.PlayerAdvancements.award(PlayerAdvancements.java:178)");
            frames.push("net.minecraft.advancements.triggers.SimpleCriterionTrigger.trigger(SimpleCriterionTrigger.java:53)");
            frames.extend(origin_base());
        }
    }
    frames
}

