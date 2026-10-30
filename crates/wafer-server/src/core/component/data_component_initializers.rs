use crate::resources::registry_load_task::{ASYNC_SUPPLY, FORK_JOIN_TAIL};
use crate::util::java_throwable::JavaThrowable;
use crate::world::item::component::codec::{registry_contains, tag_exists};

const AND_THEN_OTHER: &str = "net.minecraft.core.component.DataComponentInitializers$Initializer.lambda$andThen$0(DataComponentInitializers.java:120)";
const AND_THEN_THIS: &str = "net.minecraft.core.component.DataComponentInitializers$Initializer.lambda$andThen$0(DataComponentInitializers.java:119)";
const INITIALIZER_ENTRY_RUN: &str = "net.minecraft.core.component.DataComponentInitializers$InitializerEntry.run(DataComponentInitializers.java:100)";
const RUN_INITIALIZERS: &str = "net.minecraft.core.component.DataComponentInitializers.runInitializers(DataComponentInitializers.java:29)";
const BUILD: &str = "net.minecraft.core.component.DataComponentInitializers.build(DataComponentInitializers.java:52)";
const LOAD_RESOURCES_LAMBDA: &str = "net.minecraft.server.ReloadableServerResources.lambda$loadResources$1(ReloadableServerResources.java:84)";

