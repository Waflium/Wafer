use std::sync::LazyLock;

use wafer_core::fastutil::OpenHashMap;
use wafer_core::random::RandomSource;
use wafer_core::{AxisDirection, BlockPos, Direction, Axis};
use wafer_data::{Tag, block_tags};
use wafer_protocol::nbt::Nbt;

use super::multiface_block;
use super::sculk_behaviour::{self, SculkBehaviour};
use crate::server::level::SoundSource;
use crate::server::level::events::PARTICLES_SCULK_CHARGE;
use crate::world::level::world_gen_level::WorldGenLevel;
use crate::util;

pub const MAX_CHARGE: i32 = 1000;
const MAX_CURSORS: usize = 32;

#[derive(Clone)]
pub struct SculkSpreader {
    pub is_world_generation: bool,
    pub replaceable_blocks: &'static Tag,
    pub growth_spawn_cost: i32,
    pub no_growth_radius: i32,
    pub charge_decay_rate: i32,
    pub additional_decay_rate: i32,
    cursors: Vec<ChargeCursor>,
}

impl SculkSpreader {
    pub fn create_level_spreader() -> SculkSpreader {
        SculkSpreader {
            is_world_generation: false,
            replaceable_blocks: &block_tags::SCULK_REPLACEABLE,
            growth_spawn_cost: 10,
            no_growth_radius: 4,
            charge_decay_rate: 10,
            additional_decay_rate: 5,
            cursors: Vec::new(),
        }
    }

    pub fn create_world_gen_spreader() -> SculkSpreader {
        SculkSpreader {
            is_world_generation: true,
            replaceable_blocks: &block_tags::SCULK_REPLACEABLE_WORLD_GEN,
            growth_spawn_cost: 50,
            no_growth_radius: 1,
            charge_decay_rate: 5,
            additional_decay_rate: 10,
            cursors: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.cursors.clear();
    }

    pub fn load(&mut self, input: &Nbt) {
        self.cursors.clear();
        let Some(Nbt::List(cursors)) = input.get("cursors") else { return };
        if cursors.len() > MAX_CURSORS {
            return;
        }
        for cursor in cursors.iter().filter_map(ChargeCursor::from_nbt) {
            self.add_cursor(cursor);
        }
    }

    pub fn save(&self, output: &mut Vec<(String, Nbt)>) {
        output.push(("cursors".into(), Nbt::List(self.cursors.iter().map(ChargeCursor::to_nbt).collect())));
    }

    pub fn add_cursors(&mut self, start_pos: BlockPos, mut charge: i32) {
        while charge > 0 {
            let current_charge = charge.min(MAX_CHARGE);
            self.add_cursor(ChargeCursor::new(start_pos, current_charge));
            charge -= current_charge;
        }
    }

    fn add_cursor(&mut self, cursor: ChargeCursor) {
        if self.cursors.len() < MAX_CURSORS {
            self.cursors.push(cursor);
        }
    }

    pub fn update_cursors<R: RandomSource>(&mut self, level: &mut impl WorldGenLevel, origin_pos: BlockPos, random: &mut R, spread_veins: bool) {
        if self.cursors.is_empty() {
            return;
        }
        let mut cursors = std::mem::take(&mut self.cursors);
        let mut processed_cursors: Vec<ChargeCursor> = Vec::with_capacity(cursors.len());
        let mut mergeable_cursors: Vec<(BlockPos, usize)> = Vec::new();
        let mut charge_map: OpenHashMap<BlockPos, i32> = OpenHashMap::new();
        for mut cursor in cursors.drain(..) {
            if cursor.is_pos_unreasonable(origin_pos) {
                continue;
            }
            cursor.update(level, origin_pos, random, self, spread_veins);
            if cursor.charge <= 0 {
                if let Some(level) = level.as_server_level() {
                    level.level_event(None, PARTICLES_SCULK_CHARGE, cursor.pos, 0);
                }
                continue;
            }
            let pos = cursor.pos;
            *charge_map.get_or_insert_with(pos, || 0) += cursor.charge;
            match mergeable_cursors.iter_mut().find(|(p, _)| *p == pos) {
                None => {
                    mergeable_cursors.push((pos, processed_cursors.len()));
                    processed_cursors.push(cursor);
                }
                Some(entry) => {
                    let existing_charge = processed_cursors[entry.1].charge;
                    if !self.is_world_generation && cursor.charge + existing_charge <= MAX_CHARGE {
                        let existing = &mut processed_cursors[entry.1];
                        existing.charge += cursor.charge;
                        existing.update_delay = existing.update_delay.min(cursor.update_delay);
                    } else {
                        let less = cursor.charge < existing_charge;
                        processed_cursors.push(cursor);
                        if less {
                            entry.1 = processed_cursors.len() - 1;
                        }
                    }
                }
            }
        }
        if let Some(level) = level.as_server_level() {
            for (pos, &charge) in charge_map.iter() {
                let faces = mergeable_cursors.iter().find(|(p, _)| *p == pos).and_then(|&(_, index)| processed_cursors[index].facings);
                if charge > 0
                    && let Some(faces) = faces
                {
                    let num_particles = ((charge as f64).ln_1p() / 2.3f32 as f64) as i32 + 1;
                    level.level_event(None, PARTICLES_SCULK_CHARGE, pos, (num_particles << 6) + faces as i32);
                }
            }
        }
        self.cursors = processed_cursors;
    }
}

static NON_CORNER_NEIGHBOURS: LazyLock<Vec<(i32, i32, i32)>> = LazyLock::new(|| {
    let mut list = Vec::with_capacity(18);
    for z in -1..=1 {
        for y in -1..=1 {
            for x in -1..=1 {
                if (x == 0 || y == 0 || z == 0) && (x, y, z) != (0, 0, 0) {
                    list.push((x, y, z));
                }
            }
        }
    }
    list
});

#[derive(Clone)]
pub struct ChargeCursor {
    pub pos: BlockPos,
    pub charge: i32,
    update_delay: i32,
    pub decay_delay: i32,
    facings: Option<u8>,
}

impl ChargeCursor {
    pub fn new(pos: BlockPos, charge: i32) -> ChargeCursor {
        ChargeCursor { pos, charge, update_delay: 0, decay_delay: 1, facings: None }
    }

