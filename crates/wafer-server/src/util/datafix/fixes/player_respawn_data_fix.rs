use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::PLAYER;

pub struct PlayerRespawnDataFix {
    base: DataFixBase,
}

impl PlayerRespawnDataFix {
    pub fn new(output_schema: &Arc<Schema>) -> PlayerRespawnDataFix {
        PlayerRespawnDataFix { base: DataFixBase::new(output_schema, false) }
    }
}

impl DataFix for PlayerRespawnDataFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.fix_type_everywhere_typed("PlayerRespawnDataFix", self.get_input_schema().get_type(PLAYER), |input| {
            input.update_remainder(|tag| {
                tag.update("respawn", |respawn_tag| {
                    let dimension = respawn_tag.create_string(respawn_tag.get("dimension").as_string_or("minecraft:overworld"));
                    let yaw = respawn_tag.create_float(respawn_tag.get("angle").as_float(0.0));
                    let pitch = respawn_tag.create_float(0.0);
                    respawn_tag.set("dimension", dimension).set("yaw", yaw).set("pitch", pitch).remove("angle")
                })
            })
        })
    }
}
