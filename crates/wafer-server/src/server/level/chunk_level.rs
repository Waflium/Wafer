use std::sync::LazyLock;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum ChunkStatus {
    Empty,
    StructureStarts,
    StructureReferences,
    Biomes,
    Terrain,
    Features,
    InitializeLight,
    Light,
    Spawn,
    Full,
}

impl ChunkStatus {
    pub const ALL: [ChunkStatus; 10] = [
        ChunkStatus::Empty,
        ChunkStatus::StructureStarts,
        ChunkStatus::StructureReferences,
        ChunkStatus::Biomes,
        ChunkStatus::Terrain,
        ChunkStatus::Features,
        ChunkStatus::InitializeLight,
        ChunkStatus::Light,
        ChunkStatus::Spawn,
        ChunkStatus::Full,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn get_name(self) -> &'static str {
        match self {
            ChunkStatus::Empty => "empty",
            ChunkStatus::StructureStarts => "structure_starts",
            ChunkStatus::StructureReferences => "structure_references",
            ChunkStatus::Biomes => "biomes",
            ChunkStatus::Terrain => "terrain",
            ChunkStatus::Features => "features",
            ChunkStatus::InitializeLight => "initialize_light",
            ChunkStatus::Light => "light",
            ChunkStatus::Spawn => "spawn",
            ChunkStatus::Full => "full",
        }
    }

    pub fn by_name(name: &str) -> Option<ChunkStatus> {
        let name = name.strip_prefix("minecraft:").unwrap_or(name);
        ChunkStatus::ALL.into_iter().find(|status| status.get_name() == name)
    }

    pub fn parent(self) -> ChunkStatus {
        ChunkStatus::ALL[self.index().saturating_sub(1)]
    }

    pub fn next(self) -> ChunkStatus {
        ChunkStatus::ALL[(self.index() + 1).min(ChunkStatus::ALL.len() - 1)]
    }

    pub fn accumulated_radius_of(self, status: ChunkStatus) -> i32 {
        ChunkPyramid::Generation.accumulated_radius_of(self, status)
    }

    pub fn dependency_at(self, distance: i32) -> ChunkStatus {
        ChunkPyramid::Generation.step_to(self).accumulated_dependencies.get(distance)
    }
}

pub struct ChunkDependencies {
    dependency_by_radius: Vec<ChunkStatus>,
    radius_by_dependency: Vec<i32>,
}

impl ChunkDependencies {
    fn new(dependency_by_radius: Vec<ChunkStatus>) -> ChunkDependencies {
        let size = dependency_by_radius.first().map_or(0, |first| first.index() + 1);
        let mut radius_by_dependency = vec![0; size];
        for (radius, dependency) in dependency_by_radius.iter().enumerate() {
            for status_index in 0..=dependency.index() {
                radius_by_dependency[status_index] = radius as i32;
            }
        }
        ChunkDependencies { dependency_by_radius, radius_by_dependency }
    }

    pub fn size(&self) -> usize {
        self.dependency_by_radius.len()
    }

    pub fn get_radius_of(&self, status: ChunkStatus) -> i32 {
        match self.radius_by_dependency.get(status.index()) {
            Some(&radius) => radius,
            None => panic!("Requesting a ChunkStatus({}) outside of dependency range({:?})", status.get_name(), self.dependency_by_radius),
        }
    }

    pub fn get_radius(&self) -> i32 {
        (self.dependency_by_radius.len() as i32 - 1).max(0)
    }

    pub fn get(&self, distance: i32) -> ChunkStatus {
        self.dependency_by_radius[distance as usize]
    }
}

pub struct ChunkStep {
    pub target_status: ChunkStatus,
    pub accumulated_dependencies: ChunkDependencies,
}

impl ChunkStep {
    pub fn get_accumulated_radius_of(&self, status: ChunkStatus) -> i32 {
        if status == self.target_status { 0 } else { self.accumulated_dependencies.get_radius_of(status) }
    }
}

