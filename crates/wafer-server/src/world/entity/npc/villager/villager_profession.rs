use wafer_data::{Block, Item, blocks, items, registry};

use crate::network::chat::Component;
use crate::world::entity::ai::village::poi::poi_types::PoiType;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum JobSite {
    None,
    Is(PoiType),
    AllAcquirableJobs,
}

impl JobSite {
    pub fn test(self, poi_type: PoiType) -> bool {
        match self {
            JobSite::None => false,
            JobSite::Is(job_site) => poi_type == job_site,
            JobSite::AllAcquirableJobs => poi_type.is_acquirable_job_site(),
        }
    }
}

pub struct VillagerProfession {
    pub key: &'static str,
    pub held_job_site: JobSite,
    pub acquirable_job_site: JobSite,
    pub requested_items: &'static [Item],
    pub secondary_poi: &'static [Block],
    pub work_sound: Option<&'static str>,
    pub trade_sets_by_level: [Option<&'static str>; 5],
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ProfessionId(pub i32);

impl ProfessionId {
    pub const NONE: ProfessionId = ProfessionId(0);
    pub const ARMORER: ProfessionId = ProfessionId(1);
    pub const BUTCHER: ProfessionId = ProfessionId(2);
    pub const CARTOGRAPHER: ProfessionId = ProfessionId(3);
    pub const CLERIC: ProfessionId = ProfessionId(4);
    pub const FARMER: ProfessionId = ProfessionId(5);
    pub const FISHERMAN: ProfessionId = ProfessionId(6);
    pub const FLETCHER: ProfessionId = ProfessionId(7);
    pub const LEATHERWORKER: ProfessionId = ProfessionId(8);
    pub const LIBRARIAN: ProfessionId = ProfessionId(9);
    pub const MASON: ProfessionId = ProfessionId(10);
    pub const NITWIT: ProfessionId = ProfessionId(11);
    pub const SHEPHERD: ProfessionId = ProfessionId(12);
    pub const TOOLSMITH: ProfessionId = ProfessionId(13);
    pub const WEAPONSMITH: ProfessionId = ProfessionId(14);

    pub fn value(self) -> &'static VillagerProfession {
        &PROFESSIONS[self.0 as usize]
    }

    pub fn name(self) -> &'static str {
        registry::VILLAGER_PROFESSION[self.0 as usize]
    }

    pub fn by_name(name: &str) -> Option<ProfessionId> {
        registry::VILLAGER_PROFESSION.iter().position(|&entry| entry == name).map(|id| ProfessionId(id as i32))
    }

    pub fn by_id(id: i32) -> Option<ProfessionId> {
        (0..registry::VILLAGER_PROFESSION.len() as i32).contains(&id).then_some(ProfessionId(id))
    }

    pub fn list_elements() -> impl Iterator<Item = ProfessionId> {
        (0..registry::VILLAGER_PROFESSION.len() as i32).map(ProfessionId)
    }
}

pub const ALL_ACQUIRABLE_JOBS: JobSite = JobSite::AllAcquirableJobs;

impl VillagerProfession {
    pub fn name(&self) -> Component {
        Component::translatable(&format!("entity.minecraft.villager.{}", self.key.trim_start_matches("minecraft:")))
    }

    pub fn get_trades(&self, level: i32) -> Option<&'static str> {
        if (1..=5).contains(&level) { self.trade_sets_by_level[level as usize - 1] } else { None }
    }
}

