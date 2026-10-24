pub mod abstract_furnace_block_entity;
mod barrel_block_entity;
pub mod banner_block_entity;
pub mod banner_pattern;
pub mod base_container_block_entity;
pub mod beacon_block_entity;
pub mod beehive_block_entity;
pub mod bell_block_entity;
pub mod brewing_stand_block_entity;
pub mod brushable_block_entity;
pub mod campfire_block_entity;
pub mod chest_block_entity;
pub mod chiseled_book_shelf_block_entity;
pub mod command_block_entity;
pub mod comparator_block_entity;
pub mod conduit_block_entity;
pub mod creaking_heart_block_entity;
pub mod container_openers_counter;
pub mod crafter_block_entity;
pub mod decorated_pot_block_entity;
mod dispenser_block_entity;
pub mod decorated_pot_pattern;
pub mod enchanting_table_block_entity;
pub mod ender_chest_block_entity;
pub mod hopper;
pub mod hopper_block_entity;
pub mod jigsaw_block_entity;
pub mod jukebox_block_entity;
pub mod lectern_block_entity;
pub mod piston_moving_block_entity;
pub mod potent_sulfur_block_entity;
#[cfg(test)]
mod oracle_data;
#[cfg(test)]
mod oracle_tests;
pub mod sculk_catalyst_block_entity;
pub mod sculk_sensor_block_entity;
pub mod sculk_shrieker_block_entity;
pub mod shelf_block_entity;
pub mod sign_block_entity;
pub mod sign_text;
pub mod the_end_gateway_block_entity;
pub mod trial_spawner_block_entity;
pub mod shulker_box_block_entity;
pub mod skull_block_entity;
pub mod structure_block_entity;
pub mod test_block_entity;
pub mod test_instance_block_entity;
pub mod spawner_block_entity;
pub mod trialspawner;
pub mod vault;
#[cfg(test)]
mod tests;

use std::sync::atomic::{AtomicU64, Ordering};

use wafer_core::{BlockPos, ChunkPos};
use wafer_data::{BlockEntityType, BlockState, block_entity_types as types};
use wafer_protocol::nbt::Nbt;

pub use abstract_furnace_block_entity::AbstractFurnaceBlockEntity;
pub use barrel_block_entity::BarrelBlockEntity;
pub use base_container_block_entity::BaseContainerBlockEntity;
pub use beehive_block_entity::BeehiveBlockEntity;
pub use brushable_block_entity::BrushableBlockEntity;
pub use chest_block_entity::ChestBlockEntity;
pub use dispenser_block_entity::DispenserBlockEntity;
pub use hopper_block_entity::HopperBlockEntity;
pub use shulker_box_block_entity::{AnimationStatus, ShulkerBoxBlockEntity};

use crate::util::problem_reporter::PathElement;
use crate::world::item::component::{self as dc, DataComponentMap, DataComponentPatch, DataComponentType, PatchedDataComponentMap};
use crate::world::item::ItemStack;
use crate::world::item::crafting::recipe_type::RecipeType;
use crate::world::level::storage::tag_value_input;

pub const BLOCK_ENTITY_LOGGER: &str = "net.minecraft.world.level.block.entity.BlockEntity";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BlockEntityTicker {
    Furnace,
    Hopper,
    ShulkerBox,
    DaylightDetector,
    Jukebox,
    Conduit,
    CreakingHeart,
    SculkSensor,
    SculkCatalyst,
    SculkShrieker,
    Beacon,
    BrewingStand,
    CampfireCook,
    CampfireCooldown,
    Bell,
    Sign,
    Vault,
    TrialSpawner,
    PotentSulfurWet,
    PotentSulfurDormant,
    PotentSulfurErupting,
    PotentSulfurContinuous,
    Crafter,
    EndGateway,
    Spawner,
    Piston,
    Beehive,
}

enum UpdateTag {
    None,
    CustomOnly,
    WithoutMetadata,
}

#[derive(Clone)]
pub enum BlockEntityKind {
    Chest(ChestBlockEntity),
    Barrel(BarrelBlockEntity),
    ShulkerBox(ShulkerBoxBlockEntity),
    Furnace(AbstractFurnaceBlockEntity),
    Hopper(HopperBlockEntity),
    Dispenser(DispenserBlockEntity),
    Crafter(crafter_block_entity::CrafterBlockEntity),
    Beehive(BeehiveBlockEntity),
    Spawner(spawner_block_entity::SpawnerBlockEntity),
    Brushable(BrushableBlockEntity),
    Comparator(comparator_block_entity::ComparatorBlockEntity),
    Jukebox(jukebox_block_entity::JukeboxBlockEntity),
    Conduit(conduit_block_entity::ConduitBlockEntity),
    CreakingHeart(creaking_heart_block_entity::CreakingHeartBlockEntity),
    SculkSensor(sculk_sensor_block_entity::SculkSensorBlockEntity),
    SculkCatalyst(sculk_catalyst_block_entity::SculkCatalystBlockEntity),
    SculkShrieker(sculk_shrieker_block_entity::SculkShriekerBlockEntity),
    Beacon(beacon_block_entity::BeaconBlockEntity),
    EnchantingTable(enchanting_table_block_entity::EnchantingTableBlockEntity),
    Campfire(campfire_block_entity::CampfireBlockEntity),
    Lectern(lectern_block_entity::LecternBlockEntity),
    ChiseledBookShelf(chiseled_book_shelf_block_entity::ChiseledBookShelfBlockEntity),
    Shelf(shelf_block_entity::ShelfBlockEntity),
    DecoratedPot(decorated_pot_block_entity::DecoratedPotBlockEntity),
    Banner(banner_block_entity::BannerBlockEntity),
    Skull(skull_block_entity::SkullBlockEntity),
    Bell(bell_block_entity::BellBlockEntity),
    Sign(sign_block_entity::SignBlockEntity),
    EnderChest(ender_chest_block_entity::EnderChestBlockEntity),
    Vault(vault::vault_block_entity::VaultBlockEntity),
    TrialSpawner(trial_spawner_block_entity::TrialSpawnerBlockEntity),
    PotentSulfur(potent_sulfur_block_entity::PotentSulfurBlockEntity),
    Empty,
    BrewingStand(brewing_stand_block_entity::BrewingStandBlockEntity),
    EndGateway(the_end_gateway_block_entity::TheEndGatewayBlockEntity),
    Jigsaw(jigsaw_block_entity::JigsawBlockEntity),
    PistonMoving(piston_moving_block_entity::PistonMovingBlockEntity),
    CommandBlock(command_block_entity::CommandBlockEntity),
    StructureBlock(structure_block_entity::StructureBlockEntity),
    TestBlock(test_block_entity::TestBlockEntity),
    TestInstanceBlock(test_instance_block_entity::TestInstanceBlockEntity),
    Other(Vec<(String, Nbt)>),
}

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone)]
pub struct BlockEntity {
    ty: BlockEntityType,
    pos: BlockPos,
    block_state: BlockState,
    remove: bool,
    components: DataComponentMap,
    id: u64,
    pub kind: BlockEntityKind,
}