struct ChunkStepBuilder<'a> {
    status: ChunkStatus,
    parent: Option<&'a ChunkStep>,
    direct_dependencies_by_radius: Vec<ChunkStatus>,
}

impl<'a> ChunkStepBuilder<'a> {
    fn new(status: ChunkStatus, parent: Option<&'a ChunkStep>) -> ChunkStepBuilder<'a> {
        let direct_dependencies_by_radius = match parent {
            Some(parent) => vec![parent.target_status],
            None => Vec::new(),
        };
        ChunkStepBuilder { status, parent, direct_dependencies_by_radius }
    }

    fn add_requirement(mut self, status: ChunkStatus, radius: i32) -> ChunkStepBuilder<'a> {
        assert!(status < self.status, "Status {} can not be required by {}", status.get_name(), self.status.get_name());
        let previous = std::mem::take(&mut self.direct_dependencies_by_radius);
        let new_length = radius as usize + 1;
        self.direct_dependencies_by_radius = if new_length > previous.len() { vec![status; new_length] } else { previous.clone() };
        for i in 0..new_length.min(previous.len()) {
            self.direct_dependencies_by_radius[i] = previous[i].max(status);
        }
        self
    }

    fn build(self) -> ChunkStep {
        let accumulated = self.build_accumulated_dependencies();
        ChunkStep { target_status: self.status, accumulated_dependencies: ChunkDependencies::new(accumulated) }
    }

    fn build_accumulated_dependencies(&self) -> Vec<ChunkStatus> {
        let Some(parent) = self.parent else { return self.direct_dependencies_by_radius.clone() };
        let direct = &self.direct_dependencies_by_radius;
        let radius_of_parent = self.get_radius_of_parent(parent.target_status);
        let parent_dependencies = &parent.accumulated_dependencies;
        let length = (radius_of_parent + parent_dependencies.size()).max(direct.len());
        (0..length)
            .map(|distance| {
                let distance_in_parent = distance as i64 - radius_of_parent as i64;
                if distance_in_parent < 0 || distance_in_parent >= parent_dependencies.size() as i64 {
                    direct[distance]
                } else if distance >= direct.len() {
                    parent_dependencies.get(distance_in_parent as i32)
                } else {
                    direct[distance].max(parent_dependencies.get(distance_in_parent as i32))
                }
            })
            .collect()
    }

    fn get_radius_of_parent(&self, status: ChunkStatus) -> usize {
        self.direct_dependencies_by_radius.iter().rposition(|&dependency| dependency >= status).unwrap_or(0)
    }
}

struct ChunkPyramidSteps {
    steps: Vec<ChunkStep>,
}

fn build_pyramid(requirements: impl Fn(ChunkStatus) -> &'static [(ChunkStatus, i32)]) -> ChunkPyramidSteps {
    let mut steps: Vec<ChunkStep> = Vec::with_capacity(ChunkStatus::ALL.len());
    for status in ChunkStatus::ALL {
        let mut builder = ChunkStepBuilder::new(status, steps.last());
        for &(required, radius) in requirements(status) {
            builder = builder.add_requirement(required, radius);
        }
        let step = builder.build();
        steps.push(step);
    }
    ChunkPyramidSteps { steps }
}

static GENERATION_PYRAMID: LazyLock<ChunkPyramidSteps> = LazyLock::new(|| {
    use ChunkStatus::*;
    build_pyramid(|status| match status {
        StructureReferences => &[(StructureStarts, 8)],
        Biomes => &[(StructureStarts, 8)],
        Terrain => &[(StructureStarts, 8), (Biomes, 1)],
        Features => &[(StructureStarts, 8), (Terrain, 1)],
        Light => &[(InitializeLight, 1)],
        Spawn => &[(Biomes, 1)],
        _ => &[],
    })
});

static LOADING_PYRAMID: LazyLock<ChunkPyramidSteps> = LazyLock::new(|| {
    use ChunkStatus::*;
    build_pyramid(|status| match status {
        Light => &[(InitializeLight, 1)],
        _ => &[],
    })
});

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChunkPyramid {
    Generation,
    Loading,
}

