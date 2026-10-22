use wafer_data::{BlockState, Tag, registry};

use crate::world::entity::EntityId;

pub const DEFAULT_NOTIFICATION_RADIUS: i32 = 16;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct GameEvent(pub u8);

pub const BLOCK_ACTIVATE: GameEvent = GameEvent(0);
pub const BLOCK_ATTACH: GameEvent = GameEvent(1);
pub const BLOCK_CHANGE: GameEvent = GameEvent(2);
pub const BLOCK_CLOSE: GameEvent = GameEvent(3);
pub const BLOCK_DEACTIVATE: GameEvent = GameEvent(4);
pub const BLOCK_DESTROY: GameEvent = GameEvent(5);
pub const BLOCK_DETACH: GameEvent = GameEvent(6);
pub const BLOCK_OPEN: GameEvent = GameEvent(7);
pub const BLOCK_PLACE: GameEvent = GameEvent(8);
pub const BOUNCE: GameEvent = GameEvent(9);
pub const CONTAINER_CLOSE: GameEvent = GameEvent(10);
pub const CONTAINER_OPEN: GameEvent = GameEvent(11);
pub const DRINK: GameEvent = GameEvent(12);
pub const EAT: GameEvent = GameEvent(13);
pub const ELYTRA_GLIDE: GameEvent = GameEvent(14);
pub const ENTITY_DAMAGE: GameEvent = GameEvent(15);
pub const ENTITY_DIE: GameEvent = GameEvent(16);
pub const ENTITY_DISMOUNT: GameEvent = GameEvent(17);
pub const ENTITY_INTERACT: GameEvent = GameEvent(18);
pub const ENTITY_MOUNT: GameEvent = GameEvent(19);
pub const ENTITY_PLACE: GameEvent = GameEvent(20);
pub const ENTITY_ACTION: GameEvent = GameEvent(21);
pub const EQUIP: GameEvent = GameEvent(22);
pub const EXPLODE: GameEvent = GameEvent(23);
pub const FLAP: GameEvent = GameEvent(24);
pub const FLUID_PICKUP: GameEvent = GameEvent(25);
pub const FLUID_PLACE: GameEvent = GameEvent(26);
pub const HIT_GROUND: GameEvent = GameEvent(27);
pub const INSTRUMENT_PLAY: GameEvent = GameEvent(28);
pub const ITEM_INTERACT_FINISH: GameEvent = GameEvent(29);
pub const ITEM_INTERACT_START: GameEvent = GameEvent(30);
pub const JUKEBOX_PLAY: GameEvent = GameEvent(31);
pub const JUKEBOX_STOP_PLAY: GameEvent = GameEvent(32);
pub const LIGHTNING_STRIKE: GameEvent = GameEvent(33);
pub const NOTE_BLOCK_PLAY: GameEvent = GameEvent(34);
pub const PRIME_FUSE: GameEvent = GameEvent(35);
pub const PROJECTILE_LAND: GameEvent = GameEvent(36);
pub const PROJECTILE_SHOOT: GameEvent = GameEvent(37);
pub const SCULK_SENSOR_TENDRILS_CLICKING: GameEvent = GameEvent(38);
pub const SHEAR: GameEvent = GameEvent(39);
pub const SHRIEK: GameEvent = GameEvent(40);
pub const SPLASH: GameEvent = GameEvent(41);
pub const STEP: GameEvent = GameEvent(42);
pub const SWIM: GameEvent = GameEvent(43);
pub const TELEPORT: GameEvent = GameEvent(44);
pub const UNEQUIP: GameEvent = GameEvent(45);
pub const RESONATE_1: GameEvent = GameEvent(46);

impl GameEvent {
    pub fn notification_radius(self) -> i32 {
        match self {
            JUKEBOX_PLAY | JUKEBOX_STOP_PLAY => 10,
            SHRIEK => 32,
            _ => DEFAULT_NOTIFICATION_RADIUS,
        }
    }

    pub fn is(self, tag: &Tag) -> bool {
        tag.contains(self.0 as usize)
    }

    pub fn name(self) -> &'static str {
        registry::GAME_EVENT[self.0 as usize]
    }

    pub fn by_name(name: &str) -> Option<GameEvent> {
        registry::GAME_EVENT.iter().position(|&entry| entry == name).map(|id| GameEvent(id as u8))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Context {
    pub source_entity: Option<EntityId>,
    pub affected_state: Option<BlockState>,
}

impl Context {
    pub fn of(source_entity: Option<EntityId>) -> Context {
        Context { source_entity, affected_state: None }
    }

    pub fn of_state(affected_state: Option<BlockState>) -> Context {
        Context { source_entity: None, affected_state }
    }

    pub fn of_entity_and_state(source_entity: Option<EntityId>, affected_state: Option<BlockState>) -> Context {
        Context { source_entity, affected_state }
    }
}
