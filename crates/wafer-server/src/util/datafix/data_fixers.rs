use std::sync::OnceLock;

use crate::datafixers::data_fixer_upper::{DataFixerBuilder, DataFixerUpper};
use crate::util::datafix::fixes::add_field_fix::AddFieldFix;
use crate::util::datafix::fixes::add_new_choices::AddNewChoices;
use crate::util::datafix::fixes::advancements_rename_fix::AdvancementsRenameFix;
use crate::util::datafix::fixes::area_effect_cloud_duration_scale_fix::AreaEffectCloudDurationScaleFix;
use crate::util::datafix::fixes::area_effect_cloud_potion_fix::AreaEffectCloudPotionFix;
use crate::util::datafix::fixes::attribute_id_prefix_fix::AttributeIdPrefixFix;
use crate::util::datafix::fixes::attribute_modifier_id_fix::AttributeModifierIdFix;
use crate::util::datafix::fixes::attributes_rename_legacy::AttributesRenameLegacy;
use crate::util::datafix::fixes::banner_entity_custom_name_to_override_component_fix::BannerEntityCustomNameToOverrideComponentFix;
use crate::util::datafix::fixes::banner_pattern_format_fix::BannerPatternFormatFix;
use crate::util::datafix::fixes::beehive_field_rename_fix::BeehiveFieldRenameFix;
use crate::util::datafix::fixes::blending_data_fix::BlendingDataFix;
use crate::util::datafix::fixes::blending_data_no_value_height_fix::BlendingDataNoValueHeightFix;
use crate::util::datafix::fixes::blending_data_remove_from_nether_end_fix::BlendingDataRemoveFromNetherEndFix;
use crate::util::datafix::fixes::block_entity_furnace_burn_time_fix::BlockEntityFurnaceBurnTimeFix;
use crate::util::datafix::fixes::block_entity_id_fix::BlockEntityIdFix;
use crate::util::datafix::fixes::block_entity_rename_fix::BlockEntityRenameFix;
use crate::util::datafix::fixes::block_entity_shulker_box_color_fix::BlockEntityShulkerBoxColorFix;
use crate::util::datafix::fixes::block_entity_sign_double_sided_editable_text_fix::BlockEntitySignDoubleSidedEditableTextFix;
use crate::util::datafix::fixes::block_pos_format_and_renames_fix::BlockPosFormatAndRenamesFix;
use crate::util::datafix::fixes::block_state_field_names_fix::BlockStateFieldNamesFix;
use crate::util::datafix::fixes::boat_split_fix::BoatSplitFix;
use crate::util::datafix::fixes::chested_horses_inventory_zero_indexing_fix::ChestedHorsesInventoryZeroIndexingFix;
use crate::util::datafix::fixes::chunk_delete_ignored_light_data_fix::ChunkDeleteIgnoredLightDataFix;
use crate::util::datafix::fixes::chunk_delete_light_fix::ChunkDeleteLightFix;
use crate::util::datafix::fixes::chunk_ticket_unpack_pos_fix::ChunkTicketUnpackPosFix;
use crate::util::datafix::fixes::container_block_entity_lock_predicate_fix::ContainerBlockEntityLockPredicateFix;
use crate::util::datafix::fixes::copper_golem_weather_state_fix::CopperGolemWeatherStateFix;
use crate::util::datafix::fixes::criteria_rename_fix::CriteriaRenameFix;
use crate::util::datafix::fixes::custom_model_data_expand_fix::CustomModelDataExpandFix;
use crate::util::datafix::fixes::day_time_to_clock_fix::DayTimeToClockFix;
use crate::util::datafix::fixes::debug_profile_looking_at_split_fix::DebugProfileLookingAtSplitFix;
use crate::util::datafix::fixes::debug_profile_overlay_reference_fix::DebugProfileOverlayReferenceFix;
use crate::util::datafix::fixes::debug_profile_rename_entry_fix::DebugProfileRenameEntryFix;
use crate::util::datafix::fixes::decorated_pot_field_rename_fix::DecoratedPotFieldRenameFix;
use crate::util::datafix::fixes::drop_chances_format_fix::DropChancesFormatFix;
use crate::util::datafix::fixes::drop_invalid_sign_data_fix::DropInvalidSignDataFix;
use crate::util::datafix::fixes::effect_duration_fix::EffectDurationFix;
use crate::util::datafix::fixes::empty_item_in_hotbar_fix::EmptyItemInHotbarFix;
use crate::util::datafix::fixes::empty_item_in_villager_trade_fix::EmptyItemInVillagerTradeFix;
use crate::util::datafix::fixes::entity_armor_stand_silent_fix::EntityArmorStandSilentFix;
use crate::util::datafix::fixes::entity_attribute_base_value_fix::EntityAttributeBaseValueFix;
use crate::util::datafix::fixes::entity_brushable_block_fields_rename_fix::EntityBrushableBlockFieldsRenameFix;
use crate::util::datafix::fixes::entity_elder_guardian_split_fix::EntityElderGuardianSplitFix;
use crate::util::datafix::fixes::entity_equipment_to_armor_and_hand_fix::EntityEquipmentToArmorAndHandFix;
use crate::util::datafix::fixes::entity_fall_distance_float_to_double_fix::EntityFallDistanceFloatToDoubleFix;
use crate::util::datafix::fixes::entity_fields_rename_fix::EntityFieldsRenameFix;
use crate::util::datafix::fixes::entity_goat_missing_state_fix::EntityGoatMissingStateFix;
use crate::util::datafix::fixes::entity_health_fix::EntityHealthFix;
use crate::util::datafix::fixes::entity_horse_saddle_fix::EntityHorseSaddleFix;
use crate::util::datafix::fixes::entity_horse_split_fix::EntityHorseSplitFix;
use crate::util::datafix::fixes::entity_id_fix::EntityIdFix;
use crate::util::datafix::fixes::entity_minecart_identifiers_fix::EntityMinecartIdentifiersFix;
use crate::util::datafix::fixes::entity_painting_item_frame_direction_fix::EntityPaintingItemFrameDirectionFix;
use crate::util::datafix::fixes::entity_redundant_chance_tags_fix::EntityRedundantChanceTagsFix;
use crate::util::datafix::fixes::entity_riding_to_passengers_fix::EntityRidingToPassengersFix;
use crate::util::datafix::fixes::entity_salmon_size_fix::EntitySalmonSizeFix;
use crate::util::datafix::fixes::entity_shulker_color_fix::EntityShulkerColorFix;
use crate::util::datafix::fixes::entity_skeleton_split_fix::EntitySkeletonSplitFix;
use crate::util::datafix::fixes::entity_spawner_item_variant_component_fix::EntitySpawnerItemVariantComponentFix;
use crate::util::datafix::fixes::entity_string_uuid_fix::EntityStringUuidFix;
use crate::util::datafix::fixes::entity_tipped_arrow_fix::EntityTippedArrowFix;
use crate::util::datafix::fixes::entity_variant_fix::EntityVariantFix;
use crate::util::datafix::fixes::entity_zombie_split_fix::EntityZombieSplitFix;
use crate::util::datafix::fixes::entity_zombie_villager_type_fix::EntityZombieVillagerTypeFix;
use crate::util::datafix::fixes::equipment_format_fix::EquipmentFormatFix;
use crate::util::datafix::fixes::equippable_asset_rename_fix::EquippableAssetRenameFix;
use crate::util::datafix::fixes::explorer_map_item_fix::ExplorerMapItemFix;
use crate::util::datafix::fixes::feature_flag_remove_fix::FeatureFlagRemoveFix;
use crate::util::datafix::fixes::filtered_books_fix::FilteredBooksFix;
use crate::util::datafix::fixes::filtered_signs_fix::FilteredSignsFix;
use crate::util::datafix::fixes::fire_resistant_to_damage_resistant_component_fix::FireResistantToDamageResistantComponentFix;
use crate::util::datafix::fixes::fix_projectile_stored_item::FixProjectileStoredItem;
use crate::util::datafix::fixes::fix_wolf_health::FixWolfHealth;
use crate::util::datafix::fixes::food_to_consumable_fix::FoodToConsumableFix;
use crate::util::datafix::fixes::forced_chunk_to_ticket_fix::ForcedChunkToTicketFix;
use crate::util::datafix::fixes::game_rule_registry_fix::GameRuleRegistryFix;
use crate::util::datafix::fixes::goat_horn_id_fix::GoatHornIdFix;
use crate::util::datafix::fixes::horse_body_armor_item_fix::HorseBodyArmorItemFix;
use crate::util::datafix::fixes::inline_block_pos_format_fix::InlineBlockPosFormatFix;
use crate::util::datafix::fixes::invalid_block_entity_lock_fix::InvalidBlockEntityLockFix;
use crate::util::datafix::fixes::invalid_lock_component_fix::InvalidLockComponentFix;
use crate::util::datafix::fixes::item_banner_color_fix::ItemBannerColorFix;
use crate::util::datafix::fixes::item_id_fix::ItemIdFix;
use crate::util::datafix::fixes::item_potion_fix::ItemPotionFix;
use crate::util::datafix::fixes::item_rename_fix::ItemRenameFix;
use crate::util::datafix::fixes::item_shulker_box_color_fix::ItemShulkerBoxColorFix;
use crate::util::datafix::fixes::item_spawn_egg_fix::ItemSpawnEggFix;
use crate::util::datafix::fixes::item_stack_componentization_fix::ItemStackComponentizationFix;
use crate::util::datafix::fixes::item_stack_custom_name_to_override_component_fix::ItemStackCustomNameToOverrideComponentFix;
use crate::util::datafix::fixes::item_stack_spawn_egg_fix::ItemStackSpawnEggFix;
use crate::util::datafix::fixes::item_water_potion_fix::ItemWaterPotionFix;
use crate::util::datafix::fixes::jukebox_ticks_since_song_started_fix::JukeboxTicksSinceSongStartedFix;
use crate::util::datafix::fixes::legacy_block_property_rename_and_fix::LegacyBlockPropertyRenameAndFix;
use crate::util::datafix::fixes::legacy_block_rename_fix::LegacyBlockRenameFix;
use crate::util::datafix::fixes::legacy_dragon_fight_fix::LegacyDragonFightFix;
use crate::util::datafix::fixes::legacy_world_border_fix::LegacyWorldBorderFix;
use crate::util::datafix::fixes::level_dat_difficulty_fix::LevelDatDifficultyFix;
use crate::util::datafix::fixes::level_dat_to_saved_data_preparation_fix::LevelDatToSavedDataPreparationFix;
use crate::util::datafix::fixes::lock_component_predicate_fix::LockComponentPredicateFix;
use crate::util::datafix::fixes::lodestone_compass_component_fix::LodestoneCompassComponentFix;
use crate::util::datafix::fixes::map_banner_block_pos_format_fix::MapBannerBlockPosFormatFix;
use crate::util::datafix::fixes::map_color_component_removal_fix::MapColorComponentRemovalFix;
use crate::util::datafix::fixes::merge_terrain_chunk_status_fix::MergeTerrainChunkStatusFix;
use crate::util::datafix::fixes::mob_effect_id_fix::MobEffectIdFix;
use crate::util::datafix::fixes::mob_spawner_entity_identifiers_fix::MobSpawnerEntityIdentifiersFix;
use crate::util::datafix::fixes::named_entity_convert_unchecked_fix::NamedEntityConvertUncheckedFix;
use crate::util::datafix::fixes::namespaced_type_rename_fix::NamespacedTypeRenameFix;
use crate::util::datafix::fixes::options_accessibility_onboard_fix::OptionsAccessibilityOnboardFix;
use crate::util::datafix::fixes::options_ambient_occlusion_fix::OptionsAmbientOcclusionFix;
use crate::util::datafix::fixes::options_force_default_graphics_api_fix::OptionsForceDefaultGraphicsApiFix;
use crate::util::datafix::fixes::options_force_vbo_fix::OptionsForceVBOFix;
use crate::util::datafix::fixes::options_graphics_mode_split_fix::OptionsGraphicsModeSplitFix;
use crate::util::datafix::fixes::options_lower_case_language_fix::OptionsLowerCaseLanguageFix;
use crate::util::datafix::fixes::options_menu_blurriness_fix::OptionsMenuBlurrinessFix;
use crate::util::datafix::fixes::options_music_toast_fix::OptionsMusicToastFix;
use crate::util::datafix::fixes::options_programmer_art_fix::OptionsProgrammerArtFix;
use crate::util::datafix::fixes::options_set_graphics_preset_to_custom_fix::OptionsSetGraphicsPresetToCustomFix;
use crate::util::datafix::fixes::particle_unflattening_fix::ParticleUnflatteningFix;
use crate::util::datafix::fixes::player_equipment_fix::PlayerEquipmentFix;
use crate::util::datafix::fixes::player_head_block_profile_fix::PlayerHeadBlockProfileFix;
use crate::util::datafix::fixes::player_respawn_data_fix::PlayerRespawnDataFix;
use crate::util::datafix::fixes::poi_type_remove_fix::PoiTypeRemoveFix;
use crate::util::datafix::fixes::pot_decorations_block_entity_unflattening_fix::PotDecorationsBlockEntityUnflatteningFix;
use crate::util::datafix::fixes::pot_decorations_component_unflattening_fix::PotDecorationsComponentUnflatteningFix;
use crate::util::datafix::fixes::primed_tnt_block_state_fixer::PrimedTntBlockStateFixer;
use crate::util::datafix::fixes::projectile_stored_weapon_fix::ProjectileStoredWeaponFix;
use crate::util::datafix::fixes::raid_renames_data_fix::RaidRenamesDataFix;
use crate::util::datafix::fixes::random_sequence_settings_fix::RandomSequenceSettingsFix;
use crate::util::datafix::fixes::references::*;
use crate::util::datafix::fixes::remap_chunk_status_fix::RemapChunkStatusFix;
use crate::util::datafix::fixes::remove_block_entity_tag_fix::RemoveBlockEntityTagFix;
use crate::util::datafix::fixes::remove_block_transformer_component_fix::RemoveBlockTransformerComponentFix;
use crate::util::datafix::fixes::remove_empty_item_in_brushable_block_fix::RemoveEmptyItemInBrushableBlockFix;
use crate::util::datafix::fixes::rename_enchantments_fix::RenameEnchantmentsFix;
use crate::util::datafix::fixes::rename_nameplate_to_name_tag_fix::RenameNameplateToNameTagFix;
use crate::util::datafix::fixes::saddle_equipment_slot_fix::SaddleEquipmentSlotFix;
use crate::util::datafix::fixes::scoreboard_display_slot_fix::ScoreboardDisplaySlotFix;
use crate::util::datafix::fixes::sign_text_strict_json_fix::SignTextStrictJsonFix;
use crate::util::datafix::fixes::signs_op_flag_fix::SignsOpFlagFix;
use crate::util::datafix::fixes::stonecutter_recipe_rename_fix;
use crate::util::datafix::fixes::swing_animation_component_split_fix::SwingAnimationComponentSplitFix;
use crate::util::datafix::fixes::text_component_hover_and_click_event_fix::TextComponentHoverAndClickEventFix;
use crate::util::datafix::fixes::text_component_stringified_flags_fix::TextComponentStringifiedFlagsFix;
use crate::util::datafix::fixes::thrown_potion_split_fix::ThrownPotionSplitFix;
use crate::util::datafix::fixes::tipped_arrow_potion_to_item_fix::TippedArrowPotionToItemFix;
use crate::util::datafix::fixes::tooltip_display_component_fix::TooltipDisplayComponentFix;
use crate::util::datafix::fixes::trial_spawner_config_fix::TrialSpawnerConfigFix;
use crate::util::datafix::fixes::trial_spawner_config_in_registry_fix::TrialSpawnerConfigInRegistryFix;
use crate::util::datafix::fixes::trident_animation_fix::TridentAnimationFix;
use crate::util::datafix::fixes::unfilled_buried_treasure_map_fix::UnfilledBuriedTreasureMapFix;
use crate::util::datafix::fixes::variant_rename_fix::VariantRenameFix;
use crate::util::datafix::fixes::villager_set_can_pick_up_loot_fix::VillagerSetCanPickUpLootFix;
use crate::util::datafix::fixes::villager_set_villager_data_finalized::VillagerSetVillagerDataFinalized;
use crate::util::datafix::fixes::world_border_warning_time_fix::WorldBorderWarningTimeFix;
use crate::util::datafix::fixes::world_spawn_data_fix::WorldSpawnDataFix;
use crate::util::datafix::fixes::write_and_read_fix::WriteAndReadFix;
use crate::util::datafix::fixes::written_book_pages_strict_json_fix::WrittenBookPagesStrictJsonFix;
use crate::util::datafix::fixes::zombie_villager_set_villager_data_finalized::ZombieVillagerSetVillagerDataFinalized;
use crate::util::datafix::schemas::namespaced_schema::ensure_namespaced;
use crate::util::datafix::schemas::SAME;
use crate::util::datafix::schemas::namespaced_schema::NAMESPACED_SCHEMA;
use crate::util::datafix::schemas::v100::V100;
use crate::util::datafix::schemas::v1022::V1022;
use crate::util::datafix::schemas::v102::V102;
use crate::util::datafix::schemas::v106::V106;
use crate::util::datafix::schemas::v107::V107;
use crate::util::datafix::schemas::v1125::V1125;
use crate::util::datafix::schemas::v135::V135;
use crate::util::datafix::schemas::v143::V143;
use crate::util::datafix::schemas::v1451::V1451;
use crate::util::datafix::schemas::v1451_1::V1451_1;
use crate::util::datafix::schemas::v1451_2::V1451_2;
use crate::util::datafix::schemas::v1451_3::V1451_3;
use crate::util::datafix::schemas::v1451_4::V1451_4;
use crate::util::datafix::schemas::v1451_5::V1451_5;
use crate::util::datafix::schemas::v1451_6::V1451_6;
use crate::util::datafix::schemas::v1458::V1458;
use crate::util::datafix::schemas::v1460::V1460;
use crate::util::datafix::schemas::v1466::V1466;
use crate::util::datafix::schemas::v1470::V1470;
use crate::util::datafix::schemas::v1481::V1481;
use crate::util::datafix::schemas::v1483::V1483;
use crate::util::datafix::schemas::v1486::V1486;
use crate::util::datafix::schemas::v1488::V1488;
use crate::util::datafix::schemas::v1510::V1510;
use crate::util::datafix::schemas::v1800::V1800;
use crate::util::datafix::schemas::v1801::V1801;
use crate::util::datafix::schemas::v1904::V1904;
use crate::util::datafix::schemas::v1906::V1906;
use crate::util::datafix::schemas::v1909::V1909;
use crate::util::datafix::schemas::v1920::V1920;
use crate::util::datafix::schemas::v1928::V1928;
use crate::util::datafix::schemas::v1929::V1929;
use crate::util::datafix::schemas::v1931::V1931;
use crate::util::datafix::schemas::v2100::V2100;
use crate::util::datafix::schemas::v2501::V2501;
use crate::util::datafix::schemas::v2502::V2502;
use crate::util::datafix::schemas::v2505::V2505;
use crate::util::datafix::schemas::v2509::V2509;
use crate::util::datafix::schemas::v2511_1::V2511_1;
use crate::util::datafix::schemas::v2519::V2519;
use crate::util::datafix::schemas::v2522::V2522;
use crate::util::datafix::schemas::v2551::V2551;
use crate::util::datafix::schemas::v2568::V2568;
use crate::util::datafix::schemas::v2571::V2571;
use crate::util::datafix::schemas::v2684::V2684;
use crate::util::datafix::schemas::v2686::V2686;
use crate::util::datafix::schemas::v2688::V2688;
use crate::util::datafix::schemas::v2704::V2704;
use crate::util::datafix::schemas::v2707::V2707;
use crate::util::datafix::schemas::v2831::V2831;
use crate::util::datafix::schemas::v2832::V2832;
use crate::util::datafix::schemas::v2842::V2842;
use crate::util::datafix::schemas::v3076::V3076;
use crate::util::datafix::schemas::v3078::V3078;
use crate::util::datafix::schemas::v3081::V3081;
use crate::util::datafix::schemas::v3082::V3082;
use crate::util::datafix::schemas::v3083::V3083;
use crate::util::datafix::schemas::v3202::V3202;
use crate::util::datafix::schemas::v3203::V3203;
use crate::util::datafix::schemas::v3204::V3204;
use crate::util::datafix::schemas::v3325::V3325;
use crate::util::datafix::schemas::v3326::V3326;
use crate::util::datafix::schemas::v3327::V3327;
use crate::util::datafix::schemas::v3328::V3328;
use crate::util::datafix::schemas::v3438::V3438;
use crate::util::datafix::schemas::v3439::V3439;
use crate::util::datafix::schemas::v3439_1::V3439_1;
use crate::util::datafix::schemas::v3448::V3448;
use crate::util::datafix::schemas::v3682::V3682;
use crate::util::datafix::schemas::v3683::V3683;
use crate::util::datafix::schemas::v3685::V3685;
use crate::util::datafix::schemas::v3689::V3689;
use crate::util::datafix::schemas::v3799::V3799;
use crate::util::datafix::schemas::v3807::V3807;
use crate::util::datafix::schemas::v3808::V3808;
use crate::util::datafix::schemas::v3808_1::V3808_1;
use crate::util::datafix::schemas::v3808_2::V3808_2;
use crate::util::datafix::schemas::v3813::V3813;
use crate::util::datafix::schemas::v3816::V3816;
use crate::util::datafix::schemas::v3818::V3818;
use crate::util::datafix::schemas::v3818_3::V3818_3;
use crate::util::datafix::schemas::v3818_4::V3818_4;
use crate::util::datafix::schemas::v3818_5::V3818_5;
use crate::util::datafix::schemas::v3825::V3825;
use crate::util::datafix::schemas::v3938::V3938;
use crate::util::datafix::schemas::v4059::V4059;
use crate::util::datafix::schemas::v4067::V4067;
use crate::util::datafix::schemas::v4070::V4070;
use crate::util::datafix::schemas::v4071::V4071;
use crate::util::datafix::schemas::v4290::V4290;
use crate::util::datafix::schemas::v4292::V4292;
use crate::util::datafix::schemas::v4300::V4300;
use crate::util::datafix::schemas::v4301::V4301;
use crate::util::datafix::schemas::v4302::V4302;
use crate::util::datafix::schemas::v4306::V4306;
use crate::util::datafix::schemas::v4307::V4307;
use crate::util::datafix::schemas::v4312::V4312;
use crate::util::datafix::schemas::v4420::V4420;
use crate::util::datafix::schemas::v4421::V4421;
use crate::util::datafix::schemas::v4531::V4531;
use crate::util::datafix::schemas::v4532::V4532;
use crate::util::datafix::schemas::v4533::V4533;
use crate::util::datafix::schemas::v4543::V4543;
use crate::util::datafix::schemas::v4648::V4648;
use crate::util::datafix::schemas::v4656::V4656;
use crate::util::datafix::schemas::v4771::V4771;
use crate::util::datafix::schemas::v4881::V4881;
use crate::util::datafix::schemas::v4885::V4885;
use crate::util::datafix::schemas::v4996::V4996;
use crate::util::datafix::schemas::v4996_1::V4996_1;
use crate::util::datafix::schemas::v4997::V4997;
use crate::util::datafix::schemas::v5000::V5000;
use crate::util::datafix::schemas::v501::V501;
use crate::util::datafix::schemas::v700::V700;
use crate::util::datafix::schemas::v701::V701;
use crate::util::datafix::schemas::v702::V702;
use crate::util::datafix::schemas::v703::V703;
use crate::util::datafix::schemas::v704::V704;
use crate::util::datafix::schemas::v705::V705;
use crate::util::datafix::schemas::v808::V808;
use crate::util::datafix::schemas::v99::V99;

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
    let v101 = fixer_upper.add_schema(101, &SAME);
    fixer_upper.add_fixer(VillagerSetCanPickUpLootFix::new(&v101));
    let v102 = fixer_upper.add_schema(102, &V102);
    fixer_upper.add_fixer(ItemIdFix::new(&v102, true));
    fixer_upper.add_fixer(ItemPotionFix::new(&v102, false));
    let v105 = fixer_upper.add_schema(105, &SAME);
    fixer_upper.add_fixer(ItemSpawnEggFix::new(&v105, true));
    let v106 = fixer_upper.add_schema(106, &V106);
    fixer_upper.add_fixer(MobSpawnerEntityIdentifiersFix::new(&v106, true));
    let v107 = fixer_upper.add_schema(107, &V107);
    fixer_upper.add_fixer(EntityMinecartIdentifiersFix::new(&v107));
    let v108 = fixer_upper.add_schema(108, &SAME);
    fixer_upper.add_fixer(EntityStringUuidFix::new(&v108, true));
    let v109 = fixer_upper.add_schema(109, &SAME);
    fixer_upper.add_fixer(EntityHealthFix::new(&v109, true));
    let v110 = fixer_upper.add_schema(110, &SAME);
    fixer_upper.add_fixer(EntityHorseSaddleFix::new(&v110, true));
    let v111 = fixer_upper.add_schema(111, &SAME);
    fixer_upper.add_fixer(EntityPaintingItemFrameDirectionFix::new(&v111, true));
    let v113 = fixer_upper.add_schema(113, &SAME);
    fixer_upper.add_fixer(EntityRedundantChanceTagsFix::new(&v113, true));
    let v135 = fixer_upper.add_schema(135, &V135);
    fixer_upper.add_fixer(EntityRidingToPassengersFix::new(&v135, true));
    let v143 = fixer_upper.add_schema(143, &V143);
    fixer_upper.add_fixer(EntityTippedArrowFix::new(&v143, true));
    let v147 = fixer_upper.add_schema(147, &SAME);
    fixer_upper.add_fixer(EntityArmorStandSilentFix::new(&v147, true));
    let v165 = fixer_upper.add_schema(165, &SAME);
    fixer_upper.add_fixer(SignTextStrictJsonFix::new(&v165));
    fixer_upper.add_fixer(WrittenBookPagesStrictJsonFix::new(&v165));
    let v501 = fixer_upper.add_schema(501, &V501);
    fixer_upper.add_fixer(AddNewChoices::new(&v501, "Add 1.10 entities fix", ENTITY));
    let v502 = fixer_upper.add_schema(502, &SAME);
    fixer_upper.add_fixer(ItemRenameFix::create(&v502, "cooked_fished item renamer", create_renamer("minecraft:cooked_fished", "minecraft:cooked_fish")));
    fixer_upper.add_fixer(EntityZombieVillagerTypeFix::new(&v502, false));
    let v505 = fixer_upper.add_schema(505, &SAME);
    fixer_upper.add_fixer(OptionsForceVBOFix::new(&v505, false));
    let v700 = fixer_upper.add_schema(700, &V700);
    fixer_upper.add_fixer(EntityElderGuardianSplitFix::new(&v700, true));
    let v701 = fixer_upper.add_schema(701, &V701);
    fixer_upper.add_fixer(EntitySkeletonSplitFix::new(&v701, true));
    let v702 = fixer_upper.add_schema(702, &V702);
    fixer_upper.add_fixer(EntityZombieSplitFix::new(&v702));
    let v703 = fixer_upper.add_schema(703, &V703);
    fixer_upper.add_fixer(EntityHorseSplitFix::new(&v703, true));
    let v704 = fixer_upper.add_schema(704, &V704);
    fixer_upper.add_fixer(BlockEntityIdFix::new(&v704, true));
    let v705 = fixer_upper.add_schema(705, &V705);
    fixer_upper.add_fixer(EntityIdFix::new(&v705, true));
    let v804 = fixer_upper.add_schema(804, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(ItemBannerColorFix::new(&v804, true));
    let v806 = fixer_upper.add_schema(806, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(ItemWaterPotionFix::new(&v806, false));
    let v808 = fixer_upper.add_schema(808, &V808);
    fixer_upper.add_fixer(AddNewChoices::new(&v808, "added shulker box", BLOCK_ENTITY));
    let v808_1 = fixer_upper.add_schema_with_sub_version(808, 1, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(EntityShulkerColorFix::new(&v808_1, false));
    let v813 = fixer_upper.add_schema(813, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(ItemShulkerBoxColorFix::new(&v813, false));
    fixer_upper.add_fixer(BlockEntityShulkerBoxColorFix::new(&v813, false));
    let v816 = fixer_upper.add_schema(816, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(OptionsLowerCaseLanguageFix::new(&v816, false));
    let v820 = fixer_upper.add_schema(820, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(ItemRenameFix::create(&v820, "totem item renamer", create_renamer("minecraft:totem", "minecraft:totem_of_undying")));
    fixer_upper.add_schema(1022, &V1022);
    fixer_upper.add_schema(1125, &V1125);
    fixer_upper.add_schema(1344, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(1446, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(1450, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(1451, &V1451);
    fixer_upper.add_schema_with_sub_version(1451, 1, &V1451_1);
    fixer_upper.add_schema_with_sub_version(1451, 2, &V1451_2);
    fixer_upper.add_schema_with_sub_version(1451, 3, &V1451_3);
    fixer_upper.add_schema_with_sub_version(1451, 4, &V1451_4);
    fixer_upper.add_schema_with_sub_version(1451, 5, &V1451_5);
    fixer_upper.add_schema_with_sub_version(1451, 6, &V1451_6);
    fixer_upper.add_schema_with_sub_version(1451, 7, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(1456, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(1458, &V1458);
    fixer_upper.add_schema(1460, &V1460);
    fixer_upper.add_schema(1466, &V1466);
    fixer_upper.add_schema(1470, &V1470);
    fixer_upper.add_schema(1474, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(1475, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(1480, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(1481, &V1481);
    fixer_upper.add_schema(1483, &V1483);
    fixer_upper.add_schema(1484, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(1486, &V1486);
    fixer_upper.add_schema(1487, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(1488, &V1488);
    fixer_upper.add_schema(1490, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(1492, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(1493, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(1494, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(1496, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(1500, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(1501, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(1502, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(1506, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(1510, &V1510);
    fixer_upper.add_schema(1514, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(1515, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(1624, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(1800, &V1800);
    fixer_upper.add_schema(1801, &V1801);
    fixer_upper.add_schema(1802, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(1803, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(1904, &V1904);
    fixer_upper.add_schema(1905, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(1906, &V1906);
    fixer_upper.add_schema(1909, &V1909);
    fixer_upper.add_schema(1911, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(1914, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(1917, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(1918, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(1920, &V1920);
    fixer_upper.add_schema(1925, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(1928, &V1928);
    fixer_upper.add_schema(1929, &V1929);
    fixer_upper.add_schema(1931, &V1931);
    fixer_upper.add_schema(1936, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(1946, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(1948, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(1953, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(1955, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(1961, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(1963, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(2100, &V2100);
    fixer_upper.add_schema(2202, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(2209, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(2211, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(2218, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(2501, &V2501);
    fixer_upper.add_schema(2502, &V2502);
    fixer_upper.add_schema(2503, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(2505, &V2505);
    fixer_upper.add_schema(2508, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(2509, &V2509);
    fixer_upper.add_schema(2511, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema_with_sub_version(2511, 1, &V2511_1);
    fixer_upper.add_schema(2514, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(2516, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(2518, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(2519, &V2519);
    fixer_upper.add_schema(2522, &V2522);
    fixer_upper.add_schema(2523, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(2527, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(2528, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(2529, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(2531, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(2533, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(2535, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(2537, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(2538, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(2550, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(2551, &V2551);
    fixer_upper.add_schema(2552, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(2553, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(2556, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(2558, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(2568, &V2568);
    fixer_upper.add_schema(2571, &V2571);
    fixer_upper.add_schema(2679, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(2680, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(2684, &V2684);
    fixer_upper.add_schema(2686, &V2686);
    fixer_upper.add_schema(2688, &V2688);
    fixer_upper.add_schema(2690, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(2691, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(2693, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(2696, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(2700, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(2701, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(2702, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(2704, &V2704);
    fixer_upper.add_schema(2707, &V2707);
    fixer_upper.add_schema(2710, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(2717, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(2825, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(2831, &V2831);
    fixer_upper.add_schema(2832, &V2832);
    fixer_upper.add_schema(2833, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(2838, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(2841, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(2842, &V2842);
    fixer_upper.add_schema(2843, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(2846, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(2852, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(2967, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(2970, &NAMESPACED_SCHEMA);
    let v3076 = fixer_upper.add_schema(3076, &V3076);
    fixer_upper.add_fixer(AddNewChoices::new(&v3076, "Added Sculk Catalyst", BLOCK_ENTITY));
    let v3077 = fixer_upper.add_schema(3077, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(ChunkDeleteIgnoredLightDataFix::new(&v3077));
    let v3078 = fixer_upper.add_schema(3078, &V3078);
    fixer_upper.add_fixer(AddNewChoices::new(&v3078, "Added Frog", ENTITY));
    fixer_upper.add_fixer(AddNewChoices::new(&v3078, "Added Tadpole", ENTITY));
    fixer_upper.add_fixer(AddNewChoices::new(&v3078, "Added Sculk Shrieker", BLOCK_ENTITY));
    let v3081 = fixer_upper.add_schema(3081, &V3081);
    fixer_upper.add_fixer(AddNewChoices::new(&v3081, "Added Warden", ENTITY));
    let v3082 = fixer_upper.add_schema(3082, &V3082);
    fixer_upper.add_fixer(AddNewChoices::new(&v3082, "Added Chest Boat", ENTITY));
    let v3083 = fixer_upper.add_schema(3083, &V3083);
    fixer_upper.add_fixer(AddNewChoices::new(&v3083, "Added Allay", ENTITY));
    let v3084 = fixer_upper.add_schema(3084, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(NamespacedTypeRenameFix::new(
        &v3084,
        "game_event_renames_3084",
        GAME_EVENT_NAME,
        create_renamer_map(&[
            ("minecraft:block_press", "minecraft:block_activate"),
            ("minecraft:block_switch", "minecraft:block_activate"),
            ("minecraft:block_unpress", "minecraft:block_deactivate"),
            ("minecraft:block_unswitch", "minecraft:block_deactivate"),
            ("minecraft:drinking_finish", "minecraft:drink"),
            ("minecraft:elytra_free_fall", "minecraft:elytra_glide"),
            ("minecraft:entity_damaged", "minecraft:entity_damage"),
            ("minecraft:entity_dying", "minecraft:entity_die"),
            ("minecraft:entity_killed", "minecraft:entity_die"),
            ("minecraft:mob_interact", "minecraft:entity_interact"),
            ("minecraft:ravager_roar", "minecraft:entity_roar"),
            ("minecraft:ring_bell", "minecraft:block_change"),
            ("minecraft:shulker_close", "minecraft:container_close"),
            ("minecraft:shulker_open", "minecraft:container_open"),
            ("minecraft:wolf_shaking", "minecraft:entity_shake"),
        ]),
    ));
    let v3086 = fixer_upper.add_schema(3086, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(EntityVariantFix::new(&v3086, "Change cat variant type", ENTITY, "minecraft:cat", "CatType", |id| {
        Some(match id {
            0 => "minecraft:tabby",
            1 => "minecraft:black",
            2 => "minecraft:red",
            3 => "minecraft:siamese",
            4 => "minecraft:british",
            5 => "minecraft:calico",
            6 => "minecraft:persian",
            7 => "minecraft:ragdoll",
            8 => "minecraft:white",
            9 => "minecraft:jellie",
            10 => "minecraft:all_black",
            _ => "minecraft:tabby",
        })
    }));
    let cat_advancement_migration_map: &'static [(&'static str, &'static str)] = &[
        ("textures/entity/cat/tabby.png", "minecraft:tabby"),
        ("textures/entity/cat/black.png", "minecraft:black"),
        ("textures/entity/cat/red.png", "minecraft:red"),
        ("textures/entity/cat/siamese.png", "minecraft:siamese"),
        ("textures/entity/cat/british_shorthair.png", "minecraft:british"),
        ("textures/entity/cat/calico.png", "minecraft:calico"),
        ("textures/entity/cat/persian.png", "minecraft:persian"),
        ("textures/entity/cat/ragdoll.png", "minecraft:ragdoll"),
        ("textures/entity/cat/white.png", "minecraft:white"),
        ("textures/entity/cat/jellie.png", "minecraft:jellie"),
        ("textures/entity/cat/all_black.png", "minecraft:all_black"),
    ];
    fixer_upper.add_fixer(CriteriaRenameFix::new(
        &v3086,
        "Migrate cat variant advancement",
        "minecraft:husbandry/complete_catalogue",
        create_renamer_no_namespace(cat_advancement_migration_map),
    ));
    let v3087 = fixer_upper.add_schema(3087, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(EntityVariantFix::new(&v3087, "Change frog variant type", ENTITY, "minecraft:frog", "Variant", |id| match id {
        0 => Some("minecraft:temperate"),
        1 => Some("minecraft:warm"),
        2 => Some("minecraft:cold"),
        _ => None,
    }));
    let v3090 = fixer_upper.add_schema(3090, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(EntityFieldsRenameFix::new(&v3090, "EntityPaintingFieldsRenameFix", "minecraft:painting", &[("Motive", "variant"), ("Facing", "facing")]));
    let v3093 = fixer_upper.add_schema(3093, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(EntityGoatMissingStateFix::new(&v3093));
    let v3094 = fixer_upper.add_schema(3094, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(GoatHornIdFix::new(&v3094));
    let v3097 = fixer_upper.add_schema(3097, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(FilteredBooksFix::new(&v3097));
    fixer_upper.add_fixer(FilteredSignsFix::new(&v3097));
    let renamed_cat_criteria: &'static [(&'static str, &'static str)] = &[("minecraft:british", "minecraft:british_shorthair")];
    fixer_upper.add_fixer(VariantRenameFix::new(&v3097, "Rename british shorthair", ENTITY, "minecraft:cat", renamed_cat_criteria));
    fixer_upper.add_fixer(CriteriaRenameFix::new(
        &v3097,
        "Migrate cat variant advancement for british shorthair",
        "minecraft:husbandry/complete_catalogue",
        create_renamer_no_namespace(renamed_cat_criteria),
    ));
    fixer_upper.add_fixer(PoiTypeRemoveFix::new(&v3097, "Remove unpopulated villager PoI types", |id| matches!(id, "minecraft:unemployed" | "minecraft:nitwit")));
    let v3108 = fixer_upper.add_schema(3108, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(BlendingDataRemoveFromNetherEndFix::new(&v3108));
    let v3201 = fixer_upper.add_schema(3201, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(OptionsProgrammerArtFix::new(&v3201));
    let v3202 = fixer_upper.add_schema(3202, &V3202);
    fixer_upper.add_fixer(AddNewChoices::new(&v3202, "Added Hanging Sign", BLOCK_ENTITY));
    let v3203 = fixer_upper.add_schema(3203, &V3203);
    fixer_upper.add_fixer(AddNewChoices::new(&v3203, "Added Camel", ENTITY));
    let v3204 = fixer_upper.add_schema(3204, &V3204);
    fixer_upper.add_fixer(AddNewChoices::new(&v3204, "Added Chiseled Bookshelf", BLOCK_ENTITY));
    let v3209 = fixer_upper.add_schema(3209, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(ItemStackSpawnEggFix::new(&v3209, false, "minecraft:pig_spawn_egg"));
    let v3214 = fixer_upper.add_schema(3214, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(OptionsAmbientOcclusionFix::new(&v3214));
    let v3319 = fixer_upper.add_schema(3319, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(OptionsAccessibilityOnboardFix::new(&v3319));
    let v3322 = fixer_upper.add_schema(3322, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(EffectDurationFix::new(&v3322));
    let v3325 = fixer_upper.add_schema(3325, &V3325);
    fixer_upper.add_fixer(AddNewChoices::new(&v3325, "Added displays", ENTITY));
    let v3326 = fixer_upper.add_schema(3326, &V3326);
    fixer_upper.add_fixer(AddNewChoices::new(&v3326, "Added Sniffer", ENTITY));
    let v3327 = fixer_upper.add_schema(3327, &V3327);
    fixer_upper.add_fixer(AddNewChoices::new(&v3327, "Archaeology", BLOCK_ENTITY));
    let v3328 = fixer_upper.add_schema(3328, &V3328);
    fixer_upper.add_fixer(AddNewChoices::new(&v3328, "Added interaction", ENTITY));
    let v3438 = fixer_upper.add_schema(3438, &V3438);
    fixer_upper.add_fixer(BlockEntityRenameFix::create(&v3438, "Rename Suspicious Sand to Brushable Block", create_renamer("minecraft:suspicious_sand", "minecraft:brushable_block")));
    fixer_upper.add_fixer(EntityBrushableBlockFieldsRenameFix::new(&v3438));
    fixer_upper.add_fixer(ItemRenameFix::create(
        &v3438,
        "Pottery shard renaming",
        create_renamer_map(&[
            ("minecraft:pottery_shard_archer", "minecraft:archer_pottery_shard"),
            ("minecraft:pottery_shard_prize", "minecraft:prize_pottery_shard"),
            ("minecraft:pottery_shard_arms_up", "minecraft:arms_up_pottery_shard"),
            ("minecraft:pottery_shard_skull", "minecraft:skull_pottery_shard"),
        ]),
    ));
    fixer_upper.add_fixer(AddNewChoices::new(&v3438, "Added calibrated sculk sensor", BLOCK_ENTITY));
    let v3439 = fixer_upper.add_schema(3439, &V3439);
    fixer_upper.add_fixer(BlockEntitySignDoubleSidedEditableTextFix::new(&v3439, "Updated sign text format for Signs", "minecraft:sign"));
    let v3439_1 = fixer_upper.add_schema_with_sub_version(3439, 1, &V3439_1);
    fixer_upper.add_fixer(BlockEntitySignDoubleSidedEditableTextFix::new(&v3439_1, "Updated sign text format for Hanging Signs", "minecraft:hanging_sign"));
    let v3440 = fixer_upper.add_schema(3440, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(NamespacedTypeRenameFix::new(
        &v3440,
        "Replace experimental 1.20 overworld",
        MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST,
        create_renamer("minecraft:overworld_update_1_20", "minecraft:overworld"),
    ));
    fixer_upper.add_fixer(FeatureFlagRemoveFix::new(&v3440, "Remove 1.20 feature toggle", &["minecraft:update_1_20"]));
    let v3447 = fixer_upper.add_schema(3447, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(ItemRenameFix::create(
        &v3447,
        "Pottery shard item renaming to Pottery sherd",
        create_renamer_map(&[
            ("minecraft:angler_pottery_shard", "minecraft:angler_pottery_sherd"),
            ("minecraft:archer_pottery_shard", "minecraft:archer_pottery_sherd"),
            ("minecraft:arms_up_pottery_shard", "minecraft:arms_up_pottery_sherd"),
            ("minecraft:blade_pottery_shard", "minecraft:blade_pottery_sherd"),
            ("minecraft:brewer_pottery_shard", "minecraft:brewer_pottery_sherd"),
            ("minecraft:burn_pottery_shard", "minecraft:burn_pottery_sherd"),
            ("minecraft:danger_pottery_shard", "minecraft:danger_pottery_sherd"),
            ("minecraft:explorer_pottery_shard", "minecraft:explorer_pottery_sherd"),
            ("minecraft:friend_pottery_shard", "minecraft:friend_pottery_sherd"),
            ("minecraft:heart_pottery_shard", "minecraft:heart_pottery_sherd"),
            ("minecraft:heartbreak_pottery_shard", "minecraft:heartbreak_pottery_sherd"),
            ("minecraft:howl_pottery_shard", "minecraft:howl_pottery_sherd"),
            ("minecraft:miner_pottery_shard", "minecraft:miner_pottery_sherd"),
            ("minecraft:mourner_pottery_shard", "minecraft:mourner_pottery_sherd"),
            ("minecraft:plenty_pottery_shard", "minecraft:plenty_pottery_sherd"),
            ("minecraft:prize_pottery_shard", "minecraft:prize_pottery_sherd"),
            ("minecraft:sheaf_pottery_shard", "minecraft:sheaf_pottery_sherd"),
            ("minecraft:shelter_pottery_shard", "minecraft:shelter_pottery_sherd"),
            ("minecraft:skull_pottery_shard", "minecraft:skull_pottery_sherd"),
            ("minecraft:snort_pottery_shard", "minecraft:snort_pottery_sherd"),
        ]),
    ));
    let v3448 = fixer_upper.add_schema(3448, &V3448);
    fixer_upper.add_fixer(DecoratedPotFieldRenameFix::new(&v3448));
    let v3450 = fixer_upper.add_schema(3450, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(RemapChunkStatusFix::new(
        &v3450,
        "Remove liquid_carvers and heightmap chunk statuses",
        create_renamer_map(&[("minecraft:liquid_carvers", "minecraft:carvers"), ("minecraft:heightmaps", "minecraft:spawn")]),
    ));
    let v3451 = fixer_upper.add_schema(3451, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(ChunkDeleteLightFix::new(&v3451));
    let v3459 = fixer_upper.add_schema(3459, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(LegacyDragonFightFix::new(&v3459));
    let v3564 = fixer_upper.add_schema(3564, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(DropInvalidSignDataFix::new(&v3564, "minecraft:sign"));
    let v3564_1 = fixer_upper.add_schema_with_sub_version(3564, 1, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(DropInvalidSignDataFix::new(&v3564_1, "minecraft:hanging_sign"));
    let v3565 = fixer_upper.add_schema(3565, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(RandomSequenceSettingsFix::new(&v3565));
    let v3566 = fixer_upper.add_schema(3566, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(ScoreboardDisplaySlotFix::new(&v3566));
    let v3568 = fixer_upper.add_schema(3568, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(MobEffectIdFix::new(&v3568));
    let v3682 = fixer_upper.add_schema(3682, &V3682);
    fixer_upper.add_fixer(AddNewChoices::new(&v3682, "Added Crafter", BLOCK_ENTITY));
    let v3683 = fixer_upper.add_schema(3683, &V3683);
    fixer_upper.add_fixer(PrimedTntBlockStateFixer::new(&v3683));
    let v3685 = fixer_upper.add_schema(3685, &V3685);
    fixer_upper.add_fixer(FixProjectileStoredItem::new(&v3685));
    let v3689 = fixer_upper.add_schema(3689, &V3689);
    fixer_upper.add_fixer(AddNewChoices::new(&v3689, "Added Breeze", ENTITY));
    fixer_upper.add_fixer(AddNewChoices::new(&v3689, "Added Trial Spawner", BLOCK_ENTITY));
    let v3692 = fixer_upper.add_schema(3692, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(LegacyBlockRenameFix::create(&v3692, "Rename grass block to short_grass", create_renamer_map(&[("minecraft:grass", "minecraft:short_grass")])));
    fixer_upper.add_fixer(ItemRenameFix::create(&v3692, "Rename grass item to short_grass", create_renamer_map(&[("minecraft:grass", "minecraft:short_grass")])));
    let v3799 = fixer_upper.add_schema(3799, &V3799);
    fixer_upper.add_fixer(AddNewChoices::new(&v3799, "Added Armadillo", ENTITY));
    let v3800 = fixer_upper.add_schema(3800, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(ItemRenameFix::create(&v3800, "Rename scute item to turtle_scute", create_renamer_map(&[("minecraft:scute", "minecraft:turtle_scute")])));
    let v3803 = fixer_upper.add_schema(3803, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(RenameEnchantmentsFix::new(&v3803, "Rename sweeping enchant to sweeping_edge", &[("minecraft:sweeping", "minecraft:sweeping_edge")]));
    let v3807 = fixer_upper.add_schema(3807, &V3807);
    fixer_upper.add_fixer(AddNewChoices::new(&v3807, "Added Vault", BLOCK_ENTITY));
    let v3807_1 = fixer_upper.add_schema_with_sub_version(3807, 1, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(MapBannerBlockPosFormatFix::new(&v3807_1));
    let v3808 = fixer_upper.add_schema(3808, &V3808);
    fixer_upper.add_fixer(HorseBodyArmorItemFix::new(&v3808, "minecraft:horse", "ArmorItem", true));
    let v3808_1 = fixer_upper.add_schema_with_sub_version(3808, 1, &V3808_1);
    fixer_upper.add_fixer(HorseBodyArmorItemFix::new(&v3808_1, "minecraft:llama", "DecorItem", false));
    let v3808_2 = fixer_upper.add_schema_with_sub_version(3808, 2, &V3808_2);
    fixer_upper.add_fixer(HorseBodyArmorItemFix::new(&v3808_2, "minecraft:trader_llama", "DecorItem", false));
    let v3809 = fixer_upper.add_schema(3809, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(ChestedHorsesInventoryZeroIndexingFix::new(&v3809));
    let v3812 = fixer_upper.add_schema(3812, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(FixWolfHealth::new(&v3812));
    let v3813 = fixer_upper.add_schema(3813, &V3813);
    fixer_upper.add_fixer(BlockPosFormatAndRenamesFix::new(&v3813));
    let v3814 = fixer_upper.add_schema(3814, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(AttributesRenameLegacy::new(&v3814, "Rename jump strength attribute", create_renamer("minecraft:horse.jump_strength", "minecraft:generic.jump_strength")));
    let v3816 = fixer_upper.add_schema(3816, &V3816);
    fixer_upper.add_fixer(AddNewChoices::new(&v3816, "Added Bogged", ENTITY));
    let v3818 = fixer_upper.add_schema(3818, &V3818);
    fixer_upper.add_fixer(BeehiveFieldRenameFix::new(&v3818));
    fixer_upper.add_fixer(EmptyItemInHotbarFix::new(&v3818));
    let v3818_1 = fixer_upper.add_schema_with_sub_version(3818, 1, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(BannerPatternFormatFix::new(&v3818_1));
    let v3818_2 = fixer_upper.add_schema_with_sub_version(3818, 2, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(TippedArrowPotionToItemFix::new(&v3818_2));
    let v3818_3 = fixer_upper.add_schema_with_sub_version(3818, 3, &V3818_3);
    fixer_upper.add_fixer(WriteAndReadFix::new(&v3818_3, "Inject data component types", DATA_COMPONENTS));
    let v3818_4 = fixer_upper.add_schema_with_sub_version(3818, 4, &V3818_4);
    fixer_upper.add_fixer(ParticleUnflatteningFix::new(&v3818_4));
    let v3818_5 = fixer_upper.add_schema_with_sub_version(3818, 5, &V3818_5);
    fixer_upper.add_fixer(ItemStackComponentizationFix::new(&v3818_5));
    let v3818_6 = fixer_upper.add_schema_with_sub_version(3818, 6, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(AreaEffectCloudPotionFix::new(&v3818_6));
    let v3820 = fixer_upper.add_schema(3820, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(PlayerHeadBlockProfileFix::new(&v3820));
    fixer_upper.add_fixer(LodestoneCompassComponentFix::new(&v3820));
    let v3825 = fixer_upper.add_schema(3825, &V3825);
    fixer_upper.add_fixer(ItemStackCustomNameToOverrideComponentFix::new(&v3825));
    fixer_upper.add_fixer(BannerEntityCustomNameToOverrideComponentFix::new(&v3825));
    fixer_upper.add_fixer(TrialSpawnerConfigFix::new(&v3825));
    fixer_upper.add_fixer(AddNewChoices::new(&v3825, "Added Ominous Item Spawner", ENTITY));
    let v3828 = fixer_upper.add_schema(3828, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(EmptyItemInVillagerTradeFix::new(&v3828));
    let v3833 = fixer_upper.add_schema(3833, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(RemoveEmptyItemInBrushableBlockFix::new(&v3833));
    let v3938 = fixer_upper.add_schema(3938, &V3938);
    fixer_upper.add_fixer(ProjectileStoredWeaponFix::new(&v3938));
    let v3939 = fixer_upper.add_schema(3939, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(FeatureFlagRemoveFix::new(&v3939, "Remove 1.21 feature toggle", &["minecraft:update_1_21"]));
    let v3943 = fixer_upper.add_schema(3943, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(OptionsMenuBlurrinessFix::new(&v3943));
    let v3945 = fixer_upper.add_schema(3945, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(AttributeModifierIdFix::new(&v3945));
    fixer_upper.add_fixer(JukeboxTicksSinceSongStartedFix::new(&v3945));
    fixer_upper.add_schema(4054, &NAMESPACED_SCHEMA);
    let v4055 = fixer_upper.add_schema(4055, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(AttributeIdPrefixFix::new(&v4055));
    let v4059 = fixer_upper.add_schema(4059, &V4059);
    fixer_upper.add_fixer(FoodToConsumableFix::new(&v4059));
    let v4061 = fixer_upper.add_schema(4061, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(TrialSpawnerConfigInRegistryFix::new(&v4061));
    let v4064 = fixer_upper.add_schema(4064, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(FireResistantToDamageResistantComponentFix::new(&v4064));
    let v4067 = fixer_upper.add_schema(4067, &V4067);
    fixer_upper.add_fixer(BoatSplitFix::new(&v4067));
    fixer_upper.add_fixer(FeatureFlagRemoveFix::new(&v4067, "Remove Bundle experimental feature flag", &["minecraft:bundle"]));
    let v4068 = fixer_upper.add_schema(4068, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(LockComponentPredicateFix::new(&v4068));
    fixer_upper.add_fixer(ContainerBlockEntityLockPredicateFix::new(&v4068));
    let v4070 = fixer_upper.add_schema(4070, &V4070);
    fixer_upper.add_fixer(AddNewChoices::new(&v4070, "Added Pale Oak Boat and Pale Oak Chest Boat", ENTITY));
    let v4071 = fixer_upper.add_schema(4071, &V4071);
    fixer_upper.add_fixer(AddNewChoices::new(&v4071, "Added Creaking", ENTITY));
    fixer_upper.add_fixer(AddNewChoices::new(&v4071, "Added Creaking Heart", BLOCK_ENTITY));
    let v4081 = fixer_upper.add_schema(4081, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(EntitySalmonSizeFix::new(&v4081));
    let v4173 = fixer_upper.add_schema(4173, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(EntityFieldsRenameFix::new(&v4173, "Rename TNT Minecart fuse", "minecraft:tnt_minecart", &[("TNTFuse", "fuse")]));
    let v4175 = fixer_upper.add_schema(4175, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(EquippableAssetRenameFix::new(&v4175));
    fixer_upper.add_fixer(CustomModelDataExpandFix::new(&v4175));
    let v4176 = fixer_upper.add_schema(4176, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(InvalidBlockEntityLockFix::new(&v4176));
    fixer_upper.add_fixer(InvalidLockComponentFix::new(&v4176));
    let v4180 = fixer_upper.add_schema(4180, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(FeatureFlagRemoveFix::new(&v4180, "Remove Winter Drop toggle", &["minecraft:winter_drop"]));
    let v4181 = fixer_upper.add_schema(4181, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(BlockEntityFurnaceBurnTimeFix::new(&v4181, "minecraft:furnace"));
    fixer_upper.add_fixer(BlockEntityFurnaceBurnTimeFix::new(&v4181, "minecraft:smoker"));
    fixer_upper.add_fixer(BlockEntityFurnaceBurnTimeFix::new(&v4181, "minecraft:blast_furnace"));
    let v4187 = fixer_upper.add_schema(4187, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(EntityAttributeBaseValueFix::new(&v4187, "Villager follow range fix undo", "minecraft:villager", "minecraft:follow_range", |value| if value == 48.0 { 16.0 } else { value }));
    fixer_upper.add_fixer(EntityAttributeBaseValueFix::new(&v4187, "Bee follow range fix", "minecraft:bee", "minecraft:follow_range", |value| if value == 48.0 { 16.0 } else { value }));
    fixer_upper.add_fixer(EntityAttributeBaseValueFix::new(&v4187, "Allay follow range fix", "minecraft:allay", "minecraft:follow_range", |value| if value == 48.0 { 16.0 } else { value }));
    fixer_upper.add_fixer(EntityAttributeBaseValueFix::new(&v4187, "Llama follow range fix", "minecraft:llama", "minecraft:follow_range", |value| if value == 40.0 { 16.0 } else { value }));
    fixer_upper.add_fixer(EntityAttributeBaseValueFix::new(&v4187, "Piglin Brute follow range fix", "minecraft:piglin_brute", "minecraft:follow_range", |value| if value == 16.0 { 12.0 } else { value }));
    fixer_upper.add_fixer(EntityAttributeBaseValueFix::new(&v4187, "Warden follow range fix", "minecraft:warden", "minecraft:follow_range", |value| if value == 16.0 { 24.0 } else { value }));
    fixer_upper.add_schema(4290, &V4290);
    let v4291 = fixer_upper.add_schema(4291, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(TextComponentStringifiedFlagsFix::new(&v4291));
    let v4292 = fixer_upper.add_schema(4292, &V4292);
    fixer_upper.add_fixer(TextComponentHoverAndClickEventFix::new(&v4292));
    let v4293 = fixer_upper.add_schema(4293, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(DropChancesFormatFix::new(&v4293));
    let v4294 = fixer_upper.add_schema(4294, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(LegacyBlockPropertyRenameAndFix::new(
        &v4294,
        "CreakingHeartBlockStateFix",
        "minecraft:creaking_heart",
        "active",
        "creaking_heart_state",
        |value| if value == "true" { "awake".to_string() } else { "uprooted".to_string() },
    ));
    let v4296 = fixer_upper.add_schema(4296, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(AreaEffectCloudDurationScaleFix::new(&v4296));
    let v4297 = fixer_upper.add_schema(4297, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(ForcedChunkToTicketFix::new(&v4297));
    let v4299 = fixer_upper.add_schema(4299, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(EntitySpawnerItemVariantComponentFix::new(&v4299));
    let v4300 = fixer_upper.add_schema(4300, &V4300);
    fixer_upper.add_fixer(SaddleEquipmentSlotFix::new(&v4300));
    let v4301 = fixer_upper.add_schema(4301, &V4301);
    fixer_upper.add_fixer(EquipmentFormatFix::new(&v4301));
    let v4302 = fixer_upper.add_schema(4302, &V4302);
    fixer_upper.add_fixer(AddNewChoices::new(&v4302, "Added Test and Test Instance Block Entities", BLOCK_ENTITY));
    let v4303 = fixer_upper.add_schema(4303, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(EntityFallDistanceFloatToDoubleFix::new(&v4303, ENTITY));
    fixer_upper.add_fixer(EntityFallDistanceFloatToDoubleFix::new(&v4303, PLAYER));
    let v4305 = fixer_upper.add_schema(4305, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(LegacyBlockPropertyRenameAndFix::new(&v4305, "rename test block mode", "minecraft:test_block", "test_block_mode", "mode", |a| a.to_string()));
    let v4306 = fixer_upper.add_schema(4306, &V4306);
    fixer_upper.add_fixer(ThrownPotionSplitFix::new(&v4306));
    let v4307 = fixer_upper.add_schema(4307, &V4307);
    fixer_upper.add_fixer(TooltipDisplayComponentFix::new(&v4307));
    let v4309 = fixer_upper.add_schema(4309, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(RaidRenamesDataFix::new(&v4309));
    fixer_upper.add_fixer(ChunkTicketUnpackPosFix::new(&v4309));
    let v4311 = fixer_upper.add_schema(4311, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(AdvancementsRenameFix::new(&v4311, false, "Use lodestone category change", create_renamer("minecraft:nether/use_lodestone", "minecraft:adventure/use_lodestone")));
    let v4312 = fixer_upper.add_schema(4312, &V4312);
    fixer_upper.add_fixer(PlayerEquipmentFix::new(&v4312));
    let v4314 = fixer_upper.add_schema(4314, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(InlineBlockPosFormatFix::new(&v4314));
    let v4420 = fixer_upper.add_schema(4420, &V4420);
    fixer_upper.add_fixer(NamedEntityConvertUncheckedFix::new(&v4420, "AreaEffectCloudCustomParticleFix", ENTITY, "minecraft:area_effect_cloud"));
    let v4421 = fixer_upper.add_schema(4421, &V4421);
    fixer_upper.add_fixer(AddNewChoices::new(&v4421, "Added Happy Ghast", ENTITY));
    let v4424 = fixer_upper.add_schema(4424, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(FeatureFlagRemoveFix::new(&v4424, "Remove Locator Bar experimental feature flag", &["minecraft:locator_bar"]));
    fixer_upper.add_fixer(AddFieldFix::new(&v4424, PLAYER, "style", |field| field.create_string("minecraft:default"), &["locator_bar_icon"]));
    fixer_upper.add_fixer(AddFieldFix::new(&v4424, ENTITY, "style", |field| field.create_string("minecraft:default"), &["locator_bar_icon"]));
    let v4531 = fixer_upper.add_schema(4531, &V4531);
    fixer_upper.add_fixer(AddNewChoices::new(&v4531, "Added Copper Golem", ENTITY));
    let v4532 = fixer_upper.add_schema(4532, &V4532);
    fixer_upper.add_fixer(AddNewChoices::new(&v4532, "Added Copper Golem Statue Block Entity", BLOCK_ENTITY));
    let v4533 = fixer_upper.add_schema(4533, &V4533);
    fixer_upper.add_fixer(AddNewChoices::new(&v4533, "Added Shelf", BLOCK_ENTITY));
    let v4535 = fixer_upper.add_schema(4535, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(CopperGolemWeatherStateFix::new(&v4535));
    let v4537 = fixer_upper.add_schema(4537, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(ChunkDeleteLightFix::new(&v4537));
    let v4541 = fixer_upper.add_schema(4541, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(LegacyBlockRenameFix::create(&v4541, "Rename chain to iron_chain", create_renamer("minecraft:chain", "minecraft:iron_chain")));
    fixer_upper.add_fixer(ItemRenameFix::create(&v4541, "Rename chain to iron_chain", create_renamer("minecraft:chain", "minecraft:iron_chain")));
    let v4543 = fixer_upper.add_schema(4543, &V4543);
    fixer_upper.add_fixer(AddNewChoices::new(&v4543, "Added Mannequin", ENTITY));
    let v4544 = fixer_upper.add_schema(4544, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(LegacyWorldBorderFix::new(&v4544));
    let v4548 = fixer_upper.add_schema(4548, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(WorldSpawnDataFix::new(&v4548));
    fixer_upper.add_fixer(PlayerRespawnDataFix::new(&v4548));
    let v4648 = fixer_upper.add_schema(4648, &V4648);
    fixer_upper.add_fixer(AddNewChoices::new(&v4648, "Added Nautilus and Zombie Nautilus", ENTITY));
    let v4649 = fixer_upper.add_schema(4649, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(TridentAnimationFix::new(&v4649));
    let v4650 = fixer_upper.add_schema(4650, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(DebugProfileOverlayReferenceFix::new(&v4650));
    let v4651 = fixer_upper.add_schema(4651, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(OptionsGraphicsModeSplitFix::new(&v4651, "cutoutLeaves", "false", "true", "true"));
    fixer_upper.add_fixer(OptionsGraphicsModeSplitFix::new(&v4651, "weatherRadius", "5", "10", "10"));
    fixer_upper.add_fixer(OptionsGraphicsModeSplitFix::new(&v4651, "vignette", "false", "true", "true"));
    fixer_upper.add_fixer(OptionsGraphicsModeSplitFix::new(&v4651, "improvedTransparency", "false", "false", "true"));
    fixer_upper.add_fixer(OptionsSetGraphicsPresetToCustomFix::new(&v4651));
    let v4656 = fixer_upper.add_schema(4656, &V4656);
    fixer_upper.add_fixer(AddNewChoices::new(&v4656, "Added Parched and Camel Husk", ENTITY));
    let v4657 = fixer_upper.add_schema(4657, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(WorldBorderWarningTimeFix::new(&v4657));
    let v4658 = fixer_upper.add_schema(4658, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(GameRuleRegistryFix::new(&v4658));
    let v4661 = fixer_upper.add_schema(4661, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(OptionsMusicToastFix::new(&v4661, false));
    let v4763 = fixer_upper.add_schema(4763, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(DebugProfileLookingAtSplitFix::new(&v4763));
    fixer_upper.add_fixer(VillagerSetVillagerDataFinalized::new(&v4763));
    fixer_upper.add_fixer(ZombieVillagerSetVillagerDataFinalized::new(&v4763));
    let v4766 = fixer_upper.add_schema(4766, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(DayTimeToClockFix::new(&v4766));
    let v4769 = fixer_upper.add_schema(4769, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(NamespacedTypeRenameFix::new(&v4769, "Rename stonecutter recipes", RECIPE, create_renamer_map(stonecutter_recipe_rename_fix::RECIPE_RENAMES)));
    fixer_upper.add_fixer(AdvancementsRenameFix::new(&v4769, false, "Rename stonecutter recipe advancements", create_renamer_map(stonecutter_recipe_rename_fix::ADVANCEMENT_RENAMES)));
    let v4771 = fixer_upper.add_schema(4771, &V4771);
    fixer_upper.add_fixer(LevelDatDifficultyFix::new(&v4771));
    fixer_upper.add_fixer(LevelDatToSavedDataPreparationFix::new(&v4771));
    fixer_upper.add_schema(4772, &NAMESPACED_SCHEMA);
    fixer_upper.add_schema(4773, &NAMESPACED_SCHEMA);
    let v4881 = fixer_upper.add_schema(4881, &V4881);
    fixer_upper.add_fixer(AddNewChoices::new(&v4881, "Added Sulfur Cube", ENTITY));
    fixer_upper.add_fixer(AddNewChoices::new(&v4881, "Added Potent Sulfur", BLOCK_ENTITY));
    let v4885 = fixer_upper.add_schema(4885, &V4885);
    fixer_upper.add_fixer(RemoveBlockEntityTagFix::new(&v4885, &["minecraft:bed"]));
    let v4888 = fixer_upper.add_schema(4888, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(RenameNameplateToNameTagFix::new(&v4888));
    let v4892 = fixer_upper.add_schema(4892, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(OptionsForceDefaultGraphicsApiFix::new(&v4892));
    fixer_upper.add_schema(4899, &NAMESPACED_SCHEMA);
    let v4996 = fixer_upper.add_schema(4996, &V4996);
    fixer_upper.add_fixer(PotDecorationsComponentUnflatteningFix::new(&v4996));
    let v4996_1 = fixer_upper.add_schema_with_sub_version(4996, 1, &V4996_1);
    fixer_upper.add_fixer(PotDecorationsBlockEntityUnflatteningFix::new(&v4996_1));
    let blending_schema = fixer_upper.add_schema(4997, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(BlendingDataFix::new(&blending_schema));
    let v4997 = fixer_upper.add_schema(4997, &V4997);
    fixer_upper.add_fixer(AddNewChoices::new(&v4997, "Added Poplar Boat and Poplar Chest Boat", ENTITY));
    let v5000 = fixer_upper.add_schema(5000, &V5000);
    fixer_upper.add_fixer(AddNewChoices::new(&v5000, "Added Cushion", ENTITY));
    let v5001 = fixer_upper.add_schema(5001, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(DebugProfileRenameEntryFix::new(&v5001, "minecraft:post_effect", "minecraft:post_effects"));
    let v5002 = fixer_upper.add_schema(5002, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(SignsOpFlagFix::new(&v5002));
    let v5006 = fixer_upper.add_schema(5006, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(BlockStateFieldNamesFix::new(&v5006));
    let v5007 = fixer_upper.add_schema(5007, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(SwingAnimationComponentSplitFix::new(&v5007));
    let v5008 = fixer_upper.add_schema(5008, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(ExplorerMapItemFix::new(&v5008));
    fixer_upper.add_fixer(MapColorComponentRemovalFix::new(&v5008));
    let v5009 = fixer_upper.add_schema(5009, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(BlendingDataNoValueHeightFix::new(&v5009));
    let v5012 = fixer_upper.add_schema(5012, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(ItemRenameFix::create(
        &v5012,
        "Rename explorer map items",
        create_renamer_map(&[
            ("minecraft:ocean_explorer_map", "minecraft:ocean_monument_map"),
            ("minecraft:swamp_explorer_map", "minecraft:swamp_hut_map"),
            ("minecraft:trial_explorer_map", "minecraft:buried_trial_chambers_map"),
            ("minecraft:woodland_explorer_map", "minecraft:woodland_mansion_map"),
            ("minecraft:jungle_explorer_map", "minecraft:jungle_pyramid_map"),
        ]),
    ));
    let v5013 = fixer_upper.add_schema(5013, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(MergeTerrainChunkStatusFix::new(&v5013));
    let v5014 = fixer_upper.add_schema(5014, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(RemoveBlockTransformerComponentFix::new(&v5014));
    let v5016 = fixer_upper.add_schema(5016, &NAMESPACED_SCHEMA);
    fixer_upper.add_fixer(UnfilledBuriedTreasureMapFix::new(&v5016));
}

pub fn create_renamer_no_namespace(map: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> String + Send + Sync + 'static {
    move |id| map.iter().find(|(from, _)| *from == id).map_or_else(|| id.to_string(), |(_, to)| to.to_string())
}

pub fn create_renamer_map(map: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> String + Send + Sync + 'static {
    move |id| {
        let namespaced = ensure_namespaced(id);
        map.iter().find(|(from, _)| *from == namespaced).map_or_else(|| id.to_string(), |(_, to)| to.to_string())
    }
}

pub fn create_renamer(from: &'static str, to: &'static str) -> impl Fn(&str) -> String + Send + Sync + 'static {
    move |id| if ensure_namespaced(id) == from { to.to_string() } else { id.to_string() }
}
