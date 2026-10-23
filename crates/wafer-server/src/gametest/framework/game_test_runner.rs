use super::game_test_batch_factory::{self, GameTestBatch};
use super::game_test_info::{self, GameTestInfo, InfoId, Listener};
use super::report_game_listener::ReportGameListener;
use super::structure_grid_spawner;
use super::test_environment_definition::Activation;
use super::{multiple_test_tracker, structure_utils};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::{Arg, Component};
use crate::server::Server;

#[derive(Clone, Copy, PartialEq)]
pub enum StructureSpawner {
    InPlace,
    NotSet,
    Grid(usize),
}

pub struct GameTestRunner {
    all_test_infos: Vec<InfoId>,
    batches: Vec<GameTestBatch>,
    batch_listeners: Vec<CommandSourceStack>,
    scheduled_for_rerun: Vec<InfoId>,
    max_tests_per_batch: usize,
    stopped: bool,
    current_environment: Option<(Activation, usize)>,
    existing_structure_spawner: StructureSpawner,
    new_structure_spawner: StructureSpawner,
    halt_on_error: bool,
    clear_between_batches: bool,
}

pub struct Builder {
    batches: Vec<GameTestBatch>,
    max_tests_per_batch: usize,
    existing_structure_spawner: StructureSpawner,
    new_structure_spawner: StructureSpawner,
    halt_on_error: bool,
    clear_between_batches: bool,
}

impl Builder {
    pub fn from_batches(batches: Vec<GameTestBatch>) -> Builder {
        Builder {
            batches,
            max_tests_per_batch: game_test_batch_factory::MAX_TESTS_PER_BATCH,
            existing_structure_spawner: StructureSpawner::InPlace,
            new_structure_spawner: StructureSpawner::NotSet,
            halt_on_error: false,
            clear_between_batches: false,
        }
    }

    pub fn from_info(server: &Server, tests: Vec<InfoId>) -> Builder {
        Builder::from_batches(game_test_batch_factory::batch(server, &tests, game_test_batch_factory::MAX_TESTS_PER_BATCH))
    }

    pub fn halt_on_error(mut self) -> Builder {
        self.halt_on_error = true;
        self
    }

    pub fn clear_between_batches(mut self) -> Builder {
        self.clear_between_batches = true;
        self
    }

    pub fn new_structure_spawner(mut self, spawner: StructureSpawner) -> Builder {
        self.new_structure_spawner = spawner;
        self
    }

    pub fn existing_structure_spawner(mut self, spawner: StructureSpawner) -> Builder {
        self.existing_structure_spawner = spawner;
        self
    }

    pub fn batcher(mut self, max_tests_per_batch: usize) -> Builder {
        self.max_tests_per_batch = max_tests_per_batch;
        self
    }

    pub fn build(self, server: &mut Server) {
        let all_test_infos: Vec<InfoId> = self.batches.iter().flat_map(|batch| batch.game_test_infos.iter().copied()).collect();
        if server.game_test_ticker.runner.is_some() {
            warn!("The runner was already set in GameTestTicker");
        }
        server.game_test_ticker.runner = Some(GameTestRunner {
            all_test_infos: all_test_infos.clone(),
            batches: self.batches,
            batch_listeners: Vec::new(),
            scheduled_for_rerun: Vec::new(),
            max_tests_per_batch: self.max_tests_per_batch,
            stopped: true,
            current_environment: None,
            existing_structure_spawner: self.existing_structure_spawner,
            new_structure_spawner: self.new_structure_spawner,
            halt_on_error: self.halt_on_error,
            clear_between_batches: self.clear_between_batches,
        });
        for info in all_test_infos {
            server.game_test_ticker.report_listeners.push(ReportGameListener::default());
            let report = server.game_test_ticker.report_listeners.len() - 1;
            server.game_test_ticker.infos[info].add_listener(Listener::Report(report));
        }
    }
}

fn runner(server: &mut Server) -> &mut GameTestRunner {
    server.game_test_ticker.runner.as_mut().expect("a game test runner")
}

pub fn get_test_infos(server: &mut Server) -> Vec<InfoId> {
    runner(server).all_test_infos.clone()
}

pub fn add_listener(server: &mut Server, source: CommandSourceStack) {
    runner(server).batch_listeners.push(source);
}

pub fn start(server: &mut Server) {
    runner(server).stopped = false;
    run_batch(server, 0);
}

pub fn stop(server: &mut Server) {
    runner(server).stopped = true;
    if runner(server).current_environment.is_some() {
        end_current_environment(server);
    }
}

pub fn rerun_test(server: &mut Server, id: InfoId) {
    let copy = server.game_test_ticker.infos[id].copy_reset();
    server.game_test_ticker.infos.push(copy);
    let copy = server.game_test_ticker.infos.len() - 1;
    game_test_info::test_added_for_rerun(server, id, copy);
    if server.game_test_ticker.runner.is_none() {
        return;
    }
    runner(server).all_test_infos.push(copy);
    runner(server).scheduled_for_rerun.push(copy);
    if runner(server).stopped {
        run_scheduled_rerun_tests(server);
    }
}

