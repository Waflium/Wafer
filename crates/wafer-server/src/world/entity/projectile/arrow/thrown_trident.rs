use wafer_core::Vec3;
use wafer_data::{damage_types, items};
use wafer_protocol::nbt::Nbt;

use super::abstract_arrow::{self, AbstractArrow, Pickup};
use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::entity::data::DataValue;
use crate::world::entity::projectile::projectile::{Projectile, ProjectileKind};
use crate::world::entity::projectile::projectile_deflection::ProjectileDeflection;
use crate::world::entity::{EntityId, LevelEntity, sound_event};
use crate::world::item::ItemStack;
use crate::world::item::enchantment::enchantment::Enchantment;
use crate::world::item::enchantment::enchantment_helper;
use crate::world::phys::entity_hit_result::EntityHitResult;

const ENTITY_TYPE: &str = "minecraft:trident";
const ID_LOYALTY: usize = 11;
const ID_FOIL: usize = 12;
pub const WATER_INERTIA: f32 = 0.99;
const DEFAULT_DEALT_DAMAGE: bool = false;

pub struct ThrownTrident {
    pub abstract_arrow: AbstractArrow,
    dealt_damage: bool,
    pub client_side_return_trident_tick_count: i32,
}

pub fn get_default_pickup_item() -> ItemStack {
    ItemStack::new(items::TRIDENT, 1)
}

pub fn get_default_hit_ground_sound_event() -> i32 {
    sound_event("minecraft:item.trident.hit_ground")
}

impl ThrownTrident {
    pub fn of_type() -> Projectile {
        let mut data = abstract_arrow::defaults();
        data.push(DataValue::Byte(0));
        data.push(DataValue::Boolean(false));
        let mut abstract_arrow = AbstractArrow::new(get_default_pickup_item());
        abstract_arrow.set_sound_event(get_default_hit_ground_sound_event());
        let trident = ThrownTrident { abstract_arrow, dealt_damage: DEFAULT_DEALT_DAMAGE, client_side_return_trident_tick_count: 0 };
        Projectile::new(ENTITY_TYPE, abstract_arrow::DIMENSIONS, data, ProjectileKind::ThrownTrident(trident))
    }

    pub fn from_owner(level: &mut ServerLevel, owner: EntityId, trident_item: &ItemStack) -> Projectile {
        let projectile = abstract_arrow::from_owner(level, ThrownTrident::of_type(), owner, trident_item, None);
        set_loyalty_and_foil(level, projectile, trident_item)
    }

    pub fn at(level: &mut ServerLevel, x: f64, y: f64, z: f64, trident_item: &ItemStack) -> Projectile {
        let projectile = abstract_arrow::at(level, ThrownTrident::of_type(), x, y, z, trident_item, Some(trident_item));
        set_loyalty_and_foil(level, projectile, trident_item)
    }
}

fn set_loyalty_and_foil(level: &mut ServerLevel, projectile: Projectile, trident_item: &ItemStack) -> Projectile {
    let id = projectile.entity.id;
    level.entities.insert(id, LevelEntity::Projectile(Box::new(projectile)));
    let loyalty = get_loyalty_from_item(level, trident_item, id);
    let Some(LevelEntity::Projectile(mut projectile)) = level.entities.shift_remove(&id) else { unreachable!("the trident") };
    projectile.entity.data.set(ID_LOYALTY, DataValue::Byte(loyalty));
    projectile.entity.data.set(ID_FOIL, DataValue::Boolean(trident_item.has_foil()));
    *projectile
}

fn get_loyalty_from_item(level: &mut ServerLevel, trident_item: &ItemStack, trident: EntityId) -> i8 {
    enchantment_helper::get_trident_return_to_owner_acceleration(level, trident_item, trident).clamp(0, 127) as i8
}

fn thrown_trident(projectile: &Projectile) -> &ThrownTrident {
    match &projectile.kind {
        ProjectileKind::ThrownTrident(trident) => trident,
        _ => unreachable!("not a trident"),
    }
}

fn thrown_trident_mut(projectile: &mut Projectile) -> &mut ThrownTrident {
    match &mut projectile.kind {
        ProjectileKind::ThrownTrident(trident) => trident,
        _ => unreachable!("not a trident"),
    }
}

impl Projectile {
    fn loyalty(&self) -> i32 {
        match self.entity.data.get(ID_LOYALTY) {
            DataValue::Byte(loyalty) => *loyalty as i32,
            _ => 0,
        }
    }

    pub fn is_foil(&self) -> bool {
        matches!(self.entity.data.get(ID_FOIL), DataValue::Boolean(true))
    }

    pub fn thrown_trident_add_additional_save_data(&self, output: &mut Vec<(String, Nbt)>) {
        output.push(("DealtDamage".into(), Nbt::Byte(thrown_trident(self).dealt_damage as i8)));
    }

    pub fn thrown_trident_read_additional_save_data(&mut self, input: &Nbt) {
        thrown_trident_mut(self).dealt_damage = input.get("DealtDamage").and_then(Nbt::as_i64).map_or(DEFAULT_DEALT_DAMAGE, |v| v != 0);
        let loyalty = self.get_unconditional_loyalty_from_item();
        self.entity.data.set(ID_LOYALTY, DataValue::Byte(loyalty));
    }

    fn get_unconditional_loyalty_from_item(&mut self) -> i8 {
        let item = self.abstract_arrow().expect("a trident").pickup_item_stack.clone();
        let mut modified_acceleration = 0.0;
        for (id, enchantment_level) in item.enchantments().enchantments {
            let Some(enchantment) = Enchantment::by_id(&id) else { continue };
            for effect in enchantment.effects.trident_return_acceleration.iter().filter(|effect| effect.requirements.is_none()) {
                modified_acceleration = effect.effect.process(enchantment_level, &mut self.entity.random, modified_acceleration);
            }
        }
        (modified_acceleration as i32).clamp(0, 127) as i8
    }
}

