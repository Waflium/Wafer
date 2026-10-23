use std::sync::Arc;

use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::Typed;
use crate::util::datafix::fixes::named_entity_fix::NamedEntityFix;
use crate::util::datafix::fixes::references::ENTITY;
use crate::util::datafix::schemas::namespaced_schema::ensure_namespaced;

pub struct FixWolfHealth;

impl FixWolfHealth {
    pub fn new(output_schema: &Arc<Schema>) -> NamedEntityFix {
        NamedEntityFix::new(output_schema, false, "FixWolfHealth", ENTITY, "minecraft:wolf", fix)
    }
}

fn fix(entity: Typed) -> Typed {
    entity.update_remainder(|mut dynamic| {
        let mut health_adjusted = false;
        dynamic = dynamic.update("Attributes", |attributes| {
            let entries = attributes.as_stream();
            let fixed = entries.into_iter().map(|attribute| {
                if ensure_namespaced(&attribute.get("Name").as_string_or("")) == "minecraft:generic.max_health" {
                    attribute.update("Base", |base| {
                        if base.as_double(0.0) == 20.0 {
                            health_adjusted = true;
                            base.create_double(40.0)
                        } else {
                            base
                        }
                    })
                } else {
                    attribute
                }
            });
            let fixed: Vec<Dynamic> = fixed.collect();
            attributes.create_list(fixed)
        });
        if health_adjusted {
            dynamic = dynamic.update("Health", |health| health.create_float(health.as_float(0.0) * 2.0));
        }
        dynamic
    })
}