const HEADS: [&[&str]; 20] = [
    &[
        "net.minecraft.core.HolderGetter$Provider.lambda$getOrThrow$3(HolderGetter.java:47)",
        "java.base/java.util.Optional.orElseThrow(Optional.java:403)",
        "net.minecraft.core.HolderGetter$Provider.getOrThrow(HolderGetter.java:47)",
        "net.minecraft.world.item.Item$Properties.lambda$fireResistant$0(Item.java:248)",
        "net.minecraft.core.component.DataComponentInitializers$SingleComponentInitializer.lambda$asInitializer$0(DataComponentInitializers.java:134)",
    ],
    &[
        "net.minecraft.core.HolderGetter$Provider.lambda$getOrThrow$1(HolderGetter.java:39)",
        "java.base/java.util.Optional.orElseThrow(Optional.java:403)",
        "net.minecraft.core.HolderGetter$Provider.getOrThrow(HolderGetter.java:39)",
        "net.minecraft.world.item.Item$Properties.lambda$delayedHolderComponent$0(Item.java:481)",
    ],
    &[
        "net.minecraft.core.HolderGetter$Provider.lambda$getOrThrow$1(HolderGetter.java:39)",
        "java.base/java.util.Optional.orElseThrow(Optional.java:403)",
        "net.minecraft.core.HolderGetter$Provider.getOrThrow(HolderGetter.java:39)",
        "net.minecraft.world.item.Item$Properties.lambda$shovel$0(Item.java:301)",
        "net.minecraft.core.component.DataComponentInitializers$SingleComponentInitializer.lambda$asInitializer$0(DataComponentInitializers.java:134)",
    ],
    &[
        "net.minecraft.core.HolderGetter$Provider.lambda$getOrThrow$1(HolderGetter.java:39)",
        "java.base/java.util.Optional.orElseThrow(Optional.java:403)",
        "net.minecraft.core.HolderGetter$Provider.getOrThrow(HolderGetter.java:39)",
        "net.minecraft.world.item.Item$Properties.lambda$axe$0(Item.java:291)",
        "net.minecraft.core.component.DataComponentInitializers$SingleComponentInitializer.lambda$asInitializer$0(DataComponentInitializers.java:134)",
    ],
    &[
        "net.minecraft.core.HolderGetter$Provider.lambda$getOrThrow$1(HolderGetter.java:39)",
        "java.base/java.util.Optional.orElseThrow(Optional.java:403)",
        "net.minecraft.core.HolderGetter$Provider.getOrThrow(HolderGetter.java:39)",
        "net.minecraft.world.item.Item$Properties.lambda$hoe$0(Item.java:296)",
        "net.minecraft.core.component.DataComponentInitializers$SingleComponentInitializer.lambda$asInitializer$0(DataComponentInitializers.java:134)",
    ],
    &[
        "net.minecraft.core.HolderGetter$Provider.lambda$getOrThrow$3(HolderGetter.java:47)",
        "java.base/java.util.Optional.orElseThrow(Optional.java:403)",
        "net.minecraft.core.HolderGetter$Provider.getOrThrow(HolderGetter.java:47)",
        "net.minecraft.world.item.Items.lambda$static$294(Items.java:1189)",
        "net.minecraft.core.component.DataComponentInitializers$SingleComponentInitializer.lambda$asInitializer$0(DataComponentInitializers.java:134)",
    ],
    &[
        "net.minecraft.core.HolderGetter$Provider.lambda$getOrThrow$3(HolderGetter.java:47)",
        "java.base/java.util.Optional.orElseThrow(Optional.java:403)",
        "net.minecraft.core.HolderGetter$Provider.getOrThrow(HolderGetter.java:47)",
        "net.minecraft.world.item.Items.lambda$static$298(Items.java:1236)",
        "net.minecraft.core.component.DataComponentInitializers$SingleComponentInitializer.lambda$asInitializer$0(DataComponentInitializers.java:134)",
    ],
    &[
        "net.minecraft.core.HolderGetter$Provider.lambda$getOrThrow$1(HolderGetter.java:39)",
        "java.base/java.util.Optional.orElseThrow(Optional.java:403)",
        "net.minecraft.core.HolderGetter$Provider.getOrThrow(HolderGetter.java:39)",
        "net.minecraft.world.item.Item$Properties.lambda$jukeboxPlayable$0(Item.java:252)",
        "net.minecraft.core.component.DataComponentInitializers$SingleComponentInitializer.lambda$asInitializer$0(DataComponentInitializers.java:134)",
    ],
    &[
        "net.minecraft.core.HolderGetter$Provider.lambda$getOrThrow$3(HolderGetter.java:47)",
        "java.base/java.util.Optional.orElseThrow(Optional.java:403)",
        "net.minecraft.core.HolderGetter$Provider.getOrThrow(HolderGetter.java:47)",
        "net.minecraft.world.item.Items.lambda$static$300(Items.java:1288)",
        "net.minecraft.core.component.DataComponentInitializers$SingleComponentInitializer.lambda$asInitializer$0(DataComponentInitializers.java:134)",
    ],
    &[
        "net.minecraft.core.HolderGetter$Provider.lambda$getOrThrow$3(HolderGetter.java:47)",
        "java.base/java.util.Optional.orElseThrow(Optional.java:403)",
        "net.minecraft.core.HolderGetter$Provider.getOrThrow(HolderGetter.java:47)",
        "net.minecraft.world.item.Items.lambda$static$301(Items.java:1289)",
        "net.minecraft.core.component.DataComponentInitializers$SingleComponentInitializer.lambda$asInitializer$0(DataComponentInitializers.java:134)",
    ],
    &[
        "net.minecraft.core.HolderGetter$Provider.lambda$getOrThrow$3(HolderGetter.java:47)",
        "java.base/java.util.Optional.orElseThrow(Optional.java:403)",
        "net.minecraft.core.HolderGetter$Provider.getOrThrow(HolderGetter.java:47)",
        "net.minecraft.world.item.Items.lambda$static$302(Items.java:1290)",
        "net.minecraft.core.component.DataComponentInitializers$SingleComponentInitializer.lambda$asInitializer$0(DataComponentInitializers.java:134)",
    ],
    &[
        "net.minecraft.core.HolderGetter$Provider.lambda$getOrThrow$3(HolderGetter.java:47)",
        "java.base/java.util.Optional.orElseThrow(Optional.java:403)",
        "net.minecraft.core.HolderGetter$Provider.getOrThrow(HolderGetter.java:47)",
        "net.minecraft.world.item.Items.lambda$static$303(Items.java:1291)",
        "net.minecraft.core.component.DataComponentInitializers$SingleComponentInitializer.lambda$asInitializer$0(DataComponentInitializers.java:134)",
    ],
    &[
        "net.minecraft.core.HolderGetter$Provider.lambda$getOrThrow$3(HolderGetter.java:47)",
        "java.base/java.util.Optional.orElseThrow(Optional.java:403)",
        "net.minecraft.core.HolderGetter$Provider.getOrThrow(HolderGetter.java:47)",
        "net.minecraft.world.item.Items.lambda$static$304(Items.java:1292)",
        "net.minecraft.core.component.DataComponentInitializers$SingleComponentInitializer.lambda$asInitializer$0(DataComponentInitializers.java:134)",
    ],
    &[
        "net.minecraft.core.HolderGetter$Provider.lambda$getOrThrow$3(HolderGetter.java:47)",
        "java.base/java.util.Optional.orElseThrow(Optional.java:403)",
        "net.minecraft.core.HolderGetter$Provider.getOrThrow(HolderGetter.java:47)",
        "net.minecraft.world.item.Items.lambda$static$305(Items.java:1293)",
        "net.minecraft.core.component.DataComponentInitializers$SingleComponentInitializer.lambda$asInitializer$0(DataComponentInitializers.java:134)",
    ],
    &[
        "net.minecraft.core.HolderGetter$Provider.lambda$getOrThrow$3(HolderGetter.java:47)",
        "java.base/java.util.Optional.orElseThrow(Optional.java:403)",
        "net.minecraft.core.HolderGetter$Provider.getOrThrow(HolderGetter.java:47)",
        "net.minecraft.world.item.Items.lambda$static$306(Items.java:1294)",
        "net.minecraft.core.component.DataComponentInitializers$SingleComponentInitializer.lambda$asInitializer$0(DataComponentInitializers.java:134)",
    ],
    &[
        "net.minecraft.core.HolderGetter$Provider.lambda$getOrThrow$3(HolderGetter.java:47)",
        "java.base/java.util.Optional.orElseThrow(Optional.java:403)",
        "net.minecraft.core.HolderGetter$Provider.getOrThrow(HolderGetter.java:47)",
        "net.minecraft.world.item.Items.lambda$static$307(Items.java:1295)",
        "net.minecraft.core.component.DataComponentInitializers$SingleComponentInitializer.lambda$asInitializer$0(DataComponentInitializers.java:134)",
    ],
    &[
        "net.minecraft.core.HolderGetter$Provider.lambda$getOrThrow$3(HolderGetter.java:47)",
        "java.base/java.util.Optional.orElseThrow(Optional.java:403)",
        "net.minecraft.core.HolderGetter$Provider.getOrThrow(HolderGetter.java:47)",
        "net.minecraft.world.item.Items.lambda$static$308(Items.java:1296)",
        "net.minecraft.core.component.DataComponentInitializers$SingleComponentInitializer.lambda$asInitializer$0(DataComponentInitializers.java:134)",
    ],
    &[
        "net.minecraft.core.HolderGetter$Provider.lambda$getOrThrow$3(HolderGetter.java:47)",
        "java.base/java.util.Optional.orElseThrow(Optional.java:403)",
        "net.minecraft.core.HolderGetter$Provider.getOrThrow(HolderGetter.java:47)",
        "net.minecraft.world.item.Items.lambda$static$309(Items.java:1297)",
        "net.minecraft.core.component.DataComponentInitializers$SingleComponentInitializer.lambda$asInitializer$0(DataComponentInitializers.java:134)",
    ],
    &[
        "net.minecraft.core.HolderGetter$Provider.lambda$getOrThrow$1(HolderGetter.java:39)",
        "java.base/java.util.Optional.orElseThrow(Optional.java:403)",
        "net.minecraft.core.HolderGetter$Provider.getOrThrow(HolderGetter.java:39)",
        "net.minecraft.world.item.Items.lambda$static$310(Items.java:1298)",
        "net.minecraft.core.component.DataComponentInitializers$SingleComponentInitializer.lambda$asInitializer$0(DataComponentInitializers.java:134)",
    ],
    &[
        "net.minecraft.core.HolderGetter$Provider.lambda$getOrThrow$1(HolderGetter.java:39)",
        "java.base/java.util.Optional.orElseThrow(Optional.java:403)",
        "net.minecraft.core.HolderGetter$Provider.getOrThrow(HolderGetter.java:39)",
        "net.minecraft.world.item.Item$Properties.lambda$potPattern$0(Item.java:260)",
        "net.minecraft.core.component.DataComponentInitializers$SingleComponentInitializer.lambda$asInitializer$0(DataComponentInitializers.java:134)",
    ],
];