    fn from_nbt(nbt: &Nbt) -> Option<ChargeCursor> {
        let pos = match nbt.get("pos")? {
            Nbt::IntArray(p) if p.len() == 3 => BlockPos::new(p[0], p[1], p[2]),
            _ => return None,
        };
        let int = |name: &str, default: i64, max: i64| match nbt.get(name) {
            None => Some(default as i32),
            Some(value) => value.as_i64().filter(|value| (0..=max).contains(value)).map(|value| value as i32),
        };
        let charge = int("charge", 0, MAX_CHARGE as i64)?;
        let decay_delay = int("decay_delay", 1, 1)?;
        let update_delay = int("update_delay", 0, i32::MAX as i64)?;
        let facings = match nbt.get("facings") {
            Some(Nbt::List(names)) => names.iter().map(|name| name.as_str().and_then(Direction::by_name)).collect::<Option<Vec<Direction>>>().map(|directions| directions.into_iter().fold(0u8, |faces, direction| faces | 1 << direction.ordinal())),
            _ => None,
        };
        Some(ChargeCursor { pos, charge, update_delay, decay_delay, facings })
    }

    fn to_nbt(&self) -> Nbt {
        let mut fields = vec![("pos".to_string(), Nbt::IntArray(vec![self.pos.x, self.pos.y, self.pos.z]))];
        if self.charge != 0 {
            fields.push(("charge".into(), Nbt::Int(self.charge)));
        }
        if self.decay_delay != 1 {
            fields.push(("decay_delay".into(), Nbt::Int(self.decay_delay)));
        }
        if self.update_delay != 0 {
            fields.push(("update_delay".into(), Nbt::Int(self.update_delay)));
        }
        if let Some(faces) = self.facings {
            let names = Direction::VALUES.into_iter().filter(|direction| faces & 1 << direction.ordinal() != 0).map(|direction| Nbt::String(direction.name().into())).collect();
            fields.push(("facings".into(), Nbt::List(names)));
        }
        Nbt::Compound(fields)
    }

    fn is_pos_unreasonable(&self, origin_pos: BlockPos) -> bool {
        (self.pos.x - origin_pos.x).abs().max((self.pos.y - origin_pos.y).abs()).max((self.pos.z - origin_pos.z).abs()) > 1024
    }

    fn should_update(&self, level: &mut impl WorldGenLevel, pos: BlockPos, is_world_gen: bool) -> bool {
        if self.charge <= 0 {
            false
        } else if is_world_gen {
            true
        } else {
            level.as_server_level().is_some_and(|level| level.in_block_ticking_range(pos.chunk()))
        }
    }

