use super::Server;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum CommandOrigin {
    #[default]
    Console,
    PlayerChat,
    Rcon,
    CommandBlock,
    ChainCommandBlock,
    MinecartCommandBlock,
    ScheduledFunction,
    ScheduledFunctionTag,
    TickFunctions,
    LoadFunctions,
    AdvancementReward,
    EnchantmentEffect,
    TestEnvironmentFunction,
}

impl Server {
    pub fn run_from<R>(&mut self, origin: CommandOrigin, run: impl FnOnce(&mut Server) -> R) -> R {
        let previous = std::mem::replace(&mut self.command_origin, origin);
        let base = matches!(origin, CommandOrigin::PlayerChat | CommandOrigin::Rcon | CommandOrigin::AdvancementReward | CommandOrigin::EnchantmentEffect | CommandOrigin::TestEnvironmentFunction)
            .then(super::main_thread_frames::current);
        let previous_base = super::command_frames::replace_origin_base(base);
        let result = run(self);
        super::command_frames::replace_origin_base(previous_base);
        self.command_origin = previous;
        result
    }
}
