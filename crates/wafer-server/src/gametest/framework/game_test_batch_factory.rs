use super::game_test_info::InfoId;
use super::test_environment_definition::EnvironmentHolder;
use crate::server::Server;

pub const MAX_TESTS_PER_BATCH: usize = 50;

#[derive(Clone)]
pub struct GameTestBatch {
    pub index: i32,
    pub game_test_infos: Vec<InfoId>,
    pub environment: &'static EnvironmentHolder,
    pub dimension: String,
}

pub fn batch(server: &Server, game_test_infos: &[InfoId], max_tests_per_batch: usize) -> Vec<GameTestBatch> {
    let mut tests_per_batch: Vec<((String, String), &'static EnvironmentHolder, Vec<InfoId>)> = Vec::new();
    for &info in game_test_infos {
        let test = server.game_test_ticker.infos[info].test.1;
        let key = (test.batch().batch_key(), test.dimension().to_string());
        match tests_per_batch.iter_mut().find(|(existing, _, _)| *existing == key) {
            Some((_, _, tests)) => tests.push(info),
            None => tests_per_batch.push((key, test.batch(), vec![info])),
        }
    }
    let mut batches = Vec::new();
    for ((_, dimension), environment, tests) in tests_per_batch {
        for (index, tests) in tests.chunks(max_tests_per_batch).enumerate() {
            batches.push(to_game_test_batch(tests.to_vec(), environment, index as i32, dimension.clone()));
        }
    }
    batches
}

pub fn to_game_test_batch(tests: Vec<InfoId>, environment: &'static EnvironmentHolder, counter: i32, dimension: String) -> GameTestBatch {
    GameTestBatch { index: counter, game_test_infos: tests, environment, dimension }
}
