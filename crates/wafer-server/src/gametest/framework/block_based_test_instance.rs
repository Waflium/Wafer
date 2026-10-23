use wafer_core::BlockPos;
use wafer_data::blocks;
use wafer_data::properties::{TEST_BLOCK_MODE, TestBlockMode};

use super::game_test_exception::GameTestException;
use super::game_test_helper;
use super::game_test_info::InfoId;
use crate::network::chat::Component;
use crate::server::Server;

fn display_name(mode: TestBlockMode) -> Component {
    Component::translatable(&format!("test_block.mode.{}", mode.name()))
}

pub fn run(server: &mut Server, id: InfoId) -> Result<(), GameTestException> {
    let start_pos = find_start_block(server, id)?;
    let block_entity = game_test_helper::get_test_block_entity(server, id, start_pos)?;
    let level = server.game_test_ticker.infos[id].level;
    server.levels[level].test_block_entity_trigger(block_entity);
    game_test_helper::on_each_tick(server, id);
    Ok(())
}

pub fn each_tick(server: &mut Server, id: InfoId) -> Result<(), GameTestException> {
    let accept_blocks = find_test_blocks(server, id, TestBlockMode::Accept);
    if accept_blocks.is_empty() {
        return Err(game_test_helper::fail(server, id, Component::translatable_with("test_block.error.missing", vec![display_name(TestBlockMode::Accept).into()])));
    }
    let level = server.game_test_ticker.infos[id].level;
    let mut accept_triggered = false;
    for pos in accept_blocks {
        let block_entity = game_test_helper::get_test_block_entity(server, id, pos)?;
        if server.levels[level].test_block_entity(block_entity).is_some_and(|entity| entity.has_triggered()) {
            accept_triggered = true;
            break;
        }
    }
    if accept_triggered {
        game_test_helper::succeed(server, id);
        return Ok(());
    }
    for pos in find_test_blocks(server, id, TestBlockMode::Fail) {
        let block_entity = game_test_helper::get_test_block_entity(server, id, pos)?;
        if let Some(entity) = server.levels[level].test_block_entity(block_entity)
            && entity.has_triggered()
        {
            let message = entity.get_message().to_string();
            return Err(game_test_helper::fail(server, id, Component::literal(message)));
        }
    }
    for pos in find_test_blocks(server, id, TestBlockMode::Log) {
        let block_entity = game_test_helper::get_test_block_entity(server, id, pos)?;
        if server.levels[level].test_block_entity(block_entity).is_some_and(|entity| entity.has_triggered()) {
            server.levels[level].test_block_entity_trigger(block_entity);
            server.levels[level].test_block_entity_reset(block_entity);
        }
    }
    Ok(())
}

fn find_start_block(server: &mut Server, id: InfoId) -> Result<BlockPos, GameTestException> {
    let test_blocks = find_test_blocks(server, id, TestBlockMode::Start);
    if test_blocks.is_empty() {
        return Err(game_test_helper::fail(server, id, Component::translatable_with("test_block.error.missing", vec![display_name(TestBlockMode::Start).into()])));
    }
    if test_blocks.len() != 1 {
        return Err(game_test_helper::fail(server, id, Component::translatable_with("test_block.error.too_many", vec![display_name(TestBlockMode::Start).into()])));
    }
    Ok(test_blocks[0])
}

fn find_test_blocks(server: &mut Server, id: InfoId, mode: TestBlockMode) -> Vec<BlockPos> {
    let mut found = Vec::new();
    for pos in game_test_helper::for_every_block_in_structure(server, id) {
        let state = game_test_helper::get_block_state(server, id, pos);
        if state.is_block(blocks::TEST_BLOCK) && state.get(TEST_BLOCK_MODE) == mode {
            found.push(pos);
        }
    }
    found
}
