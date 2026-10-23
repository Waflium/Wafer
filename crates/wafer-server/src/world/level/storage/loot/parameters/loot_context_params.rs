#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LootContextParam {
    ThisEntity,
    InteractingEntity,
    TargetEntity,
    LastDamagePlayer,
    DamageSource,
    AttackingEntity,
    DirectAttackingEntity,
    Origin,
    BlockState,
    BlockEntity,
    Tool,
    ExplosionRadius,
    EnchantmentLevel,
    EnchantmentActive,
    AdditionalCostComponentAllowed,
    Container,
}

impl LootContextParam {
    pub fn name(self) -> &'static str {
        match self {
            LootContextParam::ThisEntity => "minecraft:this_entity",
            LootContextParam::InteractingEntity => "minecraft:interacting_entity",
            LootContextParam::TargetEntity => "minecraft:target_entity",
            LootContextParam::LastDamagePlayer => "minecraft:last_damage_player",
            LootContextParam::DamageSource => "minecraft:damage_source",
            LootContextParam::AttackingEntity => "minecraft:attacking_entity",
            LootContextParam::DirectAttackingEntity => "minecraft:direct_attacking_entity",
            LootContextParam::Origin => "minecraft:origin",
            LootContextParam::BlockState => "minecraft:block_state",
            LootContextParam::BlockEntity => "minecraft:block_entity",
            LootContextParam::Tool => "minecraft:tool",
            LootContextParam::ExplosionRadius => "minecraft:explosion_radius",
            LootContextParam::EnchantmentLevel => "minecraft:enchantment_level",
            LootContextParam::EnchantmentActive => "minecraft:enchantment_active",
            LootContextParam::AdditionalCostComponentAllowed => "minecraft:additional_cost_component_allowed",
            LootContextParam::Container => "minecraft:container",
        }
    }
}
