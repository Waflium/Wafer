use crate::commands::arguments::selector::entity_world::EntityWorld;
use crate::commands::source::CommandSourceStack;
use crate::world::entity::EntityId;

pub const RESOLVED_COMPONENT_LIMIT: i32 = 65536;

pub struct ResolutionContext<'a> {
    pub world: &'a dyn EntityWorld,
    pub source: &'a CommandSourceStack,
    pub default_scoreboard_entity: Option<EntityId>,
    pub resolved_component_count: i32,
}

impl<'a> ResolutionContext<'a> {
    pub fn new(world: &'a dyn EntityWorld, source: &'a CommandSourceStack, default_scoreboard_entity: Option<EntityId>) -> ResolutionContext<'a> {
        ResolutionContext { world, source, default_scoreboard_entity, resolved_component_count: 0 }
    }
}
