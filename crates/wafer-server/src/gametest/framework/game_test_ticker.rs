use super::game_test_info::{self, GameTestInfo, InfoId};
use super::game_test_runner::{self, GameTestRunner};
use super::multiple_test_tracker::{MultipleTestTracker, TestSummaryDisplayer};
use super::report_game_listener::ReportGameListener;
use super::structure_grid_spawner::StructureGridSpawner;
use crate::server::Server;

#[derive(Clone, Copy, PartialEq, Default)]
enum State {
    #[default]
    Idle,
    Running,
    Halting,
}

#[derive(Default)]
pub struct GameTestTicker {
    test_infos: Vec<InfoId>,
    pub runner: Option<GameTestRunner>,
    state: State,
    pub infos: Vec<GameTestInfo>,
    pub report_listeners: Vec<ReportGameListener>,
    pub trackers: Vec<MultipleTestTracker>,
    pub summaries: Vec<TestSummaryDisplayer>,
    pub grid_spawners: Vec<StructureGridSpawner>,
    last_failed_tests: Vec<&'static str>,
}

impl GameTestTicker {
    pub fn add(&mut self, info: InfoId) {
        self.test_infos.push(info);
    }

    pub fn remember_failed_test(&mut self, test: &'static str) {
        if !self.last_failed_tests.contains(&test) {
            self.last_failed_tests.push(test);
        }
    }

    pub fn forget_failed_tests(&mut self) {
        self.last_failed_tests.clear();
    }

    pub fn get_last_failed_tests(&self) -> Vec<&'static str> {
        self.last_failed_tests.clone()
    }

    pub fn reset_arena_if_idle(&mut self) {
        if self.runner.is_none() && self.test_infos.is_empty() {
            self.infos.clear();
            self.report_listeners.clear();
            self.trackers.clear();
            self.summaries.clear();
            self.grid_spawners.clear();
        }
    }
}

pub fn clear(server: &mut Server) {
    let ticker = &mut server.game_test_ticker;
    if ticker.state != State::Idle {
        ticker.state = State::Halting;
        return;
    }
    ticker.test_infos.clear();
    if ticker.runner.is_some() {
        game_test_runner::stop(server);
        server.game_test_ticker.runner = None;
    }
}

pub fn tick(server: &mut Server) {
    if server.game_test_ticker.runner.is_none() {
        return;
    }
    server.game_test_ticker.state = State::Running;
    for info in server.game_test_ticker.test_infos.clone() {
        if server.game_test_ticker.runner.is_none() {
            break;
        }
        game_test_info::tick(server, info);
    }
    let infos = &server.game_test_ticker.infos;
    let mut test_infos = std::mem::take(&mut server.game_test_ticker.test_infos);
    test_infos.retain(|&info| !infos[info].is_done());
    server.game_test_ticker.test_infos = test_infos;
    let finishing_state = server.game_test_ticker.state;
    server.game_test_ticker.state = State::Idle;
    if finishing_state == State::Halting {
        clear(server);
    }
}
