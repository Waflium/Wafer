use std::sync::LazyLock;

use wafer_core::random::RandomSource;
use wafer_core::{Axis, AxisDirection, Direction};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SideBias {
    Left,
    Right,
}

impl SideBias {
    pub fn get_opposite(self) -> SideBias {
        if self == SideBias::Left { SideBias::Right } else { SideBias::Left }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Orientation(u8);

struct OrientationData {
    up: Direction,
    front: Direction,
    side: Direction,
    side_bias: SideBias,
    neighbors: [Direction; 6],
    horizontal_neighbors: [Direction; 4],
    vertical_neighbors: [Direction; 2],
}

static ORIENTATIONS: LazyLock<Vec<Option<OrientationData>>> = LazyLock::new(|| {
    let mut orientations: Vec<Option<OrientationData>> = (0..48).map(|_| None).collect();
    for up in Direction::VALUES {
        for front in Direction::VALUES.into_iter().filter(|front| front.axis() != up.axis()) {
            for side_bias in [SideBias::Left, SideBias::Right] {
                orientations[generate_index(up, front, side_bias) as usize] = Some(OrientationData::new(up, front, side_bias));
            }
        }
    }
    orientations
});

impl OrientationData {
    fn new(up: Direction, front: Direction, side_bias: SideBias) -> OrientationData {
        let (fx, fy, fz) = (front.step_x(), front.step_y(), front.step_z());
        let (ux, uy, uz) = (up.step_x(), up.step_y(), up.step_z());
        let side = Direction::nearest(fy * uz - fz * uy, fz * ux - fx * uz, fx * uy - fy * ux).expect("a side direction");
        let side = if side_bias == SideBias::Right { side } else { side.opposite() };
        let neighbors = [front.opposite(), front, side, side.opposite(), up.opposite(), up];
        let mut horizontal_neighbors = [Direction::Down; 4];
        let mut vertical_neighbors = [Direction::Down; 2];
        let (mut h, mut v) = (0, 0);
        for direction in neighbors {
            if direction.axis() != up.axis() {
                horizontal_neighbors[h] = direction;
                h += 1;
            } else {
                vertical_neighbors[v] = direction;
                v += 1;
            }
        }
        OrientationData { up, front, side, side_bias, neighbors, horizontal_neighbors, vertical_neighbors }
    }
}

fn generate_index(up: Direction, front: Direction, side_bias: SideBias) -> u8 {
    assert!(up.axis() != front.axis(), "Up-vector and front-vector can not be on the same axis");
    let front_axis_key = if up.axis() == Axis::Y { (front.axis() == Axis::X) as u8 } else { (front.axis() == Axis::Y) as u8 };
    let front_key = front_axis_key << 1 | (front.axis_direction() == AxisDirection::Negative) as u8;
    (((up.ordinal() as u8) << 2) + front_key) << 1 | side_bias as u8
}

impl Orientation {
    fn data(self) -> &'static OrientationData {
        ORIENTATIONS[self.0 as usize].as_ref().expect("an orientation")
    }

    pub fn of(up: Direction, front: Direction, side_bias: SideBias) -> Orientation {
        Orientation(generate_index(up, front, side_bias))
    }

    pub fn with_up(self, up: Direction) -> Orientation {
        let data = self.data();
        let front = if up == data.front {
            data.up.opposite()
        } else if up == data.front.opposite() {
            data.up
        } else {
            data.front
        };
        Orientation::of(up, front, data.side_bias)
    }

    pub fn with_front(self, front: Direction) -> Orientation {
        let data = self.data();
        let up = if front == data.up {
            data.front.opposite()
        } else if front == data.up.opposite() {
            data.front
        } else {
            data.up
        };
        Orientation::of(up, front, data.side_bias)
    }

    pub fn with_front_preserve_up(self, front: Direction) -> Orientation {
        if front.axis() == self.data().up.axis() { self } else { self.with_front(front) }
    }

    pub fn with_front_adjust_side_bias(self, front: Direction) -> Orientation {
        let with_front = self.with_front(front);
        if self.data().front == with_front.data().side { with_front.with_mirror() } else { with_front }
    }

    pub fn with_side_bias(self, side_bias: SideBias) -> Orientation {
        let data = self.data();
        Orientation::of(data.up, data.front, side_bias)
    }

    pub fn with_mirror(self) -> Orientation {
        self.with_side_bias(self.data().side_bias.get_opposite())
    }

    pub fn get_front(self) -> Direction {
        self.data().front
    }

    pub fn get_up(self) -> Direction {
        self.data().up
    }

    pub fn get_side(self) -> Direction {
        self.data().side
    }

    pub fn get_side_bias(self) -> SideBias {
        self.data().side_bias
    }

    pub fn get_directions(self) -> &'static [Direction; 6] {
        &self.data().neighbors
    }

    pub fn get_horizontal_directions(self) -> &'static [Direction; 4] {
        &self.data().horizontal_neighbors
    }

    pub fn get_vertical_directions(self) -> &'static [Direction; 2] {
        &self.data().vertical_neighbors
    }

    pub fn get_index(self) -> i32 {
        self.0 as i32
    }

    pub fn from_index(index: i32) -> Orientation {
        Orientation(index as u8)
    }

    pub fn random(random: &mut impl RandomSource) -> Orientation {
        Orientation(random.next_int_bounded(48) as u8)
    }
}
