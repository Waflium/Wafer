use super::game_test_info::{InfoId, Listener};
use super::test_command;
use crate::commands::source::CommandSourceStack;
use crate::network::chat::{Arg, ChatFormatting, Component};
use crate::server::Server;

#[derive(Default)]
pub struct MultipleTestTracker {
    pub tests: Vec<InfoId>,
    listeners: Vec<Listener>,
}

pub struct TestSummaryDisplayer {
    pub source: CommandSourceStack,
    pub tracker: usize,
    dimensions: Vec<String>,
}

pub fn new_tracker(server: &mut Server, tests: Vec<InfoId>) -> usize {
    server.game_test_ticker.trackers.push(MultipleTestTracker { tests, listeners: Vec::new() });
    server.game_test_ticker.trackers.len() - 1
}

pub fn add_test_to_track(server: &mut Server, tracker: usize, info: InfoId) {
    server.game_test_ticker.trackers[tracker].tests.push(info);
    for listener in server.game_test_ticker.trackers[tracker].listeners.clone() {
        server.game_test_ticker.infos[info].add_listener(listener);
    }
}

pub fn add_listener(server: &mut Server, tracker: usize, listener: Listener) {
    server.game_test_ticker.trackers[tracker].listeners.push(listener);
    for info in server.game_test_ticker.trackers[tracker].tests.clone() {
        server.game_test_ticker.infos[info].add_listener(listener);
    }
}

pub fn add_summary_displayer(server: &mut Server, tracker: usize, source: CommandSourceStack) {
    server.game_test_ticker.summaries.push(TestSummaryDisplayer { source, tracker, dimensions: Vec::new() });
    let summary = server.game_test_ticker.summaries.len() - 1;
    add_listener(server, tracker, Listener::Summary(summary));
}

fn tracked<'a>(server: &'a Server, tracker: usize) -> impl Iterator<Item = &'a super::game_test_info::GameTestInfo> + 'a {
    server.game_test_ticker.trackers[tracker].tests.iter().map(|&id| &server.game_test_ticker.infos[id])
}

pub fn get_failed_required_count(server: &Server, tracker: usize) -> i32 {
    tracked(server, tracker).filter(|info| info.has_failed() && info.is_required()).count() as i32
}

pub fn get_failed_optional_count(server: &Server, tracker: usize) -> i32 {
    tracked(server, tracker).filter(|info| info.has_failed() && info.is_optional()).count() as i32
}

pub fn get_done_count(server: &Server, tracker: usize) -> i32 {
    tracked(server, tracker).filter(|info| info.is_done()).count() as i32
}

pub fn get_total_count(server: &Server, tracker: usize) -> i32 {
    server.game_test_ticker.trackers[tracker].tests.len() as i32
}

pub fn is_done(server: &Server, tracker: usize) -> bool {
    get_done_count(server, tracker) == get_total_count(server, tracker)
}

pub fn summary_test_passed(server: &mut Server, summary: usize, id: InfoId) {
    add_dimension(server, summary, id);
    show_test_summary_if_all_done(server, summary);
}

pub fn summary_test_failed(server: &mut Server, summary: usize, id: InfoId) {
    add_dimension(server, summary, id);
    show_test_summary_if_all_done(server, summary);
}

fn add_dimension(server: &mut Server, summary: usize, id: InfoId) {
    let dimension = server.game_test_ticker.infos[id].test.1.dimension().to_string();
    let dimensions = &mut server.game_test_ticker.summaries[summary].dimensions;
    if !dimensions.contains(&dimension) {
        dimensions.push(dimension);
    }
}

fn show_test_summary_if_all_done(server: &mut Server, summary: usize) {
    let tracker = server.game_test_ticker.summaries[summary].tracker;
    if !is_done(server, tracker) {
        return;
    }
    let source = server.game_test_ticker.summaries[summary].source.clone();
    let total = get_total_count(server, tracker);
    source.send_success(server, || Component::translatable_with("commands.test.summary", vec![Arg::Int(total)]).with_color(ChatFormatting::White), true);
    let failed_required = get_failed_required_count(server, tracker);
    if failed_required > 0 {
        source.send_failure(server, Component::translatable_with("commands.test.summary.failed", vec![Arg::Int(failed_required)]));
    } else {
        source.send_success(server, || Component::translatable("commands.test.summary.all_required_passed").with_color(ChatFormatting::Green), true);
    }
    let failed_optional = get_failed_optional_count(server, tracker);
    if failed_optional > 0 {
        source.send_system_message(server, &Component::translatable_with("commands.test.summary.optional_failed", vec![Arg::Int(failed_optional)]));
    }
    let dimensions = server.game_test_ticker.summaries[summary].dimensions.clone();
    test_command::output_player_coordinates(server, &source, &dimensions);
    test_command::output_test_coordinates(server, &source, &dimensions);
}
