pub mod ageable_mob;
pub mod ai;
pub mod ambient;
pub mod animal;
pub mod area_effect_cloud;
pub mod attributes;
pub mod avatar;
pub mod boss;
pub mod bucketable;
pub mod conversion_params;
pub mod conversion_tracker;
pub mod conversion_type;
pub mod crackiness;
pub mod data;
pub mod decoration;
#[cfg(test)]
mod entity_data_layout_tests;
#[cfg(test)]
mod effect_tests;
#[allow(clippy::module_inception)]
mod entity;
pub mod entity_attachments;
pub mod entity_components;
mod entity_dimensions;
pub mod entity_event;
pub mod entity_reference;
pub mod entity_save;
pub mod entity_selector;
pub mod entity_spawn_reason;
pub mod entity_type;
pub mod entity_types;
pub mod equipment_table;
pub mod equipment_user;
pub mod experience_orb;
pub mod fluid_interaction;
pub mod has_custom_inventory_screen;
pub mod item_based_steering;
pub mod item_steerable;
pub mod inside_block;
pub mod interaction;
pub mod display;
#[cfg(test)]
mod display_tests;
#[cfg(test)]
mod interaction_tests;
pub mod item;
pub mod leashable;
#[cfg(test)]
mod leashable_tests;
pub mod lightning_bolt;
pub mod living_entity;
pub mod marker;
pub mod ominous_item_spawner;
#[cfg(test)]
mod ominous_item_spawner_tests;
pub mod mob;
pub mod mob_category;
pub mod monster;
pub mod neutral_mob;
pub mod npc;
pub mod ownable_entity;
pub mod pathfinder_mob;
pub mod player_rideable_jumping;
pub mod portal_processor;
pub mod primed_tnt;
pub mod projectile;
pub mod raid;
pub mod player;
pub mod relative;
pub mod schedule;
pub mod shearable;
pub mod slot_access;
pub mod spawn_group_data;
pub mod spawn_placement_types;
pub mod spawn_placements;
pub mod tamable_animal;
#[cfg(test)]
mod survival_tests;
pub mod using_item;
pub mod variant;
pub mod vehicle;

pub use entity::*;
pub use entity_dimensions::EntityDimensions;
pub use item::ItemEntity;

pub enum LevelEntity {
    Item(ItemEntity),
    ExperienceOrb(experience_orb::ExperienceOrb),
    LightningBolt(lightning_bolt::LightningBolt),
    EyeOfEnder(projectile::eye_of_ender::EyeOfEnder),
    EvokerFangs(projectile::evoker_fangs::EvokerFangs),
    LeashKnot(decoration::leash_fence_knot_entity::LeashFenceKnotEntity),
    EndCrystal(boss::enderdragon::end_crystal::EndCrystal),
    PrimedTnt(primed_tnt::PrimedTnt),
    FallingBlock(item::falling_block_entity::FallingBlockEntity),
    Projectile(Box<projectile::projectile::Projectile>),
    AreaEffectCloud(Box<area_effect_cloud::AreaEffectCloud>),
    Mob(Box<mob::Mob>),
    Minecart(Box<vehicle::minecart::abstract_minecart::AbstractMinecart>),
    Boat(Box<vehicle::boat::abstract_boat::AbstractBoat>),
    Marker(marker::Marker),
    Interaction(interaction::Interaction),
    Display(display::Display),
    OminousItemSpawner(Box<ominous_item_spawner::OminousItemSpawner>),
    ArmorStand(Box<decoration::armor_stand::ArmorStand>),
    Mannequin(Box<decoration::mannequin::Mannequin>),
    Painting(decoration::painting::Painting),
    Cushion(decoration::cushion::Cushion),
    ItemFrame(decoration::item_frame::ItemFrame),
}

impl LevelEntity {
    pub fn name(&self) -> crate::network::chat::Component {
        match self {
            LevelEntity::Item(item) => item.entity.custom_name().unwrap_or_else(|| item.item.item_name()),
            _ => match self.entity().custom_name() {
                Some(custom_name) => remove_action(custom_name),
                None => self.type_name(),
            },
        }
    }

    fn type_name(&self) -> crate::network::chat::Component {
        match self {
            LevelEntity::Mob(mob) if matches!(mob.kind, mob::MobKind::Villager(_)) => npc::villager::villager::get_type_name(mob),
            LevelEntity::FallingBlock(falling) => crate::network::chat::Component::translatable_with(
                "entity.minecraft.falling_block_type",
                vec![crate::network::chat::Arg::Component(crate::network::chat::Component::translatable(falling.get_block_state().block().info().description_id))],
            ),
            _ => description(wafer_data::registry::ENTITY_TYPE[self.entity().entity_type as usize]),
        }
    }

    pub fn display_name(&self, team: Option<&crate::world::scores::player_team::PlayerTeam>) -> crate::network::chat::Component {
        use crate::network::chat::HoverEvent;
        let entity = self.entity();
        let type_id = wafer_data::registry::ENTITY_TYPE[entity.entity_type as usize];
        crate::world::scores::player_team::PlayerTeam::format_name_for_team(team, self.name())
            .with_hover(HoverEvent::ShowEntity { id: type_id, uuid: entity.uuid, name: Some(Box::new(self.name())) })
            .with_insertion(&crate::world::level::storage::format_uuid(entity.uuid))
    }