fn run_batch(server: &mut Server, batch_index: usize) {
    if server.game_test_ticker.runner.is_none() {
        return;
    }
    if batch_index >= runner(server).batches.len() {
        end_current_environment(server);
        run_scheduled_rerun_tests(server);
        return;
    }
    if batch_index > 0 && runner(server).clear_between_batches {
        for info in runner(server).batches[batch_index - 1].game_test_infos.clone() {
            let level = server.game_test_ticker.infos[info].level;
            let pos = game_test_info::test_block_pos(server, info);
            let bounding_box = server.levels[level].test_instance_block_entity_get_test_bounding_box(pos);
            structure_utils::clear_space_for_structure(&bounding_box, &mut server.levels[level]);
            server.levels[level].destroy_block(pos, false, None, 512);
        }
    }
    let current_batch = runner(server).batches[batch_index].clone();
    let (existing, new) = (runner(server).existing_structure_spawner, runner(server).new_structure_spawner);
    on_batch_start(server, existing);
    on_batch_start(server, new);
    let test_infos_for_this_batch: Vec<InfoId> = current_batch.game_test_infos.iter().copied().filter(|&info| spawn(server, info)).collect();
    info!(
        "Running test environment '{}' batch {} ({} tests)...",
        current_batch.environment.registered_name(),
        current_batch.index,
        test_infos_for_this_batch.len()
    );
    end_current_environment(server);
    let level = server.get_level(&current_batch.dimension).unwrap_or(0);
    if let Some(definition) = current_batch.environment.value() {
        let activation = definition.activate(server, level);
        if server.game_test_ticker.runner.is_some() {
            runner(server).current_environment = Some((activation, level));
        }
    }
    if server.game_test_ticker.runner.is_none() {
        return;
    }
    for source in runner(server).batch_listeners.clone() {
        source.send_success(
            server,
            || Component::translatable_with("commands.test.batch.starting", vec![Arg::String(current_batch.environment.registered_name().to_string()), Arg::Int(current_batch.index)]),
            true,
        );
    }
    let tracker = multiple_test_tracker::new_tracker(server, Vec::new());
    for &info in &test_infos_for_this_batch {
        multiple_test_tracker::add_test_to_track(server, tracker, info);
    }
    multiple_test_tracker::add_listener(server, tracker, Listener::BatchCompletion { tracker, batch_index });
    for info in test_infos_for_this_batch {
        server.game_test_ticker.add(info);
    }
}

pub fn test_completed(server: &mut Server, tracker: usize, batch_index: usize, id: InfoId) {
    if !multiple_test_tracker::is_done(server, tracker) {
        return;
    }
    let level = server.game_test_ticker.infos[id].level;
    unforce_chunks(server, level);
    run_batch(server, batch_index + 1);
}

pub fn batch_test_failed(server: &mut Server, tracker: usize, batch_index: usize, id: InfoId) {
    if server.game_test_ticker.runner.as_ref().is_some_and(|runner| runner.halt_on_error) {
        end_current_environment(server);
        let level = server.game_test_ticker.infos[id].level;
        unforce_chunks(server, level);
        super::game_test_ticker::clear(server);
    } else {
        test_completed(server, tracker, batch_index, id);
    }
}

fn unforce_chunks(server: &mut Server, level: usize) {
    let forced_chunks: Vec<i64> = server.levels[level].chunks.force_loaded_chunks().iter().collect();
    for pos in forced_chunks {
        let pos = wafer_core::ChunkPos::unpack(pos);
        server.levels[level].set_chunk_forced(pos.x, pos.z, false);
    }
}

fn end_current_environment(server: &mut Server) {
    let Some(runner) = server.game_test_ticker.runner.as_mut() else { return };
    if let Some((activation, level)) = runner.current_environment.take() {
        activation.teardown(server, level);
    }
}

fn run_scheduled_rerun_tests(server: &mut Server) {
    if server.game_test_ticker.runner.is_none() {
        return;
    }
    if !runner(server).scheduled_for_rerun.is_empty() {
        let scheduled = std::mem::take(&mut runner(server).scheduled_for_rerun);
        let names: Vec<&str> = scheduled.iter().map(|&info| server.game_test_ticker.infos[info].id()).collect();
        info!("Starting re-run of tests: {}", names.join(", "));
        let max = runner(server).max_tests_per_batch;
        let batches = game_test_batch_factory::batch(server, &scheduled, max);
        runner(server).batches = batches;
        runner(server).stopped = false;
        run_batch(server, 0);
    } else {
        runner(server).batches = Vec::new();
        runner(server).stopped = true;
    }
}

fn on_batch_start(server: &mut Server, spawner: StructureSpawner) {
    if let StructureSpawner::Grid(grid) = spawner {
        structure_grid_spawner::on_batch_start(server, grid);
    }
}

fn spawn(server: &mut Server, info: InfoId) -> bool {
    let spawner = if server.game_test_ticker.infos[info].get_test_block_pos().is_none() {
        runner(server).new_structure_spawner
    } else {
        runner(server).existing_structure_spawner
    };
    match spawner {
        StructureSpawner::NotSet => false,
        StructureSpawner::InPlace => {
            if !game_test_info::prepare_test_structure(server, info) {
                return false;
            }
            server.game_test_ticker.infos[info].start_execution(1);
            true
        }
        StructureSpawner::Grid(grid) => structure_grid_spawner::spawn_structure(server, grid, info),
    }
}

pub fn new_info(server: &mut Server, info: GameTestInfo) -> InfoId {
    server.game_test_ticker.infos.push(info);
    server.game_test_ticker.infos.len() - 1
}
