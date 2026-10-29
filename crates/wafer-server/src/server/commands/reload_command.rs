use crate::commands::brigadier::{CommandContext, CommandSyntaxException};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::Component;
use crate::server::Server;
use crate::server::command_frames::{COMMAND_EXECUTION, TASK_QUEUE, origin_frames};
use crate::advancements::triggers::criterion_trigger::CriterionTrigger;
use crate::server::command_origin::CommandOrigin;

pub const RELOAD_COMMAND_FRAME: &str = "net.minecraft.server.commands.ReloadCommand.lambda$register$0(ReloadCommand.java:53)";
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

pub fn invoker_frames(command: &[&'static str], server: &Server) -> Vec<&'static str> {
    let mut frames = reload_resources_frames(server.command_origin);
    frames.extend(command);
    frames.extend(COMMAND_EXECUTION);
    frames.extend(origin_frames(server.command_origin));
    frames
}

pub fn reload_packs(server: &mut Server, selected_packs: &[String], invoker: &[&'static str], source: &CommandSourceStack) {
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
    reload_packs(server, &new_selected_packs, &invoker_frames(&[RELOAD_COMMAND_FRAME], server), source);
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORIGINS: [CommandOrigin; 13] = [
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
        CommandOrigin::AdvancementReward(CriterionTrigger::Tick),
        CommandOrigin::AdvancementReward(CriterionTrigger::Location),
        CommandOrigin::EnchantmentTickPlayer,
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
        assert!(contains(&origin_frames(CommandOrigin::AdvancementReward(CriterionTrigger::Tick)), "AdvancementRewards.grant"));
        assert!(contains(&origin_frames(CommandOrigin::AdvancementReward(CriterionTrigger::Location)), "ServerPlayer.doTick(ServerPlayer.java:810)"));
        assert!(contains(&origin_frames(CommandOrigin::AdvancementReward(CriterionTrigger::Levitation)), "LevitationTrigger.trigger"));
        assert!(contains(&origin_frames(CommandOrigin::EnchantmentTickPlayer), "RunFunction.apply"));
    }
}