const LOOKUPS: [(&str, bool, &str, &str, usize, usize); 116] = [
    ("minecraft:ancient_debris", true, "minecraft:damage_type", "minecraft:is_fire", 0, 1),
    ("minecraft:netherite_block", true, "minecraft:damage_type", "minecraft:is_fire", 0, 1),
    ("minecraft:redstone", false, "minecraft:trim_material", "minecraft:redstone", 1, 1),
    ("minecraft:diamond", false, "minecraft:trim_material", "minecraft:diamond", 1, 1),
    ("minecraft:emerald", false, "minecraft:trim_material", "minecraft:emerald", 1, 1),
    ("minecraft:lapis_lazuli", false, "minecraft:trim_material", "minecraft:lapis", 1, 1),
    ("minecraft:quartz", false, "minecraft:trim_material", "minecraft:quartz", 1, 1),
    ("minecraft:amethyst_shard", false, "minecraft:trim_material", "minecraft:amethyst", 1, 1),
    ("minecraft:iron_ingot", false, "minecraft:trim_material", "minecraft:iron", 1, 1),
    ("minecraft:copper_ingot", false, "minecraft:trim_material", "minecraft:copper", 1, 1),
    ("minecraft:gold_ingot", false, "minecraft:trim_material", "minecraft:gold", 1, 1),
    ("minecraft:netherite_ingot", true, "minecraft:damage_type", "minecraft:is_fire", 0, 2),
    ("minecraft:netherite_ingot", false, "minecraft:trim_material", "minecraft:netherite", 1, 1),
    ("minecraft:netherite_scrap", true, "minecraft:damage_type", "minecraft:is_fire", 0, 1),
    ("minecraft:wooden_shovel", false, "minecraft:block_transformer", "minecraft:shovel", 2, 2),
    ("minecraft:wooden_axe", false, "minecraft:block_transformer", "minecraft:axe", 3, 2),
    ("minecraft:wooden_hoe", false, "minecraft:block_transformer", "minecraft:hoe", 4, 2),
    ("minecraft:copper_shovel", false, "minecraft:block_transformer", "minecraft:shovel", 2, 1),
    ("minecraft:copper_axe", false, "minecraft:block_transformer", "minecraft:axe", 3, 1),
    ("minecraft:copper_hoe", false, "minecraft:block_transformer", "minecraft:hoe", 4, 1),
    ("minecraft:stone_shovel", false, "minecraft:block_transformer", "minecraft:shovel", 2, 1),
    ("minecraft:stone_axe", false, "minecraft:block_transformer", "minecraft:axe", 3, 1),
    ("minecraft:stone_hoe", false, "minecraft:block_transformer", "minecraft:hoe", 4, 1),
    ("minecraft:golden_shovel", false, "minecraft:block_transformer", "minecraft:shovel", 2, 1),
    ("minecraft:golden_axe", false, "minecraft:block_transformer", "minecraft:axe", 3, 1),
    ("minecraft:golden_hoe", false, "minecraft:block_transformer", "minecraft:hoe", 4, 1),
    ("minecraft:iron_shovel", false, "minecraft:block_transformer", "minecraft:shovel", 2, 1),
    ("minecraft:iron_axe", false, "minecraft:block_transformer", "minecraft:axe", 3, 1),
    ("minecraft:iron_hoe", false, "minecraft:block_transformer", "minecraft:hoe", 4, 1),
    ("minecraft:diamond_shovel", false, "minecraft:block_transformer", "minecraft:shovel", 2, 1),
    ("minecraft:diamond_axe", false, "minecraft:block_transformer", "minecraft:axe", 3, 1),
    ("minecraft:diamond_hoe", false, "minecraft:block_transformer", "minecraft:hoe", 4, 1),
    ("minecraft:netherite_sword", true, "minecraft:damage_type", "minecraft:is_fire", 0, 1),
    ("minecraft:netherite_shovel", false, "minecraft:block_transformer", "minecraft:shovel", 2, 2),
    ("minecraft:netherite_shovel", true, "minecraft:damage_type", "minecraft:is_fire", 0, 1),
    ("minecraft:netherite_pickaxe", true, "minecraft:damage_type", "minecraft:is_fire", 0, 1),
    ("minecraft:netherite_axe", false, "minecraft:block_transformer", "minecraft:axe", 3, 2),
    ("minecraft:netherite_axe", true, "minecraft:damage_type", "minecraft:is_fire", 0, 1),
    ("minecraft:netherite_hoe", false, "minecraft:block_transformer", "minecraft:hoe", 4, 2),
    ("minecraft:netherite_hoe", true, "minecraft:damage_type", "minecraft:is_fire", 0, 1),
    ("minecraft:netherite_helmet", true, "minecraft:damage_type", "minecraft:is_fire", 0, 1),
    ("minecraft:netherite_chestplate", true, "minecraft:damage_type", "minecraft:is_fire", 0, 1),
    ("minecraft:netherite_leggings", true, "minecraft:damage_type", "minecraft:is_fire", 0, 1),
    ("minecraft:netherite_boots", true, "minecraft:damage_type", "minecraft:is_fire", 0, 1),
    ("minecraft:egg", false, "minecraft:chicken_variant", "minecraft:temperate", 1, 1),
    ("minecraft:blue_egg", false, "minecraft:chicken_variant", "minecraft:cold", 1, 1),
    ("minecraft:brown_egg", false, "minecraft:chicken_variant", "minecraft:warm", 1, 1),
    ("minecraft:nether_star", true, "minecraft:damage_type", "minecraft:is_explosion", 5, 1),
    ("minecraft:resin_brick", false, "minecraft:trim_material", "minecraft:resin", 1, 1),
    ("minecraft:netherite_horse_armor", true, "minecraft:damage_type", "minecraft:is_fire", 0, 1),
    ("minecraft:shield", true, "minecraft:damage_type", "minecraft:bypasses_shield", 6, 2),
    ("minecraft:wooden_spear", false, "minecraft:damage_type", "minecraft:spear", 1, 9),
    ("minecraft:stone_spear", false, "minecraft:damage_type", "minecraft:spear", 1, 9),
    ("minecraft:copper_spear", false, "minecraft:damage_type", "minecraft:spear", 1, 9),
    ("minecraft:iron_spear", false, "minecraft:damage_type", "minecraft:spear", 1, 9),
    ("minecraft:golden_spear", false, "minecraft:damage_type", "minecraft:spear", 1, 9),
    ("minecraft:diamond_spear", false, "minecraft:damage_type", "minecraft:spear", 1, 9),
    ("minecraft:netherite_spear", false, "minecraft:damage_type", "minecraft:spear", 1, 10),
    ("minecraft:netherite_spear", true, "minecraft:damage_type", "minecraft:is_fire", 0, 1),
    ("minecraft:music_disc_13", false, "minecraft:jukebox_song", "minecraft:13", 7, 1),
    ("minecraft:music_disc_cat", false, "minecraft:jukebox_song", "minecraft:cat", 7, 1),
    ("minecraft:music_disc_blocks", false, "minecraft:jukebox_song", "minecraft:blocks", 7, 1),
    ("minecraft:music_disc_bounce", false, "minecraft:jukebox_song", "minecraft:bounce", 7, 1),
    ("minecraft:music_disc_chirp", false, "minecraft:jukebox_song", "minecraft:chirp", 7, 1),
    ("minecraft:music_disc_creator", false, "minecraft:jukebox_song", "minecraft:creator", 7, 1),
    ("minecraft:music_disc_creator_music_box", false, "minecraft:jukebox_song", "minecraft:creator_music_box", 7, 1),
    ("minecraft:music_disc_far", false, "minecraft:jukebox_song", "minecraft:far", 7, 1),
    ("minecraft:music_disc_lava_chicken", false, "minecraft:jukebox_song", "minecraft:lava_chicken", 7, 1),
    ("minecraft:music_disc_mall", false, "minecraft:jukebox_song", "minecraft:mall", 7, 1),
    ("minecraft:music_disc_mellohi", false, "minecraft:jukebox_song", "minecraft:mellohi", 7, 1),
    ("minecraft:music_disc_stal", false, "minecraft:jukebox_song", "minecraft:stal", 7, 1),
    ("minecraft:music_disc_strad", false, "minecraft:jukebox_song", "minecraft:strad", 7, 1),
    ("minecraft:music_disc_ward", false, "minecraft:jukebox_song", "minecraft:ward", 7, 1),
    ("minecraft:music_disc_11", false, "minecraft:jukebox_song", "minecraft:11", 7, 1),
    ("minecraft:music_disc_wait", false, "minecraft:jukebox_song", "minecraft:wait", 7, 1),
    ("minecraft:music_disc_otherside", false, "minecraft:jukebox_song", "minecraft:otherside", 7, 1),
    ("minecraft:music_disc_relic", false, "minecraft:jukebox_song", "minecraft:relic", 7, 1),
    ("minecraft:music_disc_5", false, "minecraft:jukebox_song", "minecraft:5", 7, 1),
    ("minecraft:music_disc_pigstep", false, "minecraft:jukebox_song", "minecraft:pigstep", 7, 1),
    ("minecraft:music_disc_precipice", false, "minecraft:jukebox_song", "minecraft:precipice", 7, 1),
    ("minecraft:music_disc_tears", false, "minecraft:jukebox_song", "minecraft:tears", 7, 1),
    ("minecraft:netherite_nautilus_armor", true, "minecraft:damage_type", "minecraft:is_fire", 0, 1),
    ("minecraft:flower_banner_pattern", true, "minecraft:banner_pattern", "minecraft:pattern_item/flower", 8, 1),
    ("minecraft:creeper_banner_pattern", true, "minecraft:banner_pattern", "minecraft:pattern_item/creeper", 9, 1),
    ("minecraft:skull_banner_pattern", true, "minecraft:banner_pattern", "minecraft:pattern_item/skull", 10, 1),
    ("minecraft:mojang_banner_pattern", true, "minecraft:banner_pattern", "minecraft:pattern_item/mojang", 11, 1),
    ("minecraft:globe_banner_pattern", true, "minecraft:banner_pattern", "minecraft:pattern_item/globe", 12, 1),
    ("minecraft:piglin_banner_pattern", true, "minecraft:banner_pattern", "minecraft:pattern_item/piglin", 13, 1),
    ("minecraft:flow_banner_pattern", true, "minecraft:banner_pattern", "minecraft:pattern_item/flow", 14, 1),
    ("minecraft:guster_banner_pattern", true, "minecraft:banner_pattern", "minecraft:pattern_item/guster", 15, 1),
    ("minecraft:field_masoned_banner_pattern", true, "minecraft:banner_pattern", "minecraft:pattern_item/field_masoned", 16, 1),
    ("minecraft:bordure_indented_banner_pattern", true, "minecraft:banner_pattern", "minecraft:pattern_item/bordure_indented", 17, 1),
    ("minecraft:goat_horn", false, "minecraft:instrument", "minecraft:ponder_goat_horn", 18, 1),
    ("minecraft:angler_pottery_sherd", false, "minecraft:decorated_pot_pattern", "minecraft:angler", 19, 1),
    ("minecraft:archer_pottery_sherd", false, "minecraft:decorated_pot_pattern", "minecraft:archer", 19, 1),
    ("minecraft:arms_up_pottery_sherd", false, "minecraft:decorated_pot_pattern", "minecraft:arms_up", 19, 1),
    ("minecraft:blade_pottery_sherd", false, "minecraft:decorated_pot_pattern", "minecraft:blade", 19, 1),
    ("minecraft:brewer_pottery_sherd", false, "minecraft:decorated_pot_pattern", "minecraft:brewer", 19, 1),
    ("minecraft:burn_pottery_sherd", false, "minecraft:decorated_pot_pattern", "minecraft:burn", 19, 1),
    ("minecraft:danger_pottery_sherd", false, "minecraft:decorated_pot_pattern", "minecraft:danger", 19, 1),
    ("minecraft:explorer_pottery_sherd", false, "minecraft:decorated_pot_pattern", "minecraft:explorer", 19, 1),
    ("minecraft:flow_pottery_sherd", false, "minecraft:decorated_pot_pattern", "minecraft:flow", 19, 1),
    ("minecraft:friend_pottery_sherd", false, "minecraft:decorated_pot_pattern", "minecraft:friend", 19, 1),
    ("minecraft:guster_pottery_sherd", false, "minecraft:decorated_pot_pattern", "minecraft:guster", 19, 1),
    ("minecraft:heart_pottery_sherd", false, "minecraft:decorated_pot_pattern", "minecraft:heart", 19, 1),
    ("minecraft:heartbreak_pottery_sherd", false, "minecraft:decorated_pot_pattern", "minecraft:heartbreak", 19, 1),
    ("minecraft:howl_pottery_sherd", false, "minecraft:decorated_pot_pattern", "minecraft:howl", 19, 1),
    ("minecraft:miner_pottery_sherd", false, "minecraft:decorated_pot_pattern", "minecraft:miner", 19, 1),
    ("minecraft:mourner_pottery_sherd", false, "minecraft:decorated_pot_pattern", "minecraft:mourner", 19, 1),
    ("minecraft:plenty_pottery_sherd", false, "minecraft:decorated_pot_pattern", "minecraft:plenty", 19, 1),
    ("minecraft:prize_pottery_sherd", false, "minecraft:decorated_pot_pattern", "minecraft:prize", 19, 1),
    ("minecraft:scrape_pottery_sherd", false, "minecraft:decorated_pot_pattern", "minecraft:scrape", 19, 1),
    ("minecraft:sheaf_pottery_sherd", false, "minecraft:decorated_pot_pattern", "minecraft:sheaf", 19, 1),
    ("minecraft:shelter_pottery_sherd", false, "minecraft:decorated_pot_pattern", "minecraft:shelter", 19, 1),
    ("minecraft:skull_pottery_sherd", false, "minecraft:decorated_pot_pattern", "minecraft:skull", 19, 1),
    ("minecraft:snort_pottery_sherd", false, "minecraft:decorated_pot_pattern", "minecraft:snort", 19, 1),
];