impl BlockEntity {
    pub fn new(ty: BlockEntityType, pos: BlockPos, state: BlockState) -> Option<BlockEntity> {
        if !ty.is_valid(state) {
            return None;
        }
        let kind = match ty {
            types::CHEST | types::TRAPPED_CHEST => BlockEntityKind::Chest(ChestBlockEntity::default()),
            types::BARREL => BlockEntityKind::Barrel(BarrelBlockEntity::default()),
            types::SHULKER_BOX => BlockEntityKind::ShulkerBox(ShulkerBoxBlockEntity::default()),
            types::FURNACE => BlockEntityKind::Furnace(AbstractFurnaceBlockEntity::new(RecipeType::Smelting)),
            types::BLAST_FURNACE => BlockEntityKind::Furnace(AbstractFurnaceBlockEntity::new(RecipeType::Blasting)),
            types::SMOKER => BlockEntityKind::Furnace(AbstractFurnaceBlockEntity::new(RecipeType::Smoking)),
            types::HOPPER => BlockEntityKind::Hopper(HopperBlockEntity::new(state)),
            types::DISPENSER | types::DROPPER => BlockEntityKind::Dispenser(DispenserBlockEntity::default()),
            types::CRAFTER => BlockEntityKind::Crafter(crafter_block_entity::CrafterBlockEntity::new(state)),
            types::BEEHIVE => BlockEntityKind::Beehive(BeehiveBlockEntity::default()),
            types::MOB_SPAWNER => BlockEntityKind::Spawner(spawner_block_entity::SpawnerBlockEntity::default()),
            types::BRUSHABLE_BLOCK => BlockEntityKind::Brushable(BrushableBlockEntity::default()),
            types::COMPARATOR => BlockEntityKind::Comparator(comparator_block_entity::ComparatorBlockEntity::default()),
            types::JUKEBOX => BlockEntityKind::Jukebox(jukebox_block_entity::JukeboxBlockEntity::default()),
            types::CONDUIT => BlockEntityKind::Conduit(conduit_block_entity::ConduitBlockEntity::default()),
            types::CREAKING_HEART => BlockEntityKind::CreakingHeart(creaking_heart_block_entity::CreakingHeartBlockEntity::default()),
            types::SCULK_SENSOR | types::CALIBRATED_SCULK_SENSOR => BlockEntityKind::SculkSensor(sculk_sensor_block_entity::SculkSensorBlockEntity::default()),
            types::SCULK_CATALYST => BlockEntityKind::SculkCatalyst(sculk_catalyst_block_entity::SculkCatalystBlockEntity::new(state)),
            types::SCULK_SHRIEKER => BlockEntityKind::SculkShrieker(sculk_shrieker_block_entity::SculkShriekerBlockEntity::default()),
            types::BEACON => BlockEntityKind::Beacon(beacon_block_entity::BeaconBlockEntity::default()),
            types::ENCHANTING_TABLE => BlockEntityKind::EnchantingTable(enchanting_table_block_entity::EnchantingTableBlockEntity::default()),
            types::CAMPFIRE => BlockEntityKind::Campfire(campfire_block_entity::CampfireBlockEntity::default()),
            types::LECTERN => BlockEntityKind::Lectern(lectern_block_entity::LecternBlockEntity::default()),
            types::CHISELED_BOOKSHELF => BlockEntityKind::ChiseledBookShelf(chiseled_book_shelf_block_entity::ChiseledBookShelfBlockEntity::default()),
            types::SHELF => BlockEntityKind::Shelf(shelf_block_entity::ShelfBlockEntity::default()),
            types::DECORATED_POT => BlockEntityKind::DecoratedPot(decorated_pot_block_entity::DecoratedPotBlockEntity::default()),
            types::BANNER => BlockEntityKind::Banner(banner_block_entity::BannerBlockEntity::default()),
            types::SKULL => BlockEntityKind::Skull(skull_block_entity::SkullBlockEntity::default()),
            types::BELL => BlockEntityKind::Bell(bell_block_entity::BellBlockEntity::default()),
            types::SIGN | types::HANGING_SIGN => BlockEntityKind::Sign(sign_block_entity::SignBlockEntity::default()),
            types::ENDER_CHEST => BlockEntityKind::EnderChest(ender_chest_block_entity::EnderChestBlockEntity::default()),
            types::VAULT => BlockEntityKind::Vault(vault::vault_block_entity::VaultBlockEntity::default()),
            types::TRIAL_SPAWNER => BlockEntityKind::TrialSpawner(trial_spawner_block_entity::TrialSpawnerBlockEntity::default()),
            types::POTENT_SULFUR => BlockEntityKind::PotentSulfur(potent_sulfur_block_entity::PotentSulfurBlockEntity::default()),
            types::COPPER_GOLEM_STATUE | types::DAYLIGHT_DETECTOR | types::END_PORTAL => BlockEntityKind::Empty,
            types::BREWING_STAND => BlockEntityKind::BrewingStand(brewing_stand_block_entity::BrewingStandBlockEntity::default()),
            types::END_GATEWAY => BlockEntityKind::EndGateway(the_end_gateway_block_entity::TheEndGatewayBlockEntity::default()),
            types::JIGSAW => BlockEntityKind::Jigsaw(jigsaw_block_entity::JigsawBlockEntity::default()),
            types::COMMAND_BLOCK => BlockEntityKind::CommandBlock(command_block_entity::CommandBlockEntity::new(state.is_block(wafer_data::blocks::CHAIN_COMMAND_BLOCK))),
            types::STRUCTURE_BLOCK => BlockEntityKind::StructureBlock(structure_block_entity::StructureBlockEntity::new(state)),
            types::TEST_BLOCK => BlockEntityKind::TestBlock(test_block_entity::TestBlockEntity::new(state)),
            types::TEST_INSTANCE_BLOCK => BlockEntityKind::TestInstanceBlock(test_instance_block_entity::TestInstanceBlockEntity::default()),
            types::PISTON => BlockEntityKind::PistonMoving(piston_moving_block_entity::PistonMovingBlockEntity::default()),
            _ => BlockEntityKind::Other(Vec::new()),
        };
        Some(BlockEntity { ty, pos, block_state: state, remove: false, components: DataComponentMap::EMPTY, id: NEXT_ID.fetch_add(1, Ordering::Relaxed), kind })
    }

