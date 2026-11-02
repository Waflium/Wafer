use wafer_data::{DamageType, damage_types};

use crate::commands::brigadier::{CommandContext, CommandSyntaxException, SimpleCommandExceptionType};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::{Arg, Component};
use crate::server::{Server, main_thread_frames};
use crate::world::damagesource::DamageSource;

static ERROR_INVULNERABLE: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.damage.invulnerable");

type Result<T> = std::result::Result<T, CommandSyntaxException>;

fn damage_type(context: &CommandContext) -> DamageType {
    DamageType::by_name(context.get_identifier("damageType")).expect("a parsed damage type")
}

pub fn damage_generic(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let target = context.get_entity(server, source, "target")?;
    let _scope = main_thread_frames::enter_static(&["net.minecraft.server.commands.DamageCommand.lambda$register$0(DamageCommand.java:36)"]);
    damage(server, source, target, context.get_float("amount"), DamageSource::new(damage_types::GENERIC))
}

pub fn damage_typed(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let target = context.get_entity(server, source, "target")?;
    let _scope = main_thread_frames::enter_static(&["net.minecraft.server.commands.DamageCommand.lambda$register$1(DamageCommand.java:39)"]);
    damage(server, source, target, context.get_float("amount"), DamageSource::new(damage_type(context)))
}

pub fn damage_at(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let target = context.get_entity(server, source, "target")?;
    let location = context.get_coordinates("location").position(server, source);
    let mut damage_source = DamageSource::new(damage_type(context));
    damage_source.damage_source_position = Some(location);
    let _scope = main_thread_frames::enter_static(&["net.minecraft.server.commands.DamageCommand.lambda$register$2(DamageCommand.java:44)"]);
    damage(server, source, target, context.get_float("amount"), damage_source)
}

pub fn damage_by(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let target = context.get_entity(server, source, "target")?;
    let amount = context.get_float("amount");
    let entity = context.get_entity(server, source, "entity")?;
    let cause = if context.has_argument("cause") { context.get_entity(server, source, "cause")? } else { entity };
    let mut damage_source = DamageSource::new(damage_type(context));
    damage_source.direct_entity = Some(entity);
    damage_source.causing_entity = Some(cause);
    let lambda: &'static [&'static str] = if context.has_argument("cause") {
        &["net.minecraft.server.commands.DamageCommand.lambda$register$4(DamageCommand.java:56)"]
    } else {
        &["net.minecraft.server.commands.DamageCommand.lambda$register$3(DamageCommand.java:51)"]
    };
    let _scope = main_thread_frames::enter_static(lambda);
    damage(server, source, target, amount, damage_source)
}

fn damage(server: &mut Server, source: &CommandSourceStack, target: crate::world::entity::EntityId, amount: f32, damage_source: DamageSource) -> Result<i32> {
    if !main_thread_frames::within(&["net.minecraft.server.commands.DamageCommand.damage(DamageCommand.java:68)"], || {
        server.lend_entity_level(target).hurt_server(target, damage_source, amount)
    }) {
        return Err(ERROR_INVULNERABLE.create());
    }
    source.send_success(server, || Component::translatable_with("commands.damage.success", vec![Arg::Float(amount), server.entity_display_name(target).into()]), true);
    Ok(1)
}
