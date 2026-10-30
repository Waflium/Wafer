use crate::commands::brigadier::{CommandContext, CommandSyntaxException, DynamicCommandExceptionType, SimpleCommandExceptionType};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::{Arg, Component};
use crate::server::Server;
use crate::util::profiling::jfr::jfr_profiler;

static START_FAILED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.jfr.start.failed");
static DUMP_FAILED: DynamicCommandExceptionType = DynamicCommandExceptionType(|message| Component::translatable_with("commands.jfr.dump.failed", vec![message]));

pub fn start_jfr(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    if !jfr_profiler::INSTANCE.start() {
        return Err(START_FAILED.create());
    }
    source.send_success(server, || Component::translatable("commands.jfr.started"), false);
    Ok(1)
}

pub fn stop_jfr(_server: &mut Server, _context: &CommandContext, _source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let Err(message) = jfr_profiler::INSTANCE.stop();
    Err(DUMP_FAILED.create(Arg::String(message)))
}
