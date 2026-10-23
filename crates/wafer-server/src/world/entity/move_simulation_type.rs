#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MoveSimulationType {
    AuthoritativeSide,
    AuthoritativeSideAndServer,
    ServerAndClient,
}