    pub fn new_for(pos: BlockPos, state: BlockState) -> Option<BlockEntity> {
        if state.is_block(wafer_data::blocks::MOVING_PISTON) {
            return None;
        }
        BlockEntity::new(state.block().block_entity_type()?, pos, state)
    }

    pub fn ty(&self) -> BlockEntityType {
        self.ty
    }

    pub fn pos(&self) -> BlockPos {
        self.pos
    }

    pub fn block_state(&self) -> BlockState {
        self.block_state
    }

    pub fn set_block_state(&mut self, state: BlockState) {
        assert!(self.ty.is_valid(state), "Invalid block entity {:?} state at {:?}, got {state:?}", self.ty, self.pos);
        self.block_state = state;
        if let BlockEntityKind::Hopper(hopper) = &mut self.kind {
            hopper.set_block_state(state);
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn is_removed(&self) -> bool {
        self.remove
    }

    pub fn set_removed(&mut self) {
        self.remove = true;
    }

    pub fn clear_removed(&mut self) {
        self.remove = false;
    }

    pub fn set_level(&mut self, min_y: i32) {
        if let BlockEntityKind::Beacon(beacon) = &mut self.kind {
            beacon.set_level(min_y);
        }
    }

    pub fn is_valid_block_state(&self, state: BlockState) -> bool {
        self.ty.is_valid(state)
    }

    pub fn get_pos_from_tag(base: ChunkPos, tag: &Nbt) -> BlockPos {
        let int = |key: &str| tag_value_input::get_numeric(tag, key).and_then(int_value).unwrap_or(0);
        let (mut x, y, mut z) = (int("x"), int("y"), int("z"));
        if x >> 4 != base.x || z >> 4 != base.z {
            warn!("Block entity {} found in a wrong chunk, expected position from chunk {base}", crate::nbt::to_string(tag));
            x = base.min_block_x() + (x & 15);
            z = base.min_block_z() + (z & 15);
        }
        BlockPos::new(x, y, z)
    }

    pub fn get_name_for_reporting(&self) -> String {
        format!("{} // {}", self.ty.name(), canonical_class_name(self.ty.name()))
    }

    pub fn problem_path(&self) -> PathElement {
        PathElement::Text(format!("{}@BlockPos{{x={}, y={}, z={}}}", self.get_name_for_reporting(), self.pos.x, self.pos.y, self.pos.z))
    }

    pub fn load_static(pos: BlockPos, state: BlockState, tag: &Nbt) -> Option<BlockEntity> {
        let Some(ty) = tag.get("id").and_then(Nbt::as_str).and_then(crate::commands::arguments::parse_identifier).and_then(|id| BlockEntityType::by_name(&id)) else {
            error!("Skipping block entity with invalid type: {}", tag.get("id").map_or("null".to_string(), crate::nbt::to_string));
            return None;
        };
        let Some(mut entity) = BlockEntity::new(ty, pos, state) else {
            error!("Failed to create block entity {} for block {state:?} at position {pos} ", ty.name());
            return None;
        };
        let _collector = tag_value_input::ScopedCollector::new(entity.problem_path(), BLOCK_ENTITY_LOGGER).logging_as("Server thread");
        entity.load_with_components(tag);
        Some(entity)
    }

    fn load_additional(&mut self, input: &Nbt) {
        let state = self.block_state;
        match &mut self.kind {
            BlockEntityKind::Chest(chest) => chest.load_additional(input),
            BlockEntityKind::Barrel(barrel) => barrel.load_additional(input),
            BlockEntityKind::ShulkerBox(shulker) => shulker.load_additional(input),
            BlockEntityKind::Furnace(furnace) => furnace.load_additional(input),
            BlockEntityKind::Hopper(hopper) => hopper.load_additional(input),
            BlockEntityKind::Dispenser(dispenser) => dispenser.load_additional(input),
            BlockEntityKind::Crafter(crafter) => crafter.load_additional(input),
            BlockEntityKind::Beehive(beehive) => beehive.load_additional(input),
            BlockEntityKind::Spawner(spawner) => spawner.load_additional(input),
            BlockEntityKind::Brushable(brushable) => brushable.load_additional(input),
            BlockEntityKind::Comparator(comparator) => comparator.load_additional(input),
            BlockEntityKind::Jukebox(jukebox) => jukebox.load_additional(input),
            BlockEntityKind::Conduit(conduit) => conduit.load_additional(input),
            BlockEntityKind::CreakingHeart(heart) => heart.load_additional(input),
            BlockEntityKind::SculkSensor(sensor) => sensor.load_additional(input),
            BlockEntityKind::SculkCatalyst(catalyst) => catalyst.load_additional(input),
            BlockEntityKind::SculkShrieker(shrieker) => shrieker.load_additional(input),
            BlockEntityKind::Beacon(beacon) => beacon.load_additional(input),
            BlockEntityKind::EnchantingTable(enchanting_table) => enchanting_table.load_additional(input),
            BlockEntityKind::Campfire(campfire) => campfire.load_additional(input),
            BlockEntityKind::Lectern(lectern) => lectern.load_additional(input),
            BlockEntityKind::ChiseledBookShelf(bookshelf) => bookshelf.load_additional(input),
            BlockEntityKind::Shelf(shelf) => shelf.load_additional(input),
            BlockEntityKind::DecoratedPot(pot) => pot.load_additional(input),
            BlockEntityKind::Banner(banner) => banner.load_additional(input),
            BlockEntityKind::Skull(skull) => skull.load_additional(input),
            BlockEntityKind::Bell(_) | BlockEntityKind::EnderChest(_) | BlockEntityKind::Empty => {}
            BlockEntityKind::Sign(sign) => sign.load_additional(input),
            BlockEntityKind::Vault(vault) => vault.load_additional(input),
            BlockEntityKind::TrialSpawner(trial_spawner) => trial_spawner.load_additional(input),
            BlockEntityKind::PotentSulfur(potent_sulfur) => potent_sulfur.load_additional(input),
            BlockEntityKind::BrewingStand(brewing_stand) => brewing_stand.load_additional(input),
            BlockEntityKind::EndGateway(gateway) => gateway.load_additional(input),
            BlockEntityKind::Jigsaw(jigsaw) => jigsaw.load_additional(input, state),
            BlockEntityKind::PistonMoving(piston) => piston.load_additional(input),
            BlockEntityKind::CommandBlock(command_block) => command_block.load_additional(input),
            BlockEntityKind::StructureBlock(structure_block) => structure_block.load_additional(input),
            BlockEntityKind::TestBlock(test_block) => test_block.load_additional(input),
            BlockEntityKind::TestInstanceBlock(test_instance) => test_instance.load_additional(input),
            BlockEntityKind::Other(data) => {
                let _ = state;
                *data = match input {
                    Nbt::Compound(fields) => {
                        fields.iter().filter(|(k, _)| !matches!(k.as_str(), "id" | "x" | "y" | "z" | "components" | "keepPacked")).cloned().collect()
                    }
                    _ => Vec::new(),
                };
            }
        }
    }

    fn save_additional(&self, output: &mut Vec<(String, Nbt)>) {
        match &self.kind {
            BlockEntityKind::Chest(chest) => chest.save_additional(output),
            BlockEntityKind::Barrel(barrel) => barrel.save_additional(output),
            BlockEntityKind::ShulkerBox(shulker) => shulker.save_additional(output),
            BlockEntityKind::Furnace(furnace) => furnace.save_additional(output),
            BlockEntityKind::Hopper(hopper) => hopper.save_additional(output),
            BlockEntityKind::Dispenser(dispenser) => dispenser.save_additional(output),
            BlockEntityKind::Crafter(crafter) => crafter.save_additional(output),
            BlockEntityKind::Beehive(beehive) => beehive.save_additional(output),
            BlockEntityKind::Spawner(spawner) => spawner.save_additional(output),
            BlockEntityKind::Brushable(brushable) => brushable.save_additional(output),
            BlockEntityKind::Comparator(comparator) => comparator.save_additional(output),
            BlockEntityKind::Jukebox(jukebox) => jukebox.save_additional(output),
            BlockEntityKind::Conduit(conduit) => conduit.save_additional(output),
            BlockEntityKind::CreakingHeart(heart) => heart.save_additional(output),
            BlockEntityKind::SculkSensor(sensor) => sensor.save_additional(output),
            BlockEntityKind::SculkCatalyst(catalyst) => catalyst.save_additional(output),
            BlockEntityKind::SculkShrieker(shrieker) => shrieker.save_additional(output),
            BlockEntityKind::Beacon(beacon) => beacon.save_additional(output),
            BlockEntityKind::EnchantingTable(enchanting_table) => enchanting_table.save_additional(output),
            BlockEntityKind::Campfire(campfire) => campfire.save_additional(output),
            BlockEntityKind::Lectern(lectern) => lectern.save_additional(output),
            BlockEntityKind::ChiseledBookShelf(bookshelf) => bookshelf.save_additional(output),
            BlockEntityKind::Shelf(shelf) => shelf.save_additional(output),
            BlockEntityKind::DecoratedPot(pot) => pot.save_additional(output),
            BlockEntityKind::Banner(banner) => banner.save_additional(output),
            BlockEntityKind::Skull(skull) => skull.save_additional(output),
            BlockEntityKind::Bell(_) | BlockEntityKind::EnderChest(_) | BlockEntityKind::Empty => {}
            BlockEntityKind::Sign(sign) => sign.save_additional(output),
            BlockEntityKind::Vault(vault) => vault.save_additional(output),
            BlockEntityKind::TrialSpawner(trial_spawner) => trial_spawner.save_additional(output),
            BlockEntityKind::PotentSulfur(potent_sulfur) => potent_sulfur.save_additional(output),
            BlockEntityKind::BrewingStand(brewing_stand) => brewing_stand.save_additional(output),
            BlockEntityKind::EndGateway(gateway) => gateway.save_additional(output),
            BlockEntityKind::Jigsaw(jigsaw) => jigsaw.save_additional(output),
            BlockEntityKind::PistonMoving(piston) => piston.save_additional(output),
            BlockEntityKind::CommandBlock(command_block) => command_block.save_additional(output),
            BlockEntityKind::StructureBlock(structure_block) => structure_block.save_additional(output),
            BlockEntityKind::TestBlock(test_block) => test_block.save_additional(output),
            BlockEntityKind::TestInstanceBlock(test_instance) => test_instance.save_additional(output),
            BlockEntityKind::Other(data) => output.extend(data.iter().cloned()),
        }
    }

    pub fn load_with_components(&mut self, input: &Nbt) {
        self.load_additional(input);
        self.components = tag_value_input::read_or_error(input, "components", decode_component_map).unwrap_or(DataComponentMap::EMPTY);
    }

    pub fn load_custom_only(&mut self, input: &Nbt) {
        self.load_additional(input);
    }

    pub fn save_with_full_metadata(&self) -> Nbt {
        let mut output = self.save_without_metadata_fields();
        self.save_metadata(&mut output);
        Nbt::Compound(output)
    }

    pub fn save_with_id(&self) -> Nbt {
        let mut output = self.save_without_metadata_fields();
        output.push(("id".into(), Nbt::String(self.ty.name().into())));
        Nbt::Compound(output)
    }

    pub fn save_without_metadata(&self) -> Nbt {
        Nbt::Compound(self.save_without_metadata_fields())
    }

    fn save_without_metadata_fields(&self) -> Vec<(String, Nbt)> {
        let mut output = Vec::new();
        self.save_additional(&mut output);
        output.push(("components".into(), encode_component_map(&self.components)));
        output
    }

    pub fn save_custom_only(&self) -> Nbt {
        let mut output = Vec::new();
        self.save_additional(&mut output);
        Nbt::Compound(output)
    }

    fn save_metadata(&self, output: &mut Vec<(String, Nbt)>) {
        output.push(("id".into(), Nbt::String(self.ty.name().into())));
        output.push(("x".into(), Nbt::Int(self.pos.x)));
        output.push(("y".into(), Nbt::Int(self.pos.y)));
        output.push(("z".into(), Nbt::Int(self.pos.z)));
    }

    pub fn get_update_tag(&self) -> Nbt {
        if let BlockEntityKind::Spawner(_) = self.kind {
            let mut tag = self.save_custom_only();
            if let Nbt::Compound(fields) = &mut tag {
                fields.retain(|(key, _)| key != "SpawnPotentials");
            }
            return tag;
        }
        if let BlockEntityKind::Brushable(brushable) = &self.kind {
            return brushable.get_update_tag();
        }
        if let BlockEntityKind::Campfire(campfire) = &self.kind {
            return campfire.get_update_tag();
        }
        if let BlockEntityKind::Shelf(shelf) = &self.kind {
            return shelf.get_update_tag();
        }
        if let BlockEntityKind::Vault(vault) = &self.kind {
            return vault.get_update_tag();
        }
        if let BlockEntityKind::TrialSpawner(trial_spawner) = &self.kind {
            return trial_spawner.get_update_tag(self.block_state);
        }
        match self.update_tag_kind() {
            UpdateTag::CustomOnly => self.save_custom_only(),
            UpdateTag::WithoutMetadata => self.save_without_metadata(),
            UpdateTag::None => Nbt::Compound(Vec::new()),
        }
    }

    pub fn has_update_packet(&self) -> bool {
        !matches!(self.kind, BlockEntityKind::PistonMoving(_))
            && (matches!(self.kind, BlockEntityKind::Spawner(_) | BlockEntityKind::Brushable(_) | BlockEntityKind::Campfire(_) | BlockEntityKind::Shelf(_) | BlockEntityKind::Vault(_) | BlockEntityKind::TrialSpawner(_))
                || !matches!(self.update_tag_kind(), UpdateTag::None))
    }

    fn update_tag_kind(&self) -> UpdateTag {
        match self.ty {
            types::SIGN
            | types::HANGING_SIGN
            | types::BEACON
            | types::CONDUIT
            | types::CREAKING_HEART
            | types::DECORATED_POT
            | types::JIGSAW
            | types::SKULL
            | types::STRUCTURE_BLOCK
            | types::TEST_BLOCK
            | types::TEST_INSTANCE_BLOCK
            | types::END_GATEWAY
            | types::PISTON => UpdateTag::CustomOnly,
            types::BANNER => UpdateTag::WithoutMetadata,
            _ => UpdateTag::None,
        }
    }

    pub fn apply_components_from_item_stack(&mut self, stack: &ItemStack) {
        self.apply_components(stack.components(), stack.components_patch());
    }

    pub fn apply_components(&mut self, full: &PatchedDataComponentMap, patch: &DataComponentPatch) {
        let mut implicit = vec![dc::BLOCK_ENTITY_DATA, dc::BLOCK_STATE];
        let mut get = |ty: DataComponentType| {
            implicit.push(ty);
            full.get(ty)
        };
        if let Some(base) = self.container_base_mut() {
            base.apply_implicit_components(&mut get);
        } else if let BlockEntityKind::Beacon(beacon) = &mut self.kind {
            beacon.apply_implicit_components(&mut get);
        } else if let BlockEntityKind::EnchantingTable(enchanting_table) = &mut self.kind {
            enchanting_table.apply_implicit_components(&mut get);
        } else if let BlockEntityKind::Campfire(campfire) = &mut self.kind {
            campfire.apply_implicit_components(&mut get);
        } else if let BlockEntityKind::ChiseledBookShelf(bookshelf) = &mut self.kind {
            bookshelf.apply_implicit_components(&mut get);
        } else if let BlockEntityKind::Shelf(shelf) = &mut self.kind {
            shelf.apply_implicit_components(&mut get);
        } else if let BlockEntityKind::DecoratedPot(pot) = &mut self.kind {
            pot.apply_implicit_components(&mut get);
        } else if let BlockEntityKind::Banner(banner) = &mut self.kind {
            banner.apply_implicit_components(&mut get);
        } else if let BlockEntityKind::Skull(skull) = &mut self.kind {
            skull.apply_implicit_components(&mut get);
        } else if let BlockEntityKind::Sign(sign) = &mut self.kind {
            sign.apply_implicit_components(&mut get);
        } else if let BlockEntityKind::CommandBlock(command_block) = &mut self.kind {
            command_block.apply_implicit_components(&mut get);
        } else if let BlockEntityKind::Beehive(beehive) = &mut self.kind {
            beehive.apply_implicit_components(&mut get);
        }
        let added: Vec<(DataComponentType, Nbt)> =
            patch.iter().filter(|(ty, _)| !implicit.contains(ty)).filter_map(|(ty, value)| value.map(|v| (ty, v.clone()))).collect();
        self.components = DataComponentMap::new(added);
    }

    pub fn collect_components(&self) -> Vec<(DataComponentType, Option<Nbt>)> {
        let mut result: Vec<(DataComponentType, Option<Nbt>)> = self.components.iter().map(|(t, v)| (t, Some(v.clone()))).collect();
        if let Some(base) = self.container_base() {
            base.collect_implicit_components(&mut result, self.is_randomizable());
        } else if let BlockEntityKind::Beacon(beacon) = &self.kind {
            beacon.collect_implicit_components(&mut result);
        } else if let BlockEntityKind::EnchantingTable(enchanting_table) = &self.kind {
            enchanting_table.collect_implicit_components(&mut result);
        } else if let BlockEntityKind::Campfire(campfire) = &self.kind {
            campfire.collect_implicit_components(&mut result);
        } else if let BlockEntityKind::ChiseledBookShelf(bookshelf) = &self.kind {
            bookshelf.collect_implicit_components(&mut result);
        } else if let BlockEntityKind::Shelf(shelf) = &self.kind {
            shelf.collect_implicit_components(&mut result);
        } else if let BlockEntityKind::DecoratedPot(pot) = &self.kind {
            pot.collect_implicit_components(&mut result);
        } else if let BlockEntityKind::Banner(banner) = &self.kind {
            banner.collect_implicit_components(&mut result);
        } else if let BlockEntityKind::Skull(skull) = &self.kind {
            skull.collect_implicit_components(&mut result);
        } else if let BlockEntityKind::Sign(sign) = &self.kind {
            sign.collect_implicit_components(&mut result);
        } else if let BlockEntityKind::CommandBlock(command_block) = &self.kind {
            command_block.collect_implicit_components(&mut result);
        } else if let BlockEntityKind::Beehive(beehive) = &self.kind {
            beehive.collect_implicit_components(&mut result);
        }
        result
    }

    pub fn components(&self) -> &DataComponentMap {
        &self.components
    }

    pub fn set_components(&mut self, components: DataComponentMap) {
        self.components = components;
    }

    pub fn remove_components_from_tag(&self, output: &mut Vec<(String, Nbt)>) {
        if self.container_base().is_some() {
            output.retain(|(k, _)| !matches!(k.as_str(), "CustomName" | "lock" | "Items"));
            if self.is_randomizable() {
                output.retain(|(k, _)| !matches!(k.as_str(), "LootTable" | "LootTableSeed"));
            }
        } else if let BlockEntityKind::Beacon(_) = self.kind {
            output.retain(|(k, _)| !matches!(k.as_str(), "CustomName" | "lock"));
        } else if let BlockEntityKind::EnchantingTable(_) = self.kind {
            output.retain(|(k, _)| k != "CustomName");
        } else if let BlockEntityKind::Campfire(_) | BlockEntityKind::ChiseledBookShelf(_) | BlockEntityKind::Shelf(_) = self.kind {
            output.retain(|(k, _)| k != "Items");
        } else if let BlockEntityKind::DecoratedPot(_) = self.kind {
            output.retain(|(k, _)| !matches!(k.as_str(), "sherds" | "item"));
        } else if let BlockEntityKind::Banner(_) = self.kind {
            output.retain(|(k, _)| !matches!(k.as_str(), "patterns" | "CustomName"));
        } else if let BlockEntityKind::CommandBlock(_) = self.kind {
            output.retain(|(k, _)| !matches!(k.as_str(), "CustomName" | "conditionMet" | "powered"));
        } else if let BlockEntityKind::Skull(_) = self.kind {
            output.retain(|(k, _)| !matches!(k.as_str(), "profile" | "note_block_sound" | "custom_name"));
        } else if let BlockEntityKind::Sign(_) = self.kind {
            output.retain(|(k, _)| !matches!(k.as_str(), "front_text" | "back_text" | "is_waxed"));
        } else if let BlockEntityKind::Beehive(_) = self.kind {
            output.retain(|(k, _)| k != "bees");
        }
    }

    pub fn container_base(&self) -> Option<&BaseContainerBlockEntity> {
        Some(match &self.kind {
            BlockEntityKind::Chest(c) => &c.base,
            BlockEntityKind::Barrel(b) => &b.base,
            BlockEntityKind::ShulkerBox(s) => &s.base,
            BlockEntityKind::Furnace(f) => &f.base,
            BlockEntityKind::Hopper(h) => &h.base,
            BlockEntityKind::Dispenser(d) => &d.base,
            BlockEntityKind::Crafter(c) => &c.base,
            BlockEntityKind::BrewingStand(b) => &b.base,
            _ => return None,
        })
    }

    pub fn container_base_mut(&mut self) -> Option<&mut BaseContainerBlockEntity> {
        Some(match &mut self.kind {
            BlockEntityKind::Chest(c) => &mut c.base,
            BlockEntityKind::Barrel(b) => &mut b.base,
            BlockEntityKind::ShulkerBox(s) => &mut s.base,
            BlockEntityKind::Furnace(f) => &mut f.base,
            BlockEntityKind::Hopper(h) => &mut h.base,
            BlockEntityKind::Dispenser(d) => &mut d.base,
            BlockEntityKind::Crafter(c) => &mut c.base,
            BlockEntityKind::BrewingStand(b) => &mut b.base,
            _ => return None,
        })
    }

    pub fn is_randomizable(&self) -> bool {
        matches!(
            self.kind,
            BlockEntityKind::Chest(_)
                | BlockEntityKind::Barrel(_)
                | BlockEntityKind::ShulkerBox(_)
                | BlockEntityKind::Hopper(_)
                | BlockEntityKind::Dispenser(_)
                | BlockEntityKind::Crafter(_)
                | BlockEntityKind::DecoratedPot(_)
        )
    }

    pub fn default_name(&self) -> &'static str {
        match self.ty {
            types::CHEST | types::TRAPPED_CHEST => "container.chest",
            types::BARREL => "container.barrel",
            types::SHULKER_BOX => "container.shulkerBox",
            types::FURNACE => "container.furnace",
            types::BLAST_FURNACE => "container.blast_furnace",
            types::SMOKER => "container.smoker",
            types::HOPPER => "container.hopper",
            types::DISPENSER => "container.dispenser",
            types::DROPPER => "container.dropper",
            types::CRAFTER => "container.crafter",
            types::BEACON => "container.beacon",
            types::BREWING_STAND => "container.brewing",
            types::ENCHANTING_TABLE => "container.enchant",
            types::LECTERN => "container.lectern",
            types::BANNER => "block.minecraft.banner",
            _ => "",
        }
    }

    pub fn name(&self) -> crate::network::chat::Component {
        match self.get_custom_name() {
            Some(name) => crate::world::item::component::values::text(name),
            None => crate::network::chat::Component::translatable(self.default_name()),
        }
    }

    pub fn get_custom_name(&self) -> Option<&Nbt> {
        match &self.kind {
            BlockEntityKind::Beacon(beacon) => beacon.get_custom_name(),
            BlockEntityKind::EnchantingTable(enchanting_table) => enchanting_table.get_custom_name(),
            BlockEntityKind::Banner(banner) => banner.get_custom_name(),
            _ => self.container_base().and_then(|b| b.name.as_ref()),
        }
    }

    pub fn has_custom_name(&self) -> bool {
        self.get_custom_name().is_some()
    }

    pub fn game_event_listener(&self) -> Option<crate::world::level::gameevent::game_event_listener::GameEventListener> {
        match self.kind {
            BlockEntityKind::SculkSensor(_) | BlockEntityKind::SculkCatalyst(_) | BlockEntityKind::SculkShrieker(_) => Some(crate::world::level::gameevent::game_event_listener::GameEventListener::BlockEntity { pos: self.pos, id: self.id }),
            _ => None,
        }
    }

    pub fn ticker(&self, state: BlockState) -> Option<BlockEntityTicker> {
        let block = state.block();
        match self.ty {
            types::FURNACE | types::BLAST_FURNACE | types::SMOKER if block.is_instance_of("AbstractFurnaceBlock") => Some(BlockEntityTicker::Furnace),
            types::HOPPER if block.is_instance_of("HopperBlock") => Some(BlockEntityTicker::Hopper),
            types::SHULKER_BOX if block.is_instance_of("ShulkerBoxBlock") => Some(BlockEntityTicker::ShulkerBox),
            types::DAYLIGHT_DETECTOR if block.is_instance_of("DaylightDetectorBlock") => Some(BlockEntityTicker::DaylightDetector),
            types::JUKEBOX if block.is_instance_of("JukeboxBlock") && state.get(wafer_data::properties::HAS_RECORD) => Some(BlockEntityTicker::Jukebox),
            types::CONDUIT if block.is_instance_of("ConduitBlock") => Some(BlockEntityTicker::Conduit),
            types::CREAKING_HEART if block.is_instance_of("CreakingHeartBlock") && state.get(wafer_data::properties::CREAKING_HEART_STATE) != wafer_data::properties::CreakingHeartState::Uprooted => Some(BlockEntityTicker::CreakingHeart),
            types::BEACON if block.is_instance_of("BeaconBlock") => Some(BlockEntityTicker::Beacon),
            types::SCULK_SENSOR | types::CALIBRATED_SCULK_SENSOR if block.is_instance_of("SculkSensorBlock") => Some(BlockEntityTicker::SculkSensor),
            types::SCULK_CATALYST if block.is_instance_of("SculkCatalystBlock") => Some(BlockEntityTicker::SculkCatalyst),
            types::SCULK_SHRIEKER if block.is_instance_of("SculkShriekerBlock") => Some(BlockEntityTicker::SculkShrieker),
            types::BREWING_STAND if block.is_instance_of("BrewingStandBlock") => Some(BlockEntityTicker::BrewingStand),
            types::BELL if block.is_instance_of("BellBlock") => Some(BlockEntityTicker::Bell),
            types::SIGN | types::HANGING_SIGN if block.is_instance_of("SignBlock") => Some(BlockEntityTicker::Sign),
            types::VAULT if block.is_instance_of("VaultBlock") => Some(BlockEntityTicker::Vault),
            types::TRIAL_SPAWNER if block.is_instance_of("TrialSpawnerBlock") => Some(BlockEntityTicker::TrialSpawner),
            types::POTENT_SULFUR if block.is_instance_of("PotentSulfurBlock") => match state.get(wafer_data::properties::POTENT_SULFUR_STATE) {
                wafer_data::properties::PotentSulfurState::Dry => None,
                wafer_data::properties::PotentSulfurState::Wet => Some(BlockEntityTicker::PotentSulfurWet),
                wafer_data::properties::PotentSulfurState::Dormant => Some(BlockEntityTicker::PotentSulfurDormant),
                wafer_data::properties::PotentSulfurState::Erupting => Some(BlockEntityTicker::PotentSulfurErupting),
                wafer_data::properties::PotentSulfurState::Continuous => Some(BlockEntityTicker::PotentSulfurContinuous),
            },
            types::CRAFTER if block.is_instance_of("CrafterBlock") => Some(BlockEntityTicker::Crafter),
            types::END_GATEWAY if block.is_instance_of("EndGatewayBlock") => Some(BlockEntityTicker::EndGateway),
            types::MOB_SPAWNER if block.is_instance_of("SpawnerBlock") => Some(BlockEntityTicker::Spawner),
            types::PISTON if block.is_instance_of("MovingPistonBlock") => Some(BlockEntityTicker::Piston),
            types::BEEHIVE if block.is_instance_of("BeehiveBlock") => Some(BlockEntityTicker::Beehive),
            types::CAMPFIRE if block.is_instance_of("CampfireBlock") => {
                Some(if state.get(wafer_data::properties::LIT) { BlockEntityTicker::CampfireCook } else { BlockEntityTicker::CampfireCooldown })
            }
            _ => None,
        }
    }

    pub fn trigger_event(&mut self, b0: i32, b1: i32) -> bool {
        match &mut self.kind {
            BlockEntityKind::Chest(chest) => chest.trigger_event(b0, b1),
            BlockEntityKind::ShulkerBox(shulker) => shulker.trigger_event(b0, b1),
            BlockEntityKind::Spawner(spawner) => spawner.trigger_event(b0, b1),
            BlockEntityKind::EndGateway(gateway) if b0 == 1 => {
                gateway.teleport_cooldown = the_end_gateway_block_entity::COOLDOWN_TIME;
                true
            }
            _ => false,
        }
    }
}

pub fn int_value(tag: &Nbt) -> Option<i32> {
    Some(match *tag {
        Nbt::Byte(v) => v as i32,
        Nbt::Short(v) => v as i32,
        Nbt::Int(v) => v,
        Nbt::Long(v) => v as i32,
        Nbt::Float(v) => v.floor() as i32,
        Nbt::Double(v) => v.floor() as i32,
        _ => return None,
    })
}

pub fn byte_value(tag: &Nbt) -> Option<i8> {
    Some(match *tag {
        Nbt::Float(v) => (v.floor() as i32 & 0xFF) as i8,
        Nbt::Double(v) => (v.floor() as i32 & 0xFF) as i8,
        _ => tag.as_i64()? as i8,
    })
}

pub fn byte_or(input: &Nbt, key: &str, default: i8) -> i8 {
    tag_value_input::get_numeric(input, key).and_then(byte_value).unwrap_or(default)
}

pub fn float_or(input: &Nbt, key: &str, default: f32) -> f32 {
    tag_value_input::get_numeric(input, key).and_then(float_value).unwrap_or(default)
}

pub fn float_value(tag: &Nbt) -> Option<f32> {
    Some(match *tag {
        Nbt::Float(v) => v,
        Nbt::Double(v) => v as f32,
        _ => tag.as_i64()? as f32,
    })
}

pub fn long_value(tag: &Nbt) -> Option<i64> {
    Some(match *tag {
        Nbt::Float(v) => v as i64,
        Nbt::Double(v) => v as i64,
        _ => tag.as_i64()?,
    })
}

pub fn int_or(input: &Nbt, key: &str, default: i32) -> i32 {
    tag_value_input::get_numeric(input, key).and_then(int_value).unwrap_or(default)
}

fn canonical_class_name(type_name: &str) -> &'static str {
    match type_name {
        "minecraft:furnace" => "net.minecraft.world.level.block.entity.FurnaceBlockEntity",
        "minecraft:chest" => "net.minecraft.world.level.block.entity.ChestBlockEntity",
        "minecraft:trapped_chest" => "net.minecraft.world.level.block.entity.TrappedChestBlockEntity",
        "minecraft:ender_chest" => "net.minecraft.world.level.block.entity.EnderChestBlockEntity",
        "minecraft:jukebox" => "net.minecraft.world.level.block.entity.JukeboxBlockEntity",
        "minecraft:dispenser" => "net.minecraft.world.level.block.entity.DispenserBlockEntity",
        "minecraft:dropper" => "net.minecraft.world.level.block.entity.DropperBlockEntity",
        "minecraft:sign" => "net.minecraft.world.level.block.entity.SignBlockEntity",
        "minecraft:hanging_sign" => "net.minecraft.world.level.block.entity.HangingSignBlockEntity",
        "minecraft:mob_spawner" => "net.minecraft.world.level.block.entity.SpawnerBlockEntity",
        "minecraft:creaking_heart" => "net.minecraft.world.level.block.entity.CreakingHeartBlockEntity",
        "minecraft:piston" => "net.minecraft.world.level.block.piston.PistonMovingBlockEntity",
        "minecraft:brewing_stand" => "net.minecraft.world.level.block.entity.BrewingStandBlockEntity",
        "minecraft:enchanting_table" => "net.minecraft.world.level.block.entity.EnchantingTableBlockEntity",
        "minecraft:end_portal" => "net.minecraft.world.level.block.entity.TheEndPortalBlockEntity",
        "minecraft:beacon" => "net.minecraft.world.level.block.entity.BeaconBlockEntity",
        "minecraft:skull" => "net.minecraft.world.level.block.entity.SkullBlockEntity",
        "minecraft:daylight_detector" => "net.minecraft.world.level.block.entity.DaylightDetectorBlockEntity",
        "minecraft:hopper" => "net.minecraft.world.level.block.entity.HopperBlockEntity",
        "minecraft:comparator" => "net.minecraft.world.level.block.entity.ComparatorBlockEntity",
        "minecraft:banner" => "net.minecraft.world.level.block.entity.BannerBlockEntity",
        "minecraft:structure_block" => "net.minecraft.world.level.block.entity.StructureBlockEntity",
        "minecraft:end_gateway" => "net.minecraft.world.level.block.entity.TheEndGatewayBlockEntity",
        "minecraft:command_block" => "net.minecraft.world.level.block.entity.CommandBlockEntity",
        "minecraft:shulker_box" => "net.minecraft.world.level.block.entity.ShulkerBoxBlockEntity",
        "minecraft:conduit" => "net.minecraft.world.level.block.entity.ConduitBlockEntity",
        "minecraft:barrel" => "net.minecraft.world.level.block.entity.BarrelBlockEntity",
        "minecraft:smoker" => "net.minecraft.world.level.block.entity.SmokerBlockEntity",
        "minecraft:blast_furnace" => "net.minecraft.world.level.block.entity.BlastFurnaceBlockEntity",
        "minecraft:lectern" => "net.minecraft.world.level.block.entity.LecternBlockEntity",
        "minecraft:bell" => "net.minecraft.world.level.block.entity.BellBlockEntity",
        "minecraft:jigsaw" => "net.minecraft.world.level.block.entity.JigsawBlockEntity",
        "minecraft:campfire" => "net.minecraft.world.level.block.entity.CampfireBlockEntity",
        "minecraft:beehive" => "net.minecraft.world.level.block.entity.BeehiveBlockEntity",
        "minecraft:sculk_sensor" => "net.minecraft.world.level.block.entity.SculkSensorBlockEntity",
        "minecraft:calibrated_sculk_sensor" => "net.minecraft.world.level.block.entity.CalibratedSculkSensorBlockEntity",
        "minecraft:sculk_catalyst" => "net.minecraft.world.level.block.entity.SculkCatalystBlockEntity",
        "minecraft:sculk_shrieker" => "net.minecraft.world.level.block.entity.SculkShriekerBlockEntity",
        "minecraft:chiseled_bookshelf" => "net.minecraft.world.level.block.entity.ChiseledBookShelfBlockEntity",
        "minecraft:shelf" => "net.minecraft.world.level.block.entity.ShelfBlockEntity",
        "minecraft:brushable_block" => "net.minecraft.world.level.block.entity.BrushableBlockEntity",
        "minecraft:decorated_pot" => "net.minecraft.world.level.block.entity.DecoratedPotBlockEntity",
        "minecraft:crafter" => "net.minecraft.world.level.block.entity.CrafterBlockEntity",
        "minecraft:trial_spawner" => "net.minecraft.world.level.block.entity.TrialSpawnerBlockEntity",
        "minecraft:vault" => "net.minecraft.world.level.block.entity.vault.VaultBlockEntity",
        "minecraft:test_block" => "net.minecraft.world.level.block.entity.TestBlockEntity",
        "minecraft:test_instance_block" => "net.minecraft.world.level.block.entity.TestInstanceBlockEntity",
        "minecraft:copper_golem_statue" => "net.minecraft.world.level.block.entity.CopperGolemStatueBlockEntity",
        "minecraft:potent_sulfur" => "net.minecraft.world.level.block.entity.PotentSulfurBlockEntity",
        _ => "net.minecraft.world.level.block.entity.BlockEntity",
    }
}

pub fn decode_component_map(tag: &Nbt) -> Result<DataComponentMap, String> {
    let key = |key: &str| {
        let id = crate::commands::arguments::parse_identifier(key).ok_or_else(|| format!("Not a valid resource location: {key}"))?;
        let ty = DataComponentType::by_name(&id).ok_or_else(|| format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:data_component_type]: {id}"))?;
        if ty.is_transient() {
            return Err(format!("Encountered transient component {id}"));
        }
        Ok(ty)
    };
    let entries = crate::util::codec::dispatched_map(tag, key, |ty, value| ty.decode(value).map(|decoded| (*ty, decoded)))?;
    Ok(DataComponentMap::new(entries.into_iter().map(|(_, entry)| entry).collect()))
}

pub fn encode_component_map(map: &DataComponentMap) -> Nbt {
    Nbt::Compound(map.iter().filter(|(ty, _)| !ty.is_transient()).map(|(ty, value)| (ty.name().to_string(), value.clone())).collect())
}
