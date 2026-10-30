#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MetricCategory {
    PathFinding,
    EventLoops,
    ConsecutiveExecutors,
    TickLoop,
    Jvm,
    ChunkRendering,
    ChunkRenderingDispatching,
    Cpu,
    Gpu,
}

impl MetricCategory {
    pub fn get_description(self) -> &'static str {
        match self {
            MetricCategory::PathFinding => "pathfinding",
            MetricCategory::EventLoops => "event-loops",
            MetricCategory::ConsecutiveExecutors => "consecutive-executors",
            MetricCategory::TickLoop => "ticking",
            MetricCategory::Jvm => "jvm",
            MetricCategory::ChunkRendering => "chunk rendering",
            MetricCategory::ChunkRenderingDispatching => "chunk rendering dispatching",
            MetricCategory::Cpu => "cpu",
            MetricCategory::Gpu => "gpu",
        }
    }
}