const fn trades(levels: [&'static str; 5]) -> [Option<&'static str>; 5] {
    [Some(levels[0]), Some(levels[1]), Some(levels[2]), Some(levels[3]), Some(levels[4])]
}

const fn job(key: &'static str, job_site: PoiType, work_sound: &'static str, trade_sets_by_level: [Option<&'static str>; 5]) -> VillagerProfession {
    VillagerProfession {
        key,
        held_job_site: JobSite::Is(job_site),
        acquirable_job_site: JobSite::Is(job_site),
        requested_items: &[],
        secondary_poi: &[],
        work_sound: Some(work_sound),
        trade_sets_by_level,
    }
}

static PROFESSIONS: [VillagerProfession; 15] = [
    VillagerProfession {
        key: "minecraft:none",
        held_job_site: JobSite::None,
        acquirable_job_site: ALL_ACQUIRABLE_JOBS,
        requested_items: &[],
        secondary_poi: &[],
        work_sound: None,
        trade_sets_by_level: [None; 5],
    },
    job(
        "minecraft:armorer",
        PoiType::Armorer,
        "minecraft:entity.villager.work_armorer",
        trades(["minecraft:armorer/level_1", "minecraft:armorer/level_2", "minecraft:armorer/level_3", "minecraft:armorer/level_4", "minecraft:armorer/level_5"]),
    ),
    job(
        "minecraft:butcher",
        PoiType::Butcher,
        "minecraft:entity.villager.work_butcher",
        trades(["minecraft:butcher/level_1", "minecraft:butcher/level_2", "minecraft:butcher/level_3", "minecraft:butcher/level_4", "minecraft:butcher/level_5"]),
    ),
    job(
        "minecraft:cartographer",
        PoiType::Cartographer,
        "minecraft:entity.villager.work_cartographer",
        trades(["minecraft:cartographer/level_1", "minecraft:cartographer/level_2", "minecraft:cartographer/level_3", "minecraft:cartographer/level_4", "minecraft:cartographer/level_5"]),
    ),
    job(
        "minecraft:cleric",
        PoiType::Cleric,
        "minecraft:entity.villager.work_cleric",
        trades(["minecraft:cleric/level_1", "minecraft:cleric/level_2", "minecraft:cleric/level_3", "minecraft:cleric/level_4", "minecraft:cleric/level_5"]),
    ),
    VillagerProfession {
        key: "minecraft:farmer",
        held_job_site: JobSite::Is(PoiType::Farmer),
        acquirable_job_site: JobSite::Is(PoiType::Farmer),
        requested_items: &[items::WHEAT, items::WHEAT_SEEDS, items::BEETROOT_SEEDS, items::BONE_MEAL],
        secondary_poi: &[blocks::FARMLAND],
        work_sound: Some("minecraft:entity.villager.work_farmer"),
        trade_sets_by_level: trades(["minecraft:farmer/level_1", "minecraft:farmer/level_2", "minecraft:farmer/level_3", "minecraft:farmer/level_4", "minecraft:farmer/level_5"]),
    },
    job(
        "minecraft:fisherman",
        PoiType::Fisherman,
        "minecraft:entity.villager.work_fisherman",
        trades(["minecraft:fisherman/level_1", "minecraft:fisherman/level_2", "minecraft:fisherman/level_3", "minecraft:fisherman/level_4", "minecraft:fisherman/level_5"]),
    ),
    job(
        "minecraft:fletcher",
        PoiType::Fletcher,
        "minecraft:entity.villager.work_fletcher",
        trades(["minecraft:fletcher/level_1", "minecraft:fletcher/level_2", "minecraft:fletcher/level_3", "minecraft:fletcher/level_4", "minecraft:fletcher/level_5"]),
    ),
    job(
        "minecraft:leatherworker",
        PoiType::Leatherworker,
        "minecraft:entity.villager.work_leatherworker",
        trades(["minecraft:leatherworker/level_1", "minecraft:leatherworker/level_2", "minecraft:leatherworker/level_3", "minecraft:leatherworker/level_4", "minecraft:leatherworker/level_5"]),
    ),
    job(
        "minecraft:librarian",
        PoiType::Librarian,
        "minecraft:entity.villager.work_librarian",
        trades(["minecraft:librarian/level_1", "minecraft:librarian/level_2", "minecraft:librarian/level_3", "minecraft:librarian/level_4", "minecraft:librarian/level_5"]),
    ),
    job(
        "minecraft:mason",
        PoiType::Mason,
        "minecraft:entity.villager.work_mason",
        trades(["minecraft:mason/level_1", "minecraft:mason/level_2", "minecraft:mason/level_3", "minecraft:mason/level_4", "minecraft:mason/level_5"]),
    ),
    VillagerProfession {
        key: "minecraft:nitwit",
        held_job_site: JobSite::None,
        acquirable_job_site: JobSite::None,
        requested_items: &[],
        secondary_poi: &[],
        work_sound: None,
        trade_sets_by_level: [None; 5],
    },
    job(
        "minecraft:shepherd",
        PoiType::Shepherd,
        "minecraft:entity.villager.work_shepherd",
        trades(["minecraft:shepherd/level_1", "minecraft:shepherd/level_2", "minecraft:shepherd/level_3", "minecraft:shepherd/level_4", "minecraft:shepherd/level_5"]),
    ),
    job(
        "minecraft:toolsmith",
        PoiType::Toolsmith,
        "minecraft:entity.villager.work_toolsmith",
        trades(["minecraft:toolsmith/level_1", "minecraft:toolsmith/level_2", "minecraft:toolsmith/level_3", "minecraft:toolsmith/level_4", "minecraft:toolsmith/level_5"]),
    ),
    job(
        "minecraft:weaponsmith",
        PoiType::Weaponsmith,
        "minecraft:entity.villager.work_weaponsmith",
        trades(["minecraft:weaponsmith/level_1", "minecraft:weaponsmith/level_2", "minecraft:weaponsmith/level_3", "minecraft:weaponsmith/level_4", "minecraft:weaponsmith/level_5"]),
    ),
];