impl ChunkPyramid {
    pub fn step_to(self, status: ChunkStatus) -> &'static ChunkStep {
        let pyramid: &'static ChunkPyramidSteps = match self {
            ChunkPyramid::Generation => &GENERATION_PYRAMID,
            ChunkPyramid::Loading => &LOADING_PYRAMID,
        };
        &pyramid.steps[status.index()]
    }

    pub fn accumulated_radius_of(self, target: ChunkStatus, status: ChunkStatus) -> i32 {
        self.step_to(target).get_accumulated_radius_of(status)
    }
}

pub const FULL_CHUNK_LEVEL: i32 = 33;
pub const BLOCK_TICKING_LEVEL: i32 = 32;
pub const ENTITY_TICKING_LEVEL: i32 = 31;
pub const RADIUS_AROUND_FULL_CHUNK: i32 = 11;
pub const MAX_LEVEL: i32 = FULL_CHUNK_LEVEL + RADIUS_AROUND_FULL_CHUNK;

pub fn status_around_full_chunk(distance: i32) -> Option<ChunkStatus> {
    match distance {
        ..=0 => Some(ChunkStatus::Full),
        1..=RADIUS_AROUND_FULL_CHUNK => Some(ChunkStatus::Full.dependency_at(distance)),
        _ => None,
    }
}

pub fn generation_status(level: i32) -> Option<ChunkStatus> {
    status_around_full_chunk(level - FULL_CHUNK_LEVEL)
}

