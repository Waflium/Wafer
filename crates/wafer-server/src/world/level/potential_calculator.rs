use wafer_core::BlockPos;

#[derive(Default)]
pub struct PotentialCalculator {
    charges: Vec<PointCharge>,
}

impl PotentialCalculator {
    pub fn add_charge(&mut self, pos: BlockPos, charge: f64) {
        if charge != 0.0 {
            self.charges.push(PointCharge { pos, charge });
        }
    }

    pub fn get_potential_energy_change(&self, pos: BlockPos, charge: f64) -> f64 {
        if charge == 0.0 {
            return 0.0;
        }
        let mut potential_change = 0.0;
        for point in &self.charges {
            potential_change += point.get_potential_change(pos);
        }
        potential_change * charge
    }
}

struct PointCharge {
    pos: BlockPos,
    charge: f64,
}

impl PointCharge {
    fn get_potential_change(&self, pos: BlockPos) -> f64 {
        let dist_sqr = self.pos.dist_sqr(pos);
        if dist_sqr == 0.0 { f64::INFINITY } else { self.charge / dist_sqr.sqrt() }
    }
}
