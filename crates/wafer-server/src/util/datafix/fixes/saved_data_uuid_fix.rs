use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::abstract_uuid_fix::{AbstractUUIDFix, create_uuid_from_longs};
use crate::util::datafix::fixes::references::SAVED_DATA_RAIDS;

pub struct SavedDataUUIDFix {
    abstract_uuid_fix: AbstractUUIDFix,
}

impl SavedDataUUIDFix {
    pub fn new(output_schema: &Arc<Schema>) -> SavedDataUUIDFix {
        SavedDataUUIDFix { abstract_uuid_fix: AbstractUUIDFix::new(output_schema, SAVED_DATA_RAIDS) }
    }
}

impl DataFix for SavedDataUUIDFix {
    fn base(&self) -> &DataFixBase {
        &self.abstract_uuid_fix.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.fix_type_everywhere_typed("SavedDataUUIDFix", self.get_input_schema().get_type(self.abstract_uuid_fix.type_reference), |input| {
            input.update_remainder(|container| {
                container.update("data", |tag| {
                    tag.update("Raids", |raids| {
                        raids.create_list(raids.as_stream().into_iter().map(|raid| {
                            raid.update("HeroesOfTheVillage", |heros| {
                                heros.create_list(heros.as_stream().into_iter().map(|hero| {
                                    create_uuid_from_longs(&hero, "UUIDMost", "UUIDLeast").unwrap_or_else(|| {
                                        warn!("HeroesOfTheVillage contained invalid UUIDs.");
                                        hero
                                    })
                                }))
                            })
                        }))
                    })
                })
            })
        })
    }
}
