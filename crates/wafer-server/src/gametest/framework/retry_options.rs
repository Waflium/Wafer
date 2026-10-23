#[derive(Clone, Copy)]
pub struct RetryOptions {
    pub number_of_tries: i32,
    pub halt_on_failure: bool,
}

impl RetryOptions {
    pub fn no_retries() -> RetryOptions {
        RetryOptions { number_of_tries: 1, halt_on_failure: true }
    }

    pub fn unlimited_tries(&self) -> bool {
        self.number_of_tries < 1
    }

    pub fn has_tries_left(&self, attempts: i32, successes: i32) -> bool {
        let has_failures = attempts != successes;
        let has_more_attempts = self.unlimited_tries() || attempts < self.number_of_tries;
        has_more_attempts && (!has_failures || !self.halt_on_failure)
    }

    pub fn has_retries(&self) -> bool {
        self.number_of_tries != 1
    }
}
