#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Environment {
    Server,
}

impl Environment {
    pub fn from_dedicated_server() -> Environment {
        Environment::Server
    }

    pub fn get_description(self) -> &'static str {
        match self {
            Environment::Server => "server",
        }
    }
}
