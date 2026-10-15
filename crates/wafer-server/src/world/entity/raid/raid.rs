use wafer_core::BlockPos;
use wafer_protocol::nbt::Nbt;

use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::component as dc;

pub enum Raid {}

impl Raid {
    pub fn is_active(&self) -> bool {
        match *self {}
    }

    pub fn is_over(&self) -> bool {
        match *self {}
    }

    pub fn is_loss(&self) -> bool {
        match *self {}
    }

    pub fn is_victory(&self) -> bool {
        match *self {}
    }

    pub fn is_stopped(&self) -> bool {
        match *self {}
    }

    pub fn has_first_wave_spawned(&self) -> bool {
        match *self {}
    }

    pub fn is_between_waves(&self) -> bool {
        match *self {}
    }

    pub fn get_leader(&self, _wave: i32) -> Option<EntityId> {
        match *self {}
    }

    pub fn get_center(&self) -> BlockPos {
        match *self {}
    }
}

fn banner_pattern_layer(pattern: &str, color: &str) -> Nbt {
    Nbt::Compound(vec![("pattern".into(), Nbt::String(pattern.into())), ("color".into(), Nbt::String(color.into()))])
}

pub fn get_ominous_banner_instance() -> ItemStack {
    let patterns = Nbt::List(vec![
        banner_pattern_layer("minecraft:rhombus", "cyan"),
        banner_pattern_layer("minecraft:stripe_bottom", "light_gray"),
        banner_pattern_layer("minecraft:stripe_center", "gray"),
        banner_pattern_layer("minecraft:border", "light_gray"),
        banner_pattern_layer("minecraft:stripe_middle", "black"),
        banner_pattern_layer("minecraft:half_horizontal", "light_gray"),
        banner_pattern_layer("minecraft:circle", "light_gray"),
        banner_pattern_layer("minecraft:border", "black"),
    ]);
    let tooltip_display = Nbt::Compound(vec![("hidden_components".into(), Nbt::List(vec![Nbt::String(dc::BANNER_PATTERNS.name().into())]))]);
    let item_name = Nbt::Compound(vec![("translate".into(), Nbt::String("block.minecraft.ominous_banner".into()))]);
    let mut banner = ItemStack::new(wafer_data::items::WHITE_BANNER, 1);
    banner.set(dc::BANNER_PATTERNS, Some(dc::BANNER_PATTERNS.decode(&patterns).unwrap_or(patterns)));
    banner.set(dc::TOOLTIP_DISPLAY, Some(dc::TOOLTIP_DISPLAY.decode(&tooltip_display).unwrap_or(tooltip_display)));
    banner.set(dc::ITEM_NAME, Some(dc::ITEM_NAME.decode(&item_name).unwrap_or(item_name)));
    banner.set(dc::RARITY, Some(Nbt::String("uncommon".into())));
    banner
}
