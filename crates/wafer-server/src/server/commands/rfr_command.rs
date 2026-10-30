use crate::commands::brigadier::{CommandContext, CommandSyntaxException, DynamicCommandExceptionType};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::{Arg, ClickEvent, Component, HoverEvent};
use crate::server::Server;
use crate::util::profiling::rfr::environment::Environment;
use crate::util::profiling::rfr::rust_profiler;

static DUMP_FAILED: DynamicCommandExceptionType = DynamicCommandExceptionType(|message| Component::translatable_with_fallback("commands.rfr.dump.failed", "Failed to dump RFR recording: %s", vec![message]));

fn start_failed() -> CommandSyntaxException {
    CommandSyntaxException::new(Component::translatable_with_fallback("commands.rfr.start.failed", "Failed to start RFR profiling", Vec::new()))
}

pub fn start_rfr(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let env = Environment::from_dedicated_server();
    if !rust_profiler::instance().start(env) {
        return Err(start_failed());
    }
    source.send_success(server, || Component::translatable_with_fallback("commands.rfr.started", "RFR profiling started", Vec::new()), false);
    Ok(1)
}

pub fn stop_rfr(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let saved_recording = rust_profiler::instance().stop().map_err(|message| DUMP_FAILED.create(Arg::String(message)))?;
    let clipboard_path = saved_recording.display().to_string();
    let file_text = Component::literal(saved_recording.display().to_string())
        .underlined()
        .with_click(ClickEvent::CopyToClipboard(clipboard_path))
        .with_hover(HoverEvent::ShowText(Box::new(Component::translatable("chat.copy.click"))));
    source.send_success(server, || Component::translatable_with_fallback("commands.rfr.stopped", "RFR profiling stopped and dumped to %s", vec![Arg::Component(file_text)]), false);
    Ok(1)
}
