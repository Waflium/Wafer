use super::loot_context_params::LootContextParam::{self, *};

pub struct ContextKeySet {
    pub name: &'static str,
    pub required: &'static [LootContextParam],
    pub optional: &'static [LootContextParam],
}

impl ContextKeySet {
    pub fn allows(&self, param: LootContextParam) -> bool {
        self.required.contains(&param) || self.optional.contains(&param)
    }
}

const fn set(name: &'static str, required: &'static [LootContextParam], optional: &'static [LootContextParam]) -> ContextKeySet {
    ContextKeySet { name, required, optional }
}

pub static EMPTY: ContextKeySet = set("minecraft:empty", &[], &[]);
pub static ALL_PARAMS: ContextKeySet = set(
    "minecraft:generic",
    &[
        ThisEntity,
        LastDamagePlayer,
        DamageSource,
        AttackingEntity,
        DirectAttackingEntity,
        Origin,
        BlockState,
        BlockEntity,
        Tool,
        ExplosionRadius,
        AdditionalCostComponentAllowed,
        Container,
        InteractingEntity,
        TargetEntity,
        EnchantmentActive,
        EnchantmentLevel,
    ],
    &[],
);
pub static CHEST: ContextKeySet = set("minecraft:chest", &[Origin], &[ThisEntity]);
pub static COMMAND: ContextKeySet = set("minecraft:command", &[Origin], &[ThisEntity]);
pub static COMMAND_SLOT_SOURCE: ContextKeySet = set("minecraft:command_slot_source", &[Origin, Container], &[ThisEntity]);
pub static COMMAND_COMPUTE_DEFAULT: ContextKeySet = set("minecraft:command_compute_default", &[Origin], &[ThisEntity]);
pub static COMMAND_COMPUTE_POSITION: ContextKeySet = set("minecraft:command_compute_position", &[Origin, BlockState], &[ThisEntity, BlockEntity]);
pub static COMMAND_COMPUTE_ENTITY: ContextKeySet = set("minecraft:command_compute_entity", &[Origin, TargetEntity], &[ThisEntity]);
pub static SELECTOR: ContextKeySet = set("minecraft:selector", &[Origin, ThisEntity], &[]);
pub static VILLAGER_TRADE: ContextKeySet = set("minecraft:villager_trade", &[Origin, ThisEntity, AdditionalCostComponentAllowed], &[]);
pub static FISHING: ContextKeySet = set("minecraft:fishing", &[Origin, Tool], &[ThisEntity]);
pub static ENTITY: ContextKeySet = set("minecraft:entity", &[ThisEntity, Origin, DamageSource], &[AttackingEntity, DirectAttackingEntity, LastDamagePlayer]);
pub static EQUIPMENT: ContextKeySet = set("minecraft:equipment", &[Origin, ThisEntity], &[]);
pub static ARCHAEOLOGY: ContextKeySet = set("minecraft:archaeology", &[Origin, ThisEntity, Tool], &[]);
pub static GIFT: ContextKeySet = set("minecraft:gift", &[Origin, ThisEntity], &[]);
pub static PIGLIN_BARTER: ContextKeySet = set("minecraft:barter", &[ThisEntity], &[]);
pub static VAULT: ContextKeySet = set("minecraft:vault", &[Origin], &[ThisEntity, Tool]);
pub static ADVANCEMENT_REWARD: ContextKeySet = set("minecraft:advancement_reward", &[ThisEntity, Origin], &[]);
pub static ADVANCEMENT_ENTITY: ContextKeySet = set("minecraft:advancement_entity", &[ThisEntity, Origin], &[]);
pub static ADVANCEMENT_LOCATION: ContextKeySet = set("minecraft:advancement_location", &[ThisEntity, Origin, Tool, BlockState], &[BlockEntity]);
pub static BLOCK_USE: ContextKeySet = set("minecraft:block_use", &[ThisEntity, Origin, BlockState], &[]);
pub static BLOCK: ContextKeySet = set("minecraft:block", &[BlockState, Origin, Tool], &[ThisEntity, BlockEntity, ExplosionRadius]);
pub static SHEARING: ContextKeySet = set("minecraft:shearing", &[Origin, ThisEntity, Tool], &[]);
pub static ENTITY_INTERACT: ContextKeySet = set("minecraft:entity_interact", &[TargetEntity, Tool], &[InteractingEntity]);
pub static BLOCK_INTERACT: ContextKeySet = set("minecraft:block_interact", &[BlockState, Origin], &[BlockEntity, InteractingEntity, Tool]);
pub static CONTAINER_PROCESS: ContextKeySet = set("minecraft:container_process", &[BlockEntity, BlockState, Container, Origin], &[]);
pub static ENCHANTED_DAMAGE: ContextKeySet =
    set("minecraft:enchanted_damage", &[ThisEntity, EnchantmentLevel, Origin, DamageSource], &[DirectAttackingEntity, AttackingEntity]);
pub static ENCHANTED_ITEM: ContextKeySet = set("minecraft:enchanted_item", &[Tool, EnchantmentLevel], &[]);
pub static ENCHANTED_LOCATION: ContextKeySet = set("minecraft:enchanted_location", &[ThisEntity, EnchantmentLevel, Origin, EnchantmentActive], &[]);
pub static ENCHANTED_ENTITY: ContextKeySet = set("minecraft:enchanted_entity", &[ThisEntity, EnchantmentLevel, Origin], &[]);
pub static HIT_BLOCK: ContextKeySet = set("minecraft:hit_block", &[ThisEntity, EnchantmentLevel, Origin, BlockState], &[]);

static ALL: [&ContextKeySet; 31] = [
    &EMPTY,
    &ALL_PARAMS,
    &CHEST,
    &COMMAND,
    &COMMAND_SLOT_SOURCE,
    &COMMAND_COMPUTE_DEFAULT,
    &COMMAND_COMPUTE_POSITION,
    &COMMAND_COMPUTE_ENTITY,
    &SELECTOR,
    &VILLAGER_TRADE,
    &FISHING,
    &ENTITY,
    &EQUIPMENT,
    &ARCHAEOLOGY,
    &GIFT,
    &PIGLIN_BARTER,
    &VAULT,
    &ADVANCEMENT_REWARD,
    &ADVANCEMENT_ENTITY,
    &ADVANCEMENT_LOCATION,
    &BLOCK_USE,
    &BLOCK,
    &SHEARING,
    &ENTITY_INTERACT,
    &BLOCK_INTERACT,
    &CONTAINER_PROCESS,
    &ENCHANTED_DAMAGE,
    &ENCHANTED_ITEM,
    &ENCHANTED_LOCATION,
    &ENCHANTED_ENTITY,
    &HIT_BLOCK,
];

pub fn by_name(name: &str) -> Option<&'static ContextKeySet> {
    ALL.iter().copied().find(|set| set.name == name)
}