    fn update<R: RandomSource>(&mut self, level: &mut impl WorldGenLevel, origin_pos: BlockPos, random: &mut R, spreader: &SculkSpreader, spread_veins: bool) {
        if !self.should_update(level, origin_pos, spreader.is_world_generation) {
            return;
        }
        if self.update_delay > 0 {
            self.update_delay -= 1;
            return;
        }
        let mut current_state = level.get_block_state(self.pos);
        let mut sculk_behaviour = SculkBehaviour::get_block_behaviour(current_state);
        if spread_veins && sculk_behaviour.attempt_spread_vein(level, self.pos, current_state, self.facings, spreader.is_world_generation) {
            if sculk_behaviour.can_change_block_state_on_spread() {
                current_state = level.get_block_state(self.pos);
                sculk_behaviour = SculkBehaviour::get_block_behaviour(current_state);
            }
            level.play_sound(self.pos, crate::world::entity::sound_event("minecraft:block.sculk.spread"), SoundSource::Blocks, 1.0, 1.0);
        }
        self.charge = sculk_behaviour.attempt_use_charge(self, level, origin_pos, random, spreader, spread_veins);
        if self.charge <= 0 {
            sculk_behaviour.on_discharged(level, current_state, self.pos);
            return;
        }
        match get_valid_movement_pos(level, self.pos, random, origin_pos, spreader) {
            Some(transfer_pos) => {
                sculk_behaviour.on_discharged(level, current_state, self.pos);
                self.pos = transfer_pos;
                current_state = level.get_block_state(transfer_pos);
            }
            None if spreader.is_world_generation => {
                sculk_behaviour.on_discharged(level, current_state, self.pos);
                self.charge = 0;
                return;
            }
            None => {}
        }
        if SculkBehaviour::of(current_state.block()).is_some() {
            self.facings = Some(available_faces(current_state));
        }
        self.decay_delay = sculk_behaviour.update_decay_delay(self.decay_delay);
        self.update_delay = sculk_behaviour.get_sculk_spread_delay();
    }
}

fn available_faces(state: wafer_data::BlockState) -> u8 {
    if !state.block().is_instance_of("MultifaceBlock") {
        return 0;
    }
    Direction::VALUES.into_iter().filter(|&direction| multiface_block::has_face(state, direction)).fold(0, |faces, direction| faces | 1 << direction.ordinal())
}

fn get_valid_movement_pos(level: &mut impl WorldGenLevel, pos: BlockPos, random: &mut impl RandomSource, origin_pos: BlockPos, spreader: &SculkSpreader) -> Option<BlockPos> {
    let mut sculk_position = pos;
    let mut offsets = NON_CORNER_NEIGHBOURS.clone();
    util::shuffle(&mut offsets, random);
    for (x, y, z) in offsets {
        let neighbour = pos.offset(x, y, z);
        if can_move_to_pos(origin_pos, neighbour, spreader) {
            let transferee = level.get_block_state(neighbour);
            if SculkBehaviour::of(transferee.block()).is_some() && is_movement_unobstructed(level, pos, neighbour) {
                sculk_position = neighbour;
                if sculk_behaviour::has_substrate_access(level, transferee, neighbour) {
                    break;
                }
            }
        }
    }
    (sculk_position != pos).then_some(sculk_position)
}

fn can_move_to_pos(origin: BlockPos, target: BlockPos, spreader: &SculkSpreader) -> bool {
    if !spreader.is_world_generation {
        return true;
    }
    let dx = origin.x - target.x;
    let dz = origin.z - target.z;
    dx * dx + dz * dz <= 144
}

fn is_movement_unobstructed(level: &mut impl WorldGenLevel, from: BlockPos, to: BlockPos) -> bool {
    if from.dist_manhattan(to) == 1 {
        return true;
    }
    let (dx, dy, dz) = (to.x - from.x, to.y - from.y, to.z - from.z);
    let sign = |delta: i32| if delta < 0 { AxisDirection::Negative } else { AxisDirection::Positive };
    let direction_x = Direction::from_axis_and_direction(Axis::X, sign(dx));
    let direction_y = Direction::from_axis_and_direction(Axis::Y, sign(dy));
    let direction_z = Direction::from_axis_and_direction(Axis::Z, sign(dz));
    if dx == 0 {
        is_unobstructed(level, from, direction_y) || is_unobstructed(level, from, direction_z)
    } else if dy == 0 {
        is_unobstructed(level, from, direction_x) || is_unobstructed(level, from, direction_z)
    } else {
        is_unobstructed(level, from, direction_x) || is_unobstructed(level, from, direction_y)
    }
}

fn is_unobstructed(level: &mut impl WorldGenLevel, from: BlockPos, direction: Direction) -> bool {
    let test_pos = from.relative(direction);
    !level.get_block_state(test_pos).is_face_sturdy_full(direction.opposite())
}
