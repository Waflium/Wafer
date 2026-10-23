use std::sync::OnceLock;

use crate::datafixers::data_fixer_upper::{DataFixerBuilder, DataFixerUpper};
use crate::util::datafix::fixes::entity_equipment_to_armor_and_hand_fix::EntityEquipmentToArmorAndHandFix;
use crate::util::datafix::schemas::v99::V99;
use crate::util::datafix::schemas::v100::V100;

pub fn get_data_fixer() -> &'static DataFixerUpper {
    static DATA_FIXER: OnceLock<DataFixerUpper> = OnceLock::new();
    DATA_FIXER.get_or_init(|| {
        let mut builder = DataFixerBuilder::new(wafer_data::WORLD_VERSION);
        add_fixers(&mut builder);
        builder.build()
    })
}

fn add_fixers(fixer_upper: &mut DataFixerBuilder) {
    fixer_upper.add_schema(99, &V99);
    let v100 = fixer_upper.add_schema(100, &V100);
    fixer_upper.add_fixer(EntityEquipmentToArmorAndHandFix::new(&v100));
}
