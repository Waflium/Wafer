use std::time::{Duration, Instant};

use wafer_core::{BlockPos, ChunkPos};
use wafer_data::blocks;

use super::game_test_exception::GameTestException;
use super::game_test_helper;
use super::game_test_instance::{GameTestInstance, GameTestInstanceType};
use super::retry_options::RetryOptions;
use super::{block_based_test_instance, game_test_runner, multiple_test_tracker, report_game_listener};
use crate::network::chat::{Arg, Component};
use crate::server::Server;
use crate::world::level::block::entity::test_instance_block_entity::{self, Data, Status};
use crate::world::level::block::rotation::Rotation;

pub type InfoId = usize;

#[derive(Clone, Copy, PartialEq)]
pub enum Listener {
    Report(usize),
    Summary(usize),
    RememberFailure,
    BatchCompletion { tracker: usize, batch_index: usize },
}

#[derive(Clone, Copy)]
pub enum TickAction {
    BlockBasedEachTick,
}

pub struct GameTestInfo {
    pub test: (&'static str, &'static GameTestInstance),
    test_block_pos: Option<BlockPos>,
    pub level: usize,
    pub listeners: Vec<Listener>,
    timeout_ticks: i32,
    run_at_tick_time_map: Vec<(i64, TickAction)>,
    placed_structure: bool,
    chunks_loaded: bool,
    tick_count: i32,
    started: bool,
    retry_options: RetryOptions,
    timer_start: Option<Instant>,
    timer_elapsed: Duration,
    done: bool,
    extra_rotation: Rotation,
    error: Option<GameTestException>,
    has_test_instance_block_entity: bool,
}

impl GameTestInfo {
    pub fn new(test: (&'static str, &'static GameTestInstance), extra_rotation: Rotation, level: usize, retry_options: RetryOptions) -> GameTestInfo {
        GameTestInfo {
            test,
            test_block_pos: None,
            level,
            listeners: Vec::new(),
            timeout_ticks: test.1.max_ticks(),
            run_at_tick_time_map: Vec::new(),
            placed_structure: false,
            chunks_loaded: false,
            tick_count: 0,
            started: false,
            retry_options,
            timer_start: None,
            timer_elapsed: Duration::ZERO,
            done: false,
            extra_rotation,
            error: None,
            has_test_instance_block_entity: false,
        }
    }

    pub fn set_test_block_pos(&mut self, test_block_pos: Option<BlockPos>) {
        self.test_block_pos = test_block_pos;
    }

    pub fn get_test_block_pos(&self) -> Option<BlockPos> {
        self.test_block_pos
    }

    pub fn start_execution(&mut self, tick_delay: i32) {
        self.tick_count = -(self.test.1.setup_ticks() + tick_delay + 1);
    }

    pub fn set_run_at_tick_time(&mut self, time: i64, action: TickAction) {
        self.run_at_tick_time_map.push((time, action));
    }

    pub fn id(&self) -> &'static str {
        self.test.0
    }

    pub fn has_failed(&self) -> bool {
        self.error.is_some()
    }

    pub fn is_done(&self) -> bool {
        self.done
    }

    pub fn get_run_time(&self) -> u128 {
        (self.timer_elapsed + self.timer_start.map_or(Duration::ZERO, |start| start.elapsed())).as_millis()
    }

    fn finish(&mut self) {
        if !self.done {
            self.done = true;
            if let Some(start) = self.timer_start.take() {
                self.timer_elapsed += start.elapsed();
            }
        }
    }

    pub fn fail_with(&mut self, error: GameTestException) {
        self.error = Some(error);
    }

    pub fn fail(&mut self, message: Component) {
        let tick = self.tick_count;
        self.fail_with(GameTestException::Assert { message, tick });
    }

    pub fn get_error(&self) -> Option<&GameTestException> {
        self.error.as_ref()
    }

    pub fn get_tick(&self) -> i32 {
        self.tick_count
    }

    pub fn is_required(&self) -> bool {
        self.test.1.required()
    }

    pub fn is_optional(&self) -> bool {
        !self.test.1.required()
    }

    pub fn get_rotation(&self) -> Rotation {
        self.test.1.rotation().get_rotated(self.extra_rotation)
    }

    pub fn get_timeout_ticks(&self) -> i32 {
        self.timeout_ticks
    }

    pub fn is_flaky(&self) -> bool {
        self.test.1.max_attempts() > 1
    }

    pub fn max_attempts(&self) -> i32 {
        self.test.1.max_attempts()
    }

    pub fn required_successes(&self) -> i32 {
        self.test.1.required_successes()
    }

    pub fn retry_options(&self) -> RetryOptions {
        self.retry_options
    }

    pub fn add_listener(&mut self, listener: Listener) {
        self.listeners.push(listener);
    }

    pub fn copy_reset(&self) -> GameTestInfo {
        let mut info = GameTestInfo::new(self.test, self.extra_rotation, self.level, self.retry_options);
        if self.test_block_pos.is_some() {
            info.set_test_block_pos(self.test_block_pos);
        }
        info
    }
}