    pub fn tracking_config(&self) -> crate::server::level::chunk_map::TrackingConfig {
        use crate::server::level::chunk_map::TrackingConfig;
        use crate::server::level::server_entity::UpdateInterval;
        match self {
            LevelEntity::Item(_) => TrackingConfig { range: 6 * 16, update_interval: UpdateInterval::Periodic(20), track_deltas: true, stepped: false, is_item: true },
            LevelEntity::PrimedTnt(_) => TrackingConfig { range: 10 * 16, update_interval: UpdateInterval::Periodic(10), track_deltas: true, stepped: false, is_item: false },
            LevelEntity::FallingBlock(_) => TrackingConfig { range: 10 * 16, update_interval: UpdateInterval::Periodic(20), track_deltas: true, stepped: false, is_item: false },
            LevelEntity::LightningBolt(_) => TrackingConfig { range: 16 * 16, update_interval: UpdateInterval::Never, track_deltas: true, stepped: false, is_item: false },
            LevelEntity::EyeOfEnder(_) => TrackingConfig { range: 4 * 16, update_interval: UpdateInterval::Periodic(4), track_deltas: true, stepped: false, is_item: false },
            LevelEntity::Mannequin(_) => TrackingConfig { range: 32 * 16, update_interval: UpdateInterval::Periodic(2), track_deltas: true, stepped: false, is_item: false },
            LevelEntity::ArmorStand(_) => TrackingConfig { range: 10 * 16, update_interval: UpdateInterval::Periodic(3), track_deltas: true, stepped: false, is_item: false },
            LevelEntity::OminousItemSpawner(_) => TrackingConfig { range: 8 * 16, update_interval: UpdateInterval::Periodic(3), track_deltas: true, stepped: false, is_item: false },
            LevelEntity::Interaction(_) => TrackingConfig { range: 10 * 16, update_interval: UpdateInterval::Periodic(3), track_deltas: true, stepped: false, is_item: false },
            LevelEntity::Display(_) => TrackingConfig { range: 10 * 16, update_interval: UpdateInterval::Periodic(1), track_deltas: true, stepped: false, is_item: false },
            LevelEntity::Marker(_) => TrackingConfig { range: 0, update_interval: UpdateInterval::Periodic(1), track_deltas: true, stepped: false, is_item: false },
            LevelEntity::Painting(_) | LevelEntity::ItemFrame(_) | LevelEntity::Cushion(_) => TrackingConfig { range: 10 * 16, update_interval: UpdateInterval::Never, track_deltas: false, stepped: false, is_item: false },
            LevelEntity::LeashKnot(_) => TrackingConfig { range: 10 * 16, update_interval: UpdateInterval::Never, track_deltas: true, stepped: false, is_item: false },
            LevelEntity::EvokerFangs(_) => TrackingConfig { range: 6 * 16, update_interval: UpdateInterval::Periodic(2), track_deltas: false, stepped: false, is_item: false },
            LevelEntity::EndCrystal(_) => TrackingConfig { range: 16 * 16, update_interval: UpdateInterval::Never, track_deltas: false, stepped: false, is_item: false },
            LevelEntity::ExperienceOrb(_) => TrackingConfig { range: 6 * 16, update_interval: UpdateInterval::Periodic(20), track_deltas: true, stepped: false, is_item: false },
            LevelEntity::Mob(mob) => TrackingConfig { range: mob.client_tracking_range() * 16, update_interval: UpdateInterval::Periodic(3), track_deltas: mob.track_deltas(), stepped: true, is_item: false },
            LevelEntity::Projectile(projectile) if matches!(projectile.kind, projectile::projectile::ProjectileKind::ShulkerBullet(_)) => {
                TrackingConfig { range: 8 * 16, update_interval: UpdateInterval::Periodic(3), track_deltas: true, stepped: false, is_item: false }
            }
            LevelEntity::Projectile(projectile) => {
                let update_interval = if projectile.abstract_arrow().is_some() {
                    20
                } else if matches!(projectile.kind, projectile::projectile::ProjectileKind::FishingHook(_)) {
                    5
                } else {
                    10
                };
                let track_deltas = !matches!(projectile.kind, projectile::projectile::ProjectileKind::LlamaSpit(_));
                TrackingConfig { range: 4 * 16, update_interval: UpdateInterval::Periodic(update_interval), track_deltas, stepped: false, is_item: false }
            }
            LevelEntity::AreaEffectCloud(_) => TrackingConfig { range: 10 * 16, update_interval: UpdateInterval::Never, track_deltas: true, stepped: false, is_item: false },
            LevelEntity::Minecart(_) => TrackingConfig { range: 8 * 16, update_interval: UpdateInterval::Periodic(3), track_deltas: true, stepped: false, is_item: false },
            LevelEntity::Boat(_) => TrackingConfig { range: 10 * 16, update_interval: UpdateInterval::Periodic(3), track_deltas: true, stepped: false, is_item: false },
        }
    }

