use super::material_rule;
use crate::world::item::component::codec::Field;
use crate::world::item::component::types::req;
use crate::world::level::levelgen::material::condition::material_condition;

pub fn fields() -> Vec<Field> {
    vec![req("if_true", material_condition::holder_codec()), req("then_run", material_rule::holder_codec())]
}