pub fn by_status(status: ChunkStatus) -> i32 {
    FULL_CHUNK_LEVEL + ChunkStatus::Full.accumulated_radius_of(status)
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum FullChunkStatus {
    Inaccessible,
    Full,
    BlockTicking,
    EntityTicking,
}

impl FullChunkStatus {
    pub fn is_or_after(self, status: FullChunkStatus) -> bool {
        self >= status
    }
}

pub fn full_status(level: i32) -> FullChunkStatus {
    if level <= ENTITY_TICKING_LEVEL {
        FullChunkStatus::EntityTicking
    } else if level <= BLOCK_TICKING_LEVEL {
        FullChunkStatus::BlockTicking
    } else if level <= FULL_CHUNK_LEVEL {
        FullChunkStatus::Full
    } else {
        FullChunkStatus::Inaccessible
    }
}

pub fn by_full_status(status: FullChunkStatus) -> i32 {
    match status {
        FullChunkStatus::Inaccessible => MAX_LEVEL,
        FullChunkStatus::Full => FULL_CHUNK_LEVEL,
        FullChunkStatus::BlockTicking => BLOCK_TICKING_LEVEL,
        FullChunkStatus::EntityTicking => ENTITY_TICKING_LEVEL,
    }
}

pub fn is_entity_ticking(level: i32) -> bool {
    level <= ENTITY_TICKING_LEVEL
}

pub fn is_block_ticking(level: i32) -> bool {
    level <= BLOCK_TICKING_LEVEL
}

pub fn is_loaded(level: i32) -> bool {
    level <= MAX_LEVEL
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_match_vanilla() {
        assert_eq!(MAX_LEVEL, 44);
        assert_eq!(generation_status(31), Some(ChunkStatus::Full));
        assert_eq!(generation_status(33), Some(ChunkStatus::Full));
        assert_eq!(generation_status(34), Some(ChunkStatus::InitializeLight));
        assert_eq!(generation_status(35), Some(ChunkStatus::Terrain));
        assert_eq!(generation_status(36), Some(ChunkStatus::Biomes));
        assert_eq!(generation_status(37), Some(ChunkStatus::StructureStarts));
        assert_eq!(generation_status(44), Some(ChunkStatus::StructureStarts));
        assert_eq!(generation_status(45), None);
        assert_eq!(by_status(ChunkStatus::Spawn), 33);
        assert_eq!(by_status(ChunkStatus::Features), 34);
        assert_eq!(by_status(ChunkStatus::StructureReferences), 36);
        assert_eq!(by_status(ChunkStatus::StructureStarts), 44);
        assert_eq!(by_status(ChunkStatus::Empty), 44);
    }

    fn dependency_list(pyramid: ChunkPyramid, target: ChunkStatus) -> Vec<ChunkStatus> {
        let dependencies = &pyramid.step_to(target).accumulated_dependencies;
        (0..dependencies.size() as i32).map(|distance| dependencies.get(distance)).collect()
    }

    #[test]
    fn pyramids_match_vanilla() {
        use ChunkStatus::*;
        const S: ChunkStatus = StructureStarts;
        let generation: [&[ChunkStatus]; 10] = [
            &[],
            &[Empty],
            &[S, S, S, S, S, S, S, S, S],
            &[StructureReferences, S, S, S, S, S, S, S, S],
            &[Biomes, Biomes, S, S, S, S, S, S, S, S],
            &[Terrain, Terrain, Biomes, S, S, S, S, S, S, S, S],
            &[Features, Terrain, Biomes, S, S, S, S, S, S, S, S],
            &[InitializeLight, InitializeLight, Terrain, Biomes, S, S, S, S, S, S, S, S],
            &[Light, InitializeLight, Terrain, Biomes, S, S, S, S, S, S, S, S],
            &[Spawn, InitializeLight, Terrain, Biomes, S, S, S, S, S, S, S, S],
        ];
        let loading: [&[ChunkStatus]; 10] = [
            &[],
            &[Empty],
            &[StructureStarts],
            &[StructureReferences],
            &[Biomes],
            &[Terrain],
            &[Features],
            &[InitializeLight, InitializeLight],
            &[Light, InitializeLight],
            &[Spawn, InitializeLight],
        ];
        for (index, target) in ChunkStatus::ALL.into_iter().enumerate() {
            assert_eq!(dependency_list(ChunkPyramid::Generation, target), generation[index], "generation {target:?}");
            assert_eq!(dependency_list(ChunkPyramid::Loading, target), loading[index], "loading {target:?}");
        }
    }

    #[test]
    fn pyramid_radii() {
        use ChunkStatus::*;
        assert_eq!(Full.accumulated_radius_of(Full), 0);
        assert_eq!(Full.accumulated_radius_of(Light), 0);
        assert_eq!(Full.accumulated_radius_of(InitializeLight), 1);
        assert_eq!(Full.accumulated_radius_of(Terrain), 2);
        assert_eq!(Full.accumulated_radius_of(Empty), 11);
        assert_eq!(Light.accumulated_radius_of(Features), 1);
        assert_eq!(Features.accumulated_radius_of(Terrain), 1);
        assert_eq!(Features.accumulated_radius_of(StructureStarts), 10);
        assert_eq!(StructureReferences.accumulated_radius_of(StructureStarts), 8);
        assert_eq!(InitializeLight.accumulated_radius_of(Empty), 10);
        assert_eq!(Full.dependency_at(1), InitializeLight);
        assert_eq!(Full.dependency_at(2), Terrain);
        assert_eq!(ChunkPyramid::Loading.accumulated_radius_of(Full, Empty), 1);
        assert_eq!(ChunkPyramid::Loading.accumulated_radius_of(Full, InitializeLight), 1);
        assert_eq!(ChunkPyramid::Loading.accumulated_radius_of(Full, Light), 0);
        assert_eq!(ChunkPyramid::Loading.accumulated_radius_of(Light, Empty), 1);
        assert_eq!(ChunkPyramid::Loading.accumulated_radius_of(Features, Empty), 0);
        assert_eq!(ChunkPyramid::Loading.step_to(Full).accumulated_dependencies.get_radius(), 1);
        assert_eq!(ChunkPyramid::Loading.step_to(Empty).accumulated_dependencies.get_radius(), 0);
    }
}