impl ServerLevel {
    pub fn thrown_trident_tick(&mut self, id: EntityId) {
        if self.projectile(id).abstract_arrow().expect("a trident").in_ground_time > 4 {
            thrown_trident_mut(self.projectile_mut(id)).dealt_damage = true;
        }
        let current_owner = self.projectile_get_owner(id);
        let projectile = self.projectile(id);
        let loyalty = projectile.loyalty();
        if loyalty > 0
            && (thrown_trident(projectile).dealt_damage || projectile.is_no_physics())
            && let Some(current_owner) = current_owner
        {
            if !self.is_acceptible_return_owner(id) {
                if self.abstract_arrow(id).pickup == Pickup::Allowed {
                    let item = self.projectile(id).get_pickup_item();
                    self.spawn_at_location(id, item, 0.1);
                }
                self.entity_mut(id).discard();
            } else {
                let owner_eye_position = self.entity(current_owner).eye_position();
                let owner_width = self.entity(current_owner).dimensions.width;
                if !self.is_player(current_owner) && self.entity(id).position().distance_to(owner_eye_position) < owner_width as f64 + 1.0 {
                    self.entity_mut(id).discard();
                    return;
                }
                let projectile = self.projectile_mut(id);
                projectile.set_no_physics(true);
                let e = &mut projectile.entity;
                let vec = owner_eye_position.subtract(e.position());
                let (x, y, z) = (e.x(), e.y(), e.z());
                e.set_pos_raw(Vec3::new(x, y + vec.y * 0.015 * loyalty as f64, z));
                let accel = 0.05 * loyalty as f64;
                e.set_delta_movement(e.delta_movement.scale(0.95).add_vec(vec.normalize().scale(accel)));
                if thrown_trident(self.projectile(id)).client_side_return_trident_tick_count == 0 {
                    self.entity_play_sound(id, sound_event("minecraft:item.trident.return"), 10.0, 1.0);
                }
                thrown_trident_mut(self.projectile_mut(id)).client_side_return_trident_tick_count += 1;
            }
        }
        self.abstract_arrow_tick(id);
    }

    fn is_acceptible_return_owner(&self, id: EntityId) -> bool {
        match self.projectile_get_owner(id) {
            Some(current_owner) if self.is_alive(current_owner) => !self.players.contains_key(&current_owner) || !self.is_spectator(current_owner),
            _ => false,
        }
    }

    fn thrown_trident_find_hit_entity(&mut self, id: EntityId, from: Vec3, to: Vec3) -> Option<EntityHitResult> {
        if thrown_trident(self.projectile(id)).dealt_damage { None } else { self.abstract_arrow_find_hit_entity(id, from, to) }
    }

    pub fn thrown_trident_find_hit_entities(&mut self, id: EntityId, from: Vec3, to: Vec3) -> Vec<EntityHitResult> {
        self.thrown_trident_find_hit_entity(id, from, to).into_iter().collect()
    }

    pub fn thrown_trident_on_hit_entity(&mut self, id: EntityId, hit_result: EntityHitResult) {
        let entity = hit_result.entity;
        let mut dmg = 8.0;
        let current_owner = self.projectile_get_owner(id);
        let damage_source = DamageSource::source(damage_types::TRIDENT, Some(id), Some(current_owner.unwrap_or(id)));
        let weapon_item = self.thrown_trident_get_weapon_item(id);
        dmg = self.modify_damage(&weapon_item, entity, &damage_source, dmg);
        thrown_trident_mut(self.projectile_mut(id)).dealt_damage = true;
        let was_hurt = self.hurt_server(entity, damage_source, dmg);
        if was_hurt {
            self.do_post_attack_effects_with_item_source(entity, &damage_source, Some(&weapon_item));
            if self.is_living(entity) {
                self.do_knockback(id, entity, &damage_source);
                self.do_post_hurt_effects(id, entity);
            }
        }
        if self.projectile_receives_side_effects_on_hit(entity, was_hurt) {
            self.entity_play_sound(id, sound_event("minecraft:item.trident.hit"), 1.0, 1.0);
            let owner = self.projectile(id).owner;
            self.deflect(id, ProjectileDeflection::Reverse, Some(entity), owner, false, Vec3::new(0.02, 0.2, 0.02));
        }
    }

    fn projectile_receives_side_effects_on_hit(&self, _entity: EntityId, _was_hurt: bool) -> bool {
        true
    }

    pub fn thrown_trident_get_weapon_item(&self, id: EntityId) -> ItemStack {
        self.abstract_arrow(id).pickup_item_stack.clone()
    }

    pub fn thrown_trident_try_pickup(&mut self, id: EntityId, player: EntityId) -> bool {
        if self.abstract_arrow_try_pickup(id, player) {
            return true;
        }
        if self.projectile(id).is_no_physics() && self.projectile_owned_by(id, player) {
            let item = self.projectile(id).get_pickup_item();
            return self.player_inventory_add(player, item);
        }
        false
    }

    pub fn thrown_trident_player_touch(&mut self, id: EntityId, player: EntityId) {
        if self.projectile_owned_by(id, player) || self.projectile_get_owner(id).is_none() {
            self.abstract_arrow_player_touch(id, player);
        }
    }

    pub fn thrown_trident_tick_despawn(&mut self, id: EntityId) {
        let loyalty = self.projectile(id).loyalty();
        if self.abstract_arrow(id).pickup != Pickup::Allowed || loyalty <= 0 {
            self.abstract_arrow_tick_despawn(id);
        }
    }
}
