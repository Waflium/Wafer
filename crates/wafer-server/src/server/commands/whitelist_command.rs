use crate::commands::brigadier::{CommandContext, CommandSyntaxException, SimpleCommandExceptionType};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::{Arg, Component};
use crate::server::Server;
use crate::server::players::WhiteListEntry;
use crate::world::level::storage::format_uuid;

static ERROR_ALREADY_ENABLED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.whitelist.alreadyOn");
static ERROR_ALREADY_DISABLED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.whitelist.alreadyOff");
static ERROR_ALREADY_WHITELISTED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.whitelist.add.failed");
static ERROR_NOT_WHITELISTED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.whitelist.remove.failed");

const RELOAD_TAIL: [&str; 22] = [
    "net.minecraft.server.dedicated.DedicatedPlayerList.loadWhiteList(DedicatedPlayerList.java:89)",
    "net.minecraft.server.dedicated.DedicatedPlayerList.reloadWhiteList(DedicatedPlayerList.java:36)",
    "net.minecraft.server.commands.WhitelistCommand.reload(WhitelistCommand.java:72)",
    "net.minecraft.server.commands.WhitelistCommand.lambda$register$8(WhitelistCommand.java:66)",
    "com.mojang.brigadier.context.ContextChain.runExecutable(ContextChain.java:73)",
    "net.minecraft.commands.execution.tasks.ExecuteCommand.execute(ExecuteCommand.java:29)",
    "net.minecraft.commands.execution.tasks.ExecuteCommand.execute(ExecuteCommand.java:13)",
    "net.minecraft.commands.execution.UnboundEntryAction.lambda$bind$0(UnboundEntryAction.java:8)",
    "net.minecraft.commands.execution.CommandQueueEntry.execute(CommandQueueEntry.java:8)",
    "net.minecraft.commands.execution.ExecutionContext.runCommandQueue(ExecutionContext.java:106)",
    "net.minecraft.commands.Commands.executeCommandInContext(Commands.java:438)",
    "net.minecraft.commands.Commands.performCommand(Commands.java:370)",
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

pub fn reload(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    server.whitelist.reload_logged("white-list", &RELOAD_TAIL);
    source.send_success(server, || Component::translatable("commands.whitelist.reloaded"), true);
    server.kick_unlisted_players();
    Ok(1)
}

pub fn add_players(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let targets = context.get_game_profiles(server, source, "targets")?;
    let mut success = 0;
    for target in targets {
        if !server.whitelist.contains_key(&format_uuid(target.id)) {
            server.whitelist.add(WhiteListEntry { user: target.clone() });
            source.send_success(server, || Component::translatable_with("commands.whitelist.add.success", vec![Arg::Component(Component::literal(target.name.as_str()))]), true);
            success += 1;
        }
    }
    if success == 0 {
        return Err(ERROR_ALREADY_WHITELISTED.create());
    }
    Ok(success)
}

pub fn remove_players(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let targets = context.get_game_profiles(server, source, "targets")?;
    let mut success = 0;
    for target in targets {
        if server.whitelist.contains_key(&format_uuid(target.id)) {
            server.whitelist.remove(&target);
            source.send_success(
                server,
                || Component::translatable_with("commands.whitelist.remove.success", vec![Arg::Component(Component::literal(target.name.as_str()))]),
                true,
            );
            success += 1;
        }
    }
    if success == 0 {
        return Err(ERROR_NOT_WHITELISTED.create());
    }
    server.kick_unlisted_players();
    Ok(success)
}

pub fn enable(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    if server.config.white_list {
        return Err(ERROR_ALREADY_ENABLED.create());
    }
    server.set_using_whitelist(true);
    source.send_success(server, || Component::translatable("commands.whitelist.enabled"), true);
    server.kick_unlisted_players();
    Ok(1)
}

pub fn disable(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    if !server.config.white_list {
        return Err(ERROR_ALREADY_DISABLED.create());
    }
    server.set_using_whitelist(false);
    source.send_success(server, || Component::translatable("commands.whitelist.disabled"), true);
    Ok(1)
}

pub fn show_list(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let names: Vec<String> = server.whitelist.entries().into_iter().map(|e| e.user.name.clone()).collect();
    if names.is_empty() {
        source.send_success(server, || Component::translatable("commands.whitelist.none"), false);
    } else {
        source.send_success(server, || Component::translatable_with("commands.whitelist.list", vec![Arg::Int(names.len() as i32), names.join(", ").into()]), false);
    }
    Ok(names.len() as i32)
}

pub fn suggest_unlisted_players(
    server: &Server,
    _context: &CommandContext,
    _source: &CommandSourceStack,
    mut builder: crate::commands::suggestion::SuggestionsBuilder,
) -> Result<crate::commands::suggestion::Suggestions, CommandSyntaxException> {
    let names = server.players().filter(|p| !server.whitelist.contains_key(&format_uuid(p.profile.uuid))).map(|p| p.profile.name.as_str());
    crate::commands::shared_suggestion_provider::suggest(names, &mut builder);
    Ok(builder.build())
}

pub fn suggest_listed_players(
    server: &Server,
    _context: &CommandContext,
    _source: &CommandSourceStack,
    mut builder: crate::commands::suggestion::SuggestionsBuilder,
) -> Result<crate::commands::suggestion::Suggestions, CommandSyntaxException> {
    crate::commands::shared_suggestion_provider::suggest(server.whitelist.entries().into_iter().map(|e| e.user.name.as_str()), &mut builder);
    Ok(builder.build())
}
