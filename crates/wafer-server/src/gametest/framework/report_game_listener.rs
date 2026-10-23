use super::game_test_exception::GameTestException;
use super::game_test_info::{self, InfoId};
use super::game_test_runner;
use crate::network::chat::{ChatFormatting, Component};
use crate::server::Server;

#[derive(Default)]
pub struct ReportGameListener {
    attempts: i32,
    successes: i32,
}

impl ReportGameListener {
    pub fn test_structure_loaded(&mut self) {
        self.attempts += 1;
    }
}

enum Failure {
    Test(GameTestException),
    ExhaustedAttempts { message: String, cause: Option<GameTestException> },
}

fn handle_retry(server: &mut Server, listener: usize, id: InfoId, passed: bool) {
    let ReportGameListener { attempts, successes } = server.game_test_ticker.report_listeners[listener];
    let info = &server.game_test_ticker.infos[id];
    let retry_options = info.retry_options();
    let mut report_as = format!("[Run: {attempts:4}, Ok: {successes:4}, Fail: {:4}", attempts - successes);
    if !retry_options.unlimited_tries() {
        report_as += &format!(", Left: {:4}", retry_options.number_of_tries - attempts);
    }
    report_as += "]";
    let name_part = format!("{} {}! {}ms", info.id(), if passed { "passed" } else { "failed" }, info.get_run_time());
    let text = format!("{report_as:<53}{name_part}");
    if passed {
        report_passed(server, id, &text);
    } else {
        say(server, ChatFormatting::Red, &text);
    }
    if retry_options.has_tries_left(attempts, successes) {
        game_test_runner::rerun_test(server, id);
    }
}

pub fn test_passed(server: &mut Server, listener: usize, id: InfoId) {
    server.game_test_ticker.report_listeners[listener].successes += 1;
    let info = &server.game_test_ticker.infos[id];
    if info.retry_options().has_retries() {
        handle_retry(server, listener, id, true);
    } else if !info.is_flaky() {
        let text = format!("{} passed! ({}ms / {}gameticks)", info.id(), info.get_run_time(), info.get_tick());
        report_passed(server, id, &text);
    } else {
        let ReportGameListener { attempts, successes } = server.game_test_ticker.report_listeners[listener];
        if successes >= info.required_successes() {
            let text = format!("{} passed {successes} times of {attempts} attempts.", info.id());
            report_passed(server, id, &text);
        } else {
            let text = format!("Flaky test {} succeeded, attempt: {attempts} successes: {successes}", info.id());
            say(server, ChatFormatting::Green, &text);
            game_test_runner::rerun_test(server, id);
        }
    }
}

pub fn test_failed(server: &mut Server, listener: usize, id: InfoId) {
    let info = &server.game_test_ticker.infos[id];
    if !info.is_flaky() {
        let error = info.get_error().cloned().expect("a failed test has an error");
        report_failure(server, id, Failure::Test(error));
        if server.game_test_ticker.infos[id].retry_options().has_retries() {
            handle_retry(server, listener, id, false);
        }
    } else {
        let ReportGameListener { attempts, successes } = server.game_test_ticker.report_listeners[listener];
        let test = info.test.1;
        let mut text = format!("Flaky test {} failed, attempt: {attempts}/{}", info.id(), test.max_attempts());
        if test.required_successes() > 1 {
            text += &format!(", successes: {successes} ({} required)", test.required_successes());
        }
        say(server, ChatFormatting::Yellow, &text);
        let info = &server.game_test_ticker.infos[id];
        if info.max_attempts() - attempts + successes >= info.required_successes() {
            game_test_runner::rerun_test(server, id);
        } else {
            let message = format!(
                "Not enough successes: {successes} out of {attempts} attempts. Required successes: {}. max attempts: {}.",
                info.required_successes(),
                info.max_attempts()
            );
            let cause = info.get_error().cloned();
            report_failure(server, id, Failure::ExhaustedAttempts { message, cause });
        }
    }
}

pub fn report_passed(server: &mut Server, id: InfoId, text: &str) {
    if let Some((level, pos)) = test_instance_block_entity(server, id) {
        server.levels[level].test_instance_block_entity_set_success(pos);
    }
    visualize_passed_test(server, text);
}

fn visualize_passed_test(server: &mut Server, text: &str) {
    say(server, ChatFormatting::Green, text);
}

fn report_failure(server: &mut Server, id: InfoId, error: Failure) {
    let description = match &error {
        Failure::Test(error) if error.is_assert() => error.get_description(),
        Failure::Test(error) => Component::literal(error.get_message()),
        Failure::ExhaustedAttempts { message, cause } => Component::literal(cause.as_ref().map_or_else(|| message.clone(), GameTestException::get_message)),
    };
    if let Some((level, pos)) = test_instance_block_entity(server, id) {
        server.levels[level].test_instance_block_entity_set_error_message(pos, description);
    }
    visualize_failed_test(server, id, &error);
}

fn visualize_failed_test(server: &mut Server, id: InfoId, error: &Failure) {
    let error_message = match error {
        Failure::Test(error) => error.get_message(),
        Failure::ExhaustedAttempts { message, cause: Some(cause) } => format!("{message} cause: {}", cause.get_message()),
        Failure::ExhaustedAttempts { message, cause: None } => message.clone(),
    };
    let info = &server.game_test_ticker.infos[id];
    let failure_message = format!("{}{} failed! {error_message}", if info.is_required() { "" } else { "(optional) " }, info.id());
    let format = if info.is_required() { ChatFormatting::Red } else { ChatFormatting::Yellow };
    say(server, format, &failure_message);
    let root_cause = match error {
        Failure::Test(error) => Some(error),
        Failure::ExhaustedAttempts { cause, .. } => cause.as_ref(),
    };
    if let Some(GameTestException::AssertPos { message, absolute_pos, .. }) = root_cause {
        let (level, pos) = (server.game_test_ticker.infos[id].level, game_test_info::test_block_pos(server, id));
        server.levels[level].test_instance_block_entity_mark_error(pos, *absolute_pos, message.clone());
    }
    on_test_failed(server, id);
}

fn on_test_failed(server: &mut Server, id: InfoId) {
    let info = &server.game_test_ticker.infos[id];
    let test_position = info.get_test_block_pos().map_or_else(String::new, |pos| pos.to_short_string());
    let error = info.get_error().map_or_else(String::new, GameTestException::get_message);
    if info.is_required() {
        error!("{} failed at {test_position}! {error}", info.id());
    } else {
        warn!("(optional) {} failed at {test_position}. {error}", info.id());
    }
}

fn test_instance_block_entity(server: &mut Server, id: InfoId) -> Option<(usize, wafer_core::BlockPos)> {
    let info = &server.game_test_ticker.infos[id];
    let (level, pos) = (info.level, info.get_test_block_pos()?);
    server.levels[level].test_instance_block_entity(pos)?;
    Some((level, pos))
}

pub fn say(server: &mut Server, format: ChatFormatting, text: &str) {
    let message = Component::literal(text).with_color(format);
    for player in server.player_ids() {
        server.send_player_system_message(player, &message, false);
    }
}
