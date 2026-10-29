use crate::commands::brigadier::{CommandContext, CommandSyntaxException};
use crate::commands::source::{CommandSource, CommandSourceStack};
use crate::network::chat::Component;
use crate::server::Server;

const COMMAND_EXECUTION: [&str; 8] = [
    "com.mojang.brigadier.context.ContextChain.runExecutable(ContextChain.java:73)",
    "net.minecraft.commands.execution.tasks.ExecuteCommand.execute(ExecuteCommand.java:29)",
    "net.minecraft.commands.execution.tasks.ExecuteCommand.execute(ExecuteCommand.java:13)",
    "net.minecraft.commands.execution.UnboundEntryAction.lambda$bind$0(UnboundEntryAction.java:8)",
    "net.minecraft.commands.execution.CommandQueueEntry.execute(CommandQueueEntry.java:8)",
    "net.minecraft.commands.execution.ExecutionContext.runCommandQueue(ExecutionContext.java:106)",
    "net.minecraft.commands.Commands.executeCommandInContext(Commands.java:438)",
    "net.minecraft.commands.Commands.performCommand(Commands.java:370)",
];

const CONSOLE_SOURCE: [&str; 10] = [
    "net.minecraft.commands.Commands.performPrefixedCommand(Commands.java:356)",
    "net.minecraft.server.dedicated.DedicatedServer.handleConsoleInputs(DedicatedServer.java:412)",
    "net.minecraft.server.dedicated.DedicatedServer.tickConnection(DedicatedServer.java:402)",
    "net.minecraft.server.MinecraftServer.tickChildren(MinecraftServer.java:1127)",
    "net.minecraft.server.MinecraftServer.tickServer(MinecraftServer.java:980)",
    "net.minecraft.server.dedicated.DedicatedServer.tickServer(DedicatedServer.java:278)",
    "net.minecraft.server.MinecraftServer.processPacketsAndTick(MinecraftServer.java:1013)",
    "net.minecraft.server.MinecraftServer.runServer(MinecraftServer.java:752)",
    "net.minecraft.server.MinecraftServer.lambda$spin$0(MinecraftServer.java:310)",
    "java.base/java.lang.Thread.run(Thread.java:1474)",
];

const PLAYER_SOURCE: [&str; 15] = [
    "net.minecraft.server.network.ServerGamePacketListenerImpl.performUnsignedChatCommand(ServerGamePacketListenerImpl.java:1717)",
    "net.minecraft.server.network.ServerGamePacketListenerImpl.lambda$handleChatCommand$0(ServerGamePacketListenerImpl.java:1705)",
    "net.minecraft.server.TickTask.run(TickTask.java:18)",
    "net.minecraft.util.thread.BlockableEventLoop.doRunTask(BlockableEventLoop.java:173)",
    "net.minecraft.util.thread.ReentrantBlockableEventLoop.doRunTask(ReentrantBlockableEventLoop.java:23)",
    "net.minecraft.server.MinecraftServer.doRunTask(MinecraftServer.java:912)",
    "net.minecraft.server.MinecraftServer.doRunTask(MinecraftServer.java:193)",
    "net.minecraft.util.thread.BlockableEventLoop.pollTask(BlockableEventLoop.java:147)",
    "net.minecraft.server.MinecraftServer.pollTaskInternal(MinecraftServer.java:894)",
    "net.minecraft.server.MinecraftServer.pollTask(MinecraftServer.java:888)",
    "net.minecraft.util.thread.BlockableEventLoop.runAllTasks(BlockableEventLoop.java:127)",
    "net.minecraft.server.MinecraftServer.waitUntilNextTick(MinecraftServer.java:856)",
    "net.minecraft.server.MinecraftServer.runServer(MinecraftServer.java:758)",
    "net.minecraft.server.MinecraftServer.lambda$spin$0(MinecraftServer.java:310)",
    "java.base/java.lang.Thread.run(Thread.java:1474)",
];

pub const RELOAD_COMMAND_FRAME: &str = "net.minecraft.server.commands.ReloadCommand.lambda$register$0(ReloadCommand.java:53)";

pub fn invoker_frames(command: &[&'static str], source: &CommandSourceStack) -> &'static [&'static str] {
    let mut frames = command.to_vec();
    frames.extend(COMMAND_EXECUTION);
    match source.source {
        CommandSource::Player(_) => frames.extend(PLAYER_SOURCE),
        _ => frames.extend(CONSOLE_SOURCE),
    }
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
    reload_packs(server, &new_selected_packs, invoker_frames(&[RELOAD_COMMAND_FRAME], source), source);
    Ok(0)
}
