use super::Server;
use crate::advancements::triggers::criterion_trigger::CriterionTrigger;

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
    AdvancementReward(CriterionTrigger),
    EnchantmentTickPlayer,
}

impl Server {
    pub fn run_from<R>(&mut self, origin: CommandOrigin, run: impl FnOnce(&mut Server) -> R) -> R {
        let previous = std::mem::replace(&mut self.command_origin, origin);
        let result = run(self);
        self.command_origin = previous;
        result
    }
}
