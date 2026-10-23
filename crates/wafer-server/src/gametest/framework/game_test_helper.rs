use wafer_core::BlockPos;
use wafer_data::BlockState;

use super::game_test_exception::GameTestException;
use super::game_test_info::{self, InfoId};
use crate::network::chat::{Arg, Component};
use crate::server::Server;
use crate::world::level::BlockGetter;
use crate::world::level::block::entity::BlockEntityKind;
use crate::world::level::block::mirror::Mirror;
use crate::world::level::block::rotation::Rotation;
use crate::world::level::levelgen::structure::templatesystem::structure_template;

pub fn assertion_exception(server: &mut Server, id: InfoId, description: Component) -> GameTestException {
    GameTestException::Assert { message: description, tick: server.game_test_ticker.infos[id].get_tick() }
}

pub fn assertion_exception_at(server: &mut Server, id: InfoId, pos: BlockPos, description: Component) -> GameTestException {
    let absolute_pos = absolute_pos(server, id, pos);
    GameTestException::AssertPos { message: description, absolute_pos, relative_pos: pos, tick: server.game_test_ticker.infos[id].get_tick() }
}

pub fn get_block_state(server: &mut Server, id: InfoId, pos: BlockPos) -> BlockState {
    let absolute_pos = absolute_pos(server, id, pos);
    let level = server.game_test_ticker.infos[id].level;
    server.levels[level].block_state(absolute_pos)
}

pub fn get_test_block_entity(server: &mut Server, id: InfoId, pos: BlockPos) -> Result<BlockPos, GameTestException> {
    let absolute_pos = absolute_pos(server, id, pos);
    let level = server.game_test_ticker.infos[id].level;
    match server.levels[level].get_block_entity(absolute_pos).map(|block_entity| (matches!(block_entity.kind, BlockEntityKind::TestBlock(_)), block_entity.ty())) {
        None => Err(assertion_exception_at(server, id, pos, Component::translatable("test.error.missing_block_entity"))),
        Some((true, _)) => Ok(absolute_pos),
        Some((false, ty)) => {
            Err(assertion_exception_at(server, id, pos, Component::translatable_with("test.error.wrong_block_entity", vec![Arg::String(ty.name().to_string())])))
        }
    }
}

pub fn absolute_pos(server: &mut Server, id: InfoId, relative_pos: BlockPos) -> BlockPos {
    let test_pos = game_test_info::get_test_origin(server, id);
    let absolute_pos_before_transform = test_pos.offset(relative_pos.x, relative_pos.y, relative_pos.z);
    let rotation = server.game_test_ticker.infos[id].get_rotation();
    structure_template::transform(absolute_pos_before_transform, Mirror::None, rotation, test_pos)
}

pub fn succeed(server: &mut Server, id: InfoId) {
    game_test_info::succeed(server, id);
}

pub fn fail(server: &mut Server, id: InfoId, message: Component) -> GameTestException {
    assertion_exception(server, id, message)
}

pub fn for_every_block_in_structure(server: &mut Server, id: InfoId) -> Vec<BlockPos> {
    let level = server.game_test_ticker.infos[id].level;
    let pos = game_test_info::test_block_pos(server, id);
    let absolute = server.levels[level].test_instance_block_entity_get_structure_bounds(pos);
    let rotation = server.game_test_ticker.infos[id].get_rotation();
    let (x_size, y_size, z_size) = match rotation {
        Rotation::Counterclockwise90 | Rotation::Clockwise90 => (absolute.z_size(), absolute.y_size(), absolute.x_size()),
        _ => (absolute.x_size(), absolute.y_size(), absolute.z_size()),
    };
    let max = BlockPos::new((x_size - 1.0).floor() as i32, (y_size - 1.0).floor() as i32, (z_size - 1.0).floor() as i32);
    BlockPos::between_closed(BlockPos::ZERO, max).collect()
}

pub fn on_each_tick(server: &mut Server, id: InfoId) {
    let info = &mut server.game_test_ticker.infos[id];
    for tick in info.get_tick() as i64..info.get_timeout_ticks() as i64 {
        info.set_run_at_tick_time(tick, game_test_info::TickAction::BlockBasedEachTick);
    }
}
