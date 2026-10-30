use crate::commands::brigadier::{CommandContext, CommandSyntaxException, SimpleCommandExceptionType};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::Component;
use crate::server::Server;
use crate::server::command_frames::command_frames;
use crate::server::main_thread_frames;

const SAVE_ALL_SAVE_EVERYTHING: &str = "net.minecraft.server.commands.SaveAllCommand.saveAll(SaveAllCommand.java:32)";
const REGISTER: &str = "net.minecraft.server.commands.SaveAllCommand.lambda$register$0(SaveAllCommand.java:20)";
const REGISTER_FLUSH: &str = "net.minecraft.server.commands.SaveAllCommand.lambda$register$1(SaveAllCommand.java:23)";

static ERROR_FAILED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.save.failed");
static ERROR_ALREADY_ON: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.save.alreadyOn");
static ERROR_ALREADY_OFF: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.save.alreadyOff");

fn save(server: &mut Server, source: &CommandSourceStack, flush: bool) -> Result<i32, CommandSyntaxException> {
    source.send_success(server, || Component::translatable("commands.save.saving"), false);
    let mut frames = vec![SAVE_ALL_SAVE_EVERYTHING, if flush { REGISTER_FLUSH } else { REGISTER }];
    frames.extend(command_frames(server.command_origin));
    let saved = {
        let _scope = main_thread_frames::enter_complete(frames);
        server.save_everything(true, flush, true)
    };
    if !saved {
        return Err(ERROR_FAILED.create());
    }
    source.send_success(server, || Component::translatable("commands.save.success"), true);
    Ok(1)
}

pub fn save_all(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    save(server, source, false)
}

pub fn save_all_flush(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    save(server, source, true)
}

pub fn save_on(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    if !server.set_auto_save(true) {
        return Err(ERROR_ALREADY_ON.create());
    }
    source.send_success(server, || Component::translatable("commands.save.enabled"), true);
    Ok(1)
}

pub fn save_off(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    if !server.set_auto_save(false) {
        return Err(ERROR_ALREADY_OFF.create());
    }
    source.send_success(server, || Component::translatable("commands.save.disabled"), true);
    Ok(1)
}