fn info(server: &mut Server, id: InfoId) -> &mut GameTestInfo {
    &mut server.game_test_ticker.infos[id]
}

pub fn test_block_pos(server: &mut Server, id: InfoId) -> BlockPos {
    info(server, id).test_block_pos.expect("This GameTestInfo has no position")
}

pub fn get_test_origin(server: &mut Server, id: InfoId) -> BlockPos {
    let (level, pos) = (info(server, id).level, test_block_pos(server, id));
    server.levels[level].test_instance_block_entity_get_start_corner(pos)
}

pub fn place_structure(server: &mut Server, id: InfoId) {
    if info(server, id).placed_structure {
        return;
    }
    mark_has_test_instance_block_entity(server, id);
    let (level, pos) = (info(server, id).level, test_block_pos(server, id));
    if !server.levels[level].test_instance_block_entity_place_structure(pos) {
        let name = server.levels[level].test_instance_block_entity_get_test_name(pos).get_string();
        info(server, id).fail(Component::translatable_with("test.error.structure.failure", vec![Arg::String(name)]));
    }
    info(server, id).placed_structure = true;
    server.levels[level].test_instance_block_entity_encase_structure(pos);
    let bounding_box = server.levels[level].test_instance_block_entity_get_test_bounding_box(pos);
    server.levels[level].chunks.block_ticks.clear_area(&bounding_box);
    server.levels[level].clear_block_events(&bounding_box);
    for listener in info(server, id).listeners.clone() {
        if let Listener::Report(report) = listener {
            server.game_test_ticker.report_listeners[report].test_structure_loaded();
        }
    }
}

pub fn tick(server: &mut Server, id: InfoId) {
    if info(server, id).is_done() {
        return;
    }
    if !info(server, id).placed_structure {
        info(server, id).fail(Component::translatable("test.error.ticking_without_structure"));
    }
    if !info(server, id).has_test_instance_block_entity {
        info(server, id).fail(Component::translatable("test.error.missing_block_entity"));
    }
    if info(server, id).error.is_some() {
        info(server, id).finish();
    } else {
        if !info(server, id).chunks_loaded {
            let (level, pos) = (info(server, id).level, test_block_pos(server, id));
            let bounding_box = server.levels[level].test_instance_block_entity_get_structure_bounding_box(pos);
            let loaded = (bounding_box.min.x >> 4..=bounding_box.max.x >> 4)
                .flat_map(|x| (bounding_box.min.z >> 4..=bounding_box.max.z >> 4).map(move |z| ChunkPos::new(x, z)))
                .all(|chunk| server.levels[level].are_entities_actually_loaded_and_ticking(chunk));
            if !loaded {
                return;
            }
        }
        info(server, id).chunks_loaded = true;
    }
    tick_internal(server, id);
    if info(server, id).is_done() {
        if info(server, id).error.is_some() {
            test_failed(server, id);
        } else {
            test_passed(server, id);
        }
    }
}

fn tick_internal(server: &mut Server, id: InfoId) {
    info(server, id).tick_count += 1;
    let tick_count = info(server, id).tick_count;
    if tick_count < 0 {
        return;
    }
    if !info(server, id).started {
        start_test(server, id);
    }
    let mut index = 0;
    while index < info(server, id).run_at_tick_time_map.len() {
        let (time, action) = info(server, id).run_at_tick_time_map[index];
        if time <= tick_count as i64 {
            let result = match action {
                TickAction::BlockBasedEachTick => block_based_test_instance::each_tick(server, id),
            };
            if let Err(error) = result {
                info(server, id).fail_with(error);
            }
            info(server, id).run_at_tick_time_map.remove(index);
        } else {
            index += 1;
        }
    }
    if tick_count > info(server, id).timeout_ticks {
        let max_ticks = info(server, id).test.1.max_ticks();
        info(server, id).fail_with(GameTestException::Timeout(Component::translatable_with("test.error.timeout.no_result", vec![Arg::Int(max_ticks)])));
    }
}