    pub fn add_entity_data(&self) -> i32 {
        match self {
            LevelEntity::FallingBlock(falling) => falling.get_block_state().id() as i32,
            LevelEntity::Painting(painting) => decoration::hanging_entity::get_direction(&painting.entity).ordinal() as i32,
            LevelEntity::ItemFrame(frame) => decoration::hanging_entity::get_direction(&frame.entity).ordinal() as i32,
            _ => 0,
        }
    }

    pub fn entity(&self) -> &Entity {
        match self {
            LevelEntity::Item(item) => &item.entity,
            LevelEntity::LightningBolt(bolt) => &bolt.entity,
            LevelEntity::EyeOfEnder(eye) => &eye.entity,
            LevelEntity::EvokerFangs(fangs) => &fangs.entity,
            LevelEntity::LeashKnot(knot) => &knot.entity,
            LevelEntity::Painting(painting) => &painting.entity,
            LevelEntity::Cushion(cushion) => &cushion.entity,
            LevelEntity::ItemFrame(frame) => &frame.entity,
            LevelEntity::Marker(marker) => &marker.entity,
            LevelEntity::OminousItemSpawner(spawner) => &spawner.entity,
            LevelEntity::ArmorStand(stand) => &stand.entity,
            LevelEntity::Mannequin(mannequin) => &mannequin.entity,
            LevelEntity::Interaction(interaction) => &interaction.entity,
            LevelEntity::Display(display) => &display.entity,
            LevelEntity::EndCrystal(crystal) => &crystal.entity,
            LevelEntity::PrimedTnt(tnt) => &tnt.entity,
            LevelEntity::FallingBlock(falling) => &falling.entity,
            LevelEntity::ExperienceOrb(orb) => &orb.entity,
            LevelEntity::Projectile(projectile) => &projectile.entity,
            LevelEntity::AreaEffectCloud(cloud) => &cloud.entity,
            LevelEntity::Mob(mob) => &mob.entity,
            LevelEntity::Minecart(minecart) => &minecart.entity,
            LevelEntity::Boat(boat) => &boat.entity,
        }
    }

    pub fn entity_mut(&mut self) -> &mut Entity {
        match self {
            LevelEntity::Item(item) => &mut item.entity,
            LevelEntity::LightningBolt(bolt) => &mut bolt.entity,
            LevelEntity::EyeOfEnder(eye) => &mut eye.entity,
            LevelEntity::EvokerFangs(fangs) => &mut fangs.entity,
            LevelEntity::LeashKnot(knot) => &mut knot.entity,
            LevelEntity::Painting(painting) => &mut painting.entity,
            LevelEntity::Cushion(cushion) => &mut cushion.entity,
            LevelEntity::ItemFrame(frame) => &mut frame.entity,
            LevelEntity::Marker(marker) => &mut marker.entity,
            LevelEntity::OminousItemSpawner(spawner) => &mut spawner.entity,
            LevelEntity::ArmorStand(stand) => &mut stand.entity,
            LevelEntity::Mannequin(mannequin) => &mut mannequin.entity,
            LevelEntity::Interaction(interaction) => &mut interaction.entity,
            LevelEntity::Display(display) => &mut display.entity,
            LevelEntity::EndCrystal(crystal) => &mut crystal.entity,
            LevelEntity::PrimedTnt(tnt) => &mut tnt.entity,
            LevelEntity::FallingBlock(falling) => &mut falling.entity,
            LevelEntity::ExperienceOrb(orb) => &mut orb.entity,
            LevelEntity::Projectile(projectile) => &mut projectile.entity,
            LevelEntity::AreaEffectCloud(cloud) => &mut cloud.entity,
            LevelEntity::Mob(mob) => &mut mob.entity,
            LevelEntity::Minecart(minecart) => &mut minecart.entity,
            LevelEntity::Boat(boat) => &mut boat.entity,
        }
    }

    pub fn as_item(&self) -> Option<&ItemEntity> {
        match self {
            LevelEntity::Item(item) => Some(item),
            _ => None,
        }
    }

    pub fn as_item_mut(&mut self) -> Option<&mut ItemEntity> {
        match self {
            LevelEntity::Item(item) => Some(item),
            _ => None,
        }
    }

    pub fn as_experience_orb(&self) -> Option<&experience_orb::ExperienceOrb> {
        match self {
            LevelEntity::ExperienceOrb(orb) => Some(orb),
            _ => None,
        }
    }
}

pub fn can_summon(entity_type: &str) -> bool {
    !matches!(entity_type, "minecraft:player" | "minecraft:fishing_bobber")
}

fn remove_action(component: crate::network::chat::Component) -> crate::network::chat::Component {
    crate::network::chat::Component {
        contents: component.contents,
        style: crate::network::chat::Style { click_event: None, ..component.style },
        siblings: component.siblings.into_iter().map(remove_action).collect(),
    }
}

pub fn description(entity_type: &str) -> crate::network::chat::Component {
    crate::network::chat::Component::translatable(&format!("entity.{}", entity_type.replacen(':', ".", 1)))
}
