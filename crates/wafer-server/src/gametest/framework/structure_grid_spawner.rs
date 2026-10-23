use wafer_core::{AABB, BlockPos};

use super::game_test_info::{self, InfoId};
use super::structure_utils;
use crate::server::Server;

const SPACE_BETWEEN_COLUMNS: i32 = 5;
const SPACE_BETWEEN_ROWS: i32 = 6;

struct DimensionGridState {
    dimension: String,
    first_test_north_west_corner: BlockPos,
    next_corner: BlockPos,
    row_bounds: AABB,
    current_row_count: i32,
    tests_in_last_batch: Vec<InfoId>,
}

pub struct StructureGridSpawner {
    tests_per_row: i32,
    first_test_north_west_corner: Box<dyn Fn(&mut Server, &str) -> BlockPos + Send>,
    clear_on_batch: bool,
    grids: Vec<DimensionGridState>,
}

pub fn new(server: &mut Server, first_test_north_west_corner: Box<dyn Fn(&mut Server, &str) -> BlockPos + Send>, tests_per_row: i32, clear_on_batch: bool) -> usize {
    server.game_test_ticker.grid_spawners.push(StructureGridSpawner { tests_per_row, first_test_north_west_corner, clear_on_batch, grids: Vec::new() });
    server.game_test_ticker.grid_spawners.len() - 1
}

fn grid_for(server: &mut Server, spawner: usize, info: InfoId) -> usize {
    let dimension = server.game_test_ticker.infos[info].test.1.dimension().to_string();
    if let Some(index) = server.game_test_ticker.grid_spawners[spawner].grids.iter().position(|grid| grid.dimension == dimension) {
        return index;
    }
    let first = std::mem::replace(&mut server.game_test_ticker.grid_spawners[spawner].first_test_north_west_corner, Box::new(|_, _| BlockPos::ZERO));
    let start = first(server, &dimension);
    server.game_test_ticker.grid_spawners[spawner].first_test_north_west_corner = first;
    let grids = &mut server.game_test_ticker.grid_spawners[spawner].grids;
    grids.push(DimensionGridState {
        dimension,
        first_test_north_west_corner: start,
        next_corner: start,
        row_bounds: AABB::of_block(start),
        current_row_count: 0,
        tests_in_last_batch: Vec::new(),
    });
    grids.len() - 1
}

pub fn on_batch_start(server: &mut Server, spawner: usize) {
    if !server.game_test_ticker.grid_spawners[spawner].clear_on_batch {
        return;
    }
    for grid in 0..server.game_test_ticker.grid_spawners[spawner].grids.len() {
        for info in server.game_test_ticker.grid_spawners[spawner].grids[grid].tests_in_last_batch.clone() {
            let level = server.game_test_ticker.infos[info].level;
            let pos = game_test_info::test_block_pos(server, info);
            let bounding_box = server.levels[level].test_instance_block_entity_get_test_bounding_box(pos);
            structure_utils::clear_space_for_structure(&bounding_box, &mut server.levels[level]);
        }
        let grid = &mut server.game_test_ticker.grid_spawners[spawner].grids[grid];
        grid.tests_in_last_batch.clear();
        grid.next_corner = grid.first_test_north_west_corner;
        grid.row_bounds = AABB::of_block(grid.first_test_north_west_corner);
        grid.current_row_count = 0;
    }
}

pub fn spawn_structure(server: &mut Server, spawner: usize, info: InfoId) -> bool {
    let grid = grid_for(server, spawner, info);
    let north_west_corner = server.game_test_ticker.grid_spawners[spawner].grids[grid].next_corner;
    server.game_test_ticker.infos[info].set_test_block_pos(Some(north_west_corner));
    if !game_test_info::prepare_test_structure(server, info) {
        return false;
    }
    server.game_test_ticker.infos[info].start_execution(1);
    let level = server.game_test_ticker.infos[info].level;
    let structure_bounds = server.levels[level].test_instance_block_entity_get_test_bounds(north_west_corner);
    let tests_per_row = server.game_test_ticker.grid_spawners[spawner].tests_per_row;
    let grid = &mut server.game_test_ticker.grid_spawners[spawner].grids[grid];
    grid.row_bounds = grid.row_bounds.minmax(&structure_bounds);
    grid.next_corner = grid.next_corner.offset(structure_bounds.x_size() as i32 + SPACE_BETWEEN_COLUMNS, 0, 0);
    grid.current_row_count += 1;
    if grid.current_row_count >= tests_per_row {
        grid.current_row_count = 0;
        grid.next_corner = grid.next_corner.offset(0, 0, grid.row_bounds.z_size() as i32 + SPACE_BETWEEN_ROWS);
        grid.next_corner.x = grid.first_test_north_west_corner.x;
        grid.row_bounds = AABB::of_block(grid.next_corner);
    }
    grid.tests_in_last_batch.push(info);
    true
}