fn start_test(server: &mut Server, id: InfoId) {
    if info(server, id).started {
        return;
    }
    info(server, id).started = true;
    info(server, id).timer_start = Some(Instant::now());
    let (level, pos) = (info(server, id).level, test_block_pos(server, id));
    server.levels[level].test_instance_block_entity_set_running(pos);
    let result = match &info(server, id).test.1.kind {
        GameTestInstanceType::Function(function) if function == "minecraft:always_pass" => {
            game_test_helper::succeed(server, id);
            Ok(())
        }
        GameTestInstanceType::Function(function) => Err(GameTestException::Unknown(format!("Trying to access missing test function: {function}"))),
        GameTestInstanceType::BlockBased => block_based_test_instance::run(server, id),
    };
    if let Err(error) = result {
        info(server, id).fail_with(error);
    }
}

pub fn succeed(server: &mut Server, id: InfoId) {
    if info(server, id).error.is_some() {
        return;
    }
    info(server, id).finish();
    let (level, pos) = (info(server, id).level, test_block_pos(server, id));
    let bounds = server.levels[level].test_instance_block_entity_get_structure_bounds(pos).inflate_all(1.0);
    let level = &mut server.levels[level];
    for entity in level.get_entities(None, &bounds) {
        if !level.is_player(entity) {
            level.discard(entity);
        }
    }
}

pub fn prepare_test_structure(server: &mut Server, id: InfoId) -> bool {
    let (level, pos, extra_rotation, key) = {
        let info = info(server, id);
        (info.level, info.test_block_pos.expect("a test position"), info.extra_rotation, info.test.0)
    };
    let level_ref = &mut server.levels[level];
    level_ref.set_block(pos, blocks::TEST_INSTANCE_BLOCK.default_state(), 3);
    if level_ref.test_instance_block_entity(pos).is_none() {
        return false;
    }
    let size = test_instance_block_entity::get_structure_size(key).unwrap_or(BlockPos::new(1, 1, 1));
    level_ref.test_instance_block_entity_set(
        pos,
        Data { test: Some(key.to_string()), size, rotation: extra_rotation, ignore_entities: false, status: Status::Cleared, error_message: None },
    );
    info(server, id).has_test_instance_block_entity = true;
    place_structure(server, id);
    true
}

pub fn mark_has_test_instance_block_entity(server: &mut Server, id: InfoId) {
    let (level, pos) = (info(server, id).level, test_block_pos(server, id));
    if server.levels[level].test_instance_block_entity(pos).is_some() {
        info(server, id).has_test_instance_block_entity = true;
    }
}

fn test_passed(server: &mut Server, id: InfoId) {
    for listener in info(server, id).listeners.clone() {
        match listener {
            Listener::Report(report) => report_game_listener::test_passed(server, report, id),
            Listener::Summary(summary) => multiple_test_tracker::summary_test_passed(server, summary, id),
            Listener::RememberFailure => {}
            Listener::BatchCompletion { tracker, batch_index } => {
                let (level, pos) = (info(server, id).level, test_block_pos(server, id));
                server.levels[level].test_instance_block_entity_remove_barriers(pos);
                game_test_runner::test_completed(server, tracker, batch_index, id);
            }
        }
    }
}

fn test_failed(server: &mut Server, id: InfoId) {
    for listener in info(server, id).listeners.clone() {
        match listener {
            Listener::Report(report) => report_game_listener::test_failed(server, report, id),
            Listener::Summary(summary) => multiple_test_tracker::summary_test_failed(server, summary, id),
            Listener::RememberFailure => {
                let test = info(server, id).test.0;
                server.game_test_ticker.remember_failed_test(test);
            }
            Listener::BatchCompletion { tracker, batch_index } => game_test_runner::batch_test_failed(server, tracker, batch_index, id),
        }
    }
}

pub fn test_added_for_rerun(server: &mut Server, original: InfoId, copy: InfoId) {
    for listener in info(server, original).listeners.clone() {
        match listener {
            Listener::Report(report) => info(server, copy).add_listener(Listener::Report(report)),
            Listener::Summary(summary) => {
                let tracker = server.game_test_ticker.summaries[summary].tracker;
                multiple_test_tracker::add_test_to_track(server, tracker, copy);
            }
            Listener::RememberFailure | Listener::BatchCompletion { .. } => {}
        }
    }
}