pub fn build() -> Result<(), JavaThrowable> {
    build_with(|is_tag, registry, id| if is_tag { tag_exists(registry, id) } else { registry_contains(registry, id) })
}

fn build_with(present: impl Fn(bool, &str, &str) -> bool) -> Result<(), JavaThrowable> {
    for &(_, is_tag, registry, id, head, wrappers) in &LOOKUPS {
        if present(is_tag, registry, id) {
            continue;
        }
        let message = if is_tag { format!("Missing tag TagKey[{registry} / {id}]") } else { format!("Missing element ResourceKey[{registry} / {id}]") };
        let mut frames = HEADS[head].to_vec();
        frames.push(AND_THEN_OTHER);
        frames.extend(std::iter::repeat_n(AND_THEN_THIS, wrappers));
        frames.extend([INITIALIZER_ENTRY_RUN, RUN_INITIALIZERS, BUILD, LOAD_RESOURCES_LAMBDA]);
        frames.extend(ASYNC_SUPPLY);
        frames.extend(FORK_JOIN_TAIL);
        return Err(JavaThrowable::new(format!("java.lang.IllegalStateException: {message}"), frames));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::build_with;

    #[test]
    fn nothing_is_missing_in_a_full_context() {
        assert!(build_with(|_, _, _| true).is_ok());
    }

    #[test]
    fn a_missing_jukebox_song_fails_like_vanilla() {
        let trace = build_with(|_, registry, id| !(registry == "minecraft:jukebox_song" && id == "minecraft:11")).unwrap_err().print_stack_trace();
        let expected = "java.lang.IllegalStateException: Missing element ResourceKey[minecraft:jukebox_song / minecraft:11]
\tat net.minecraft.core.HolderGetter$Provider.lambda$getOrThrow$1(HolderGetter.java:39)
\tat java.base/java.util.Optional.orElseThrow(Optional.java:403)
\tat net.minecraft.core.HolderGetter$Provider.getOrThrow(HolderGetter.java:39)
\tat net.minecraft.world.item.Item$Properties.lambda$jukeboxPlayable$0(Item.java:252)
\tat net.minecraft.core.component.DataComponentInitializers$SingleComponentInitializer.lambda$asInitializer$0(DataComponentInitializers.java:134)
\tat net.minecraft.core.component.DataComponentInitializers$Initializer.lambda$andThen$0(DataComponentInitializers.java:120)
\tat net.minecraft.core.component.DataComponentInitializers$Initializer.lambda$andThen$0(DataComponentInitializers.java:119)
\tat net.minecraft.core.component.DataComponentInitializers$InitializerEntry.run(DataComponentInitializers.java:100)
\tat net.minecraft.core.component.DataComponentInitializers.runInitializers(DataComponentInitializers.java:29)
\tat net.minecraft.core.component.DataComponentInitializers.build(DataComponentInitializers.java:52)
\tat net.minecraft.server.ReloadableServerResources.lambda$loadResources$1(ReloadableServerResources.java:84)
\tat java.base/java.util.concurrent.CompletableFuture$AsyncSupply.run(CompletableFuture.java:1789)
\tat java.base/java.util.concurrent.CompletableFuture$AsyncSupply.exec(CompletableFuture.java:1781)
\tat java.base/java.util.concurrent.ForkJoinTask.doExec(ForkJoinTask.java:511)
\tat java.base/java.util.concurrent.ForkJoinPool$WorkQueue.topLevelExec(ForkJoinPool.java:1450)
\tat java.base/java.util.concurrent.ForkJoinPool.runWorker(ForkJoinPool.java:2019)
\tat java.base/java.util.concurrent.ForkJoinWorkerThread.run(ForkJoinWorkerThread.java:187)
";
        assert_eq!(trace, expected);
    }

    #[test]
    fn the_first_missing_lookup_in_registration_order_wins() {
        let error = build_with(|_, registry, _| registry != "minecraft:jukebox_song" && registry != "minecraft:decorated_pot_pattern").unwrap_err();
        assert_eq!(error.description, "java.lang.IllegalStateException: Missing element ResourceKey[minecraft:jukebox_song / minecraft:13]");
    }

    #[test]
    fn a_missing_tag_reads_tag_key() {
        let error = build_with(|is_tag, _, id| !(is_tag && id == "minecraft:is_fire")).unwrap_err();
        assert_eq!(error.description, "java.lang.IllegalStateException: Missing tag TagKey[minecraft:damage_type / minecraft:is_fire]");
        assert!(error.stack_trace[3].contains("lambda$fireResistant$0"));
    }
}
