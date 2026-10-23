use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::find_field;
use crate::util::datafix::fixes::references::SAVED_DATA_SCOREBOARD;

const SLOT_RENAMES: [(&str, &str); 19] = [
    ("slot_0", "list"),
    ("slot_1", "sidebar"),
    ("slot_2", "below_name"),
    ("slot_3", "sidebar.team.black"),
    ("slot_4", "sidebar.team.dark_blue"),
    ("slot_5", "sidebar.team.dark_green"),
    ("slot_6", "sidebar.team.dark_aqua"),
    ("slot_7", "sidebar.team.dark_red"),
    ("slot_8", "sidebar.team.dark_purple"),
    ("slot_9", "sidebar.team.gold"),
    ("slot_10", "sidebar.team.gray"),
    ("slot_11", "sidebar.team.dark_gray"),
    ("slot_12", "sidebar.team.blue"),
    ("slot_13", "sidebar.team.green"),
    ("slot_14", "sidebar.team.aqua"),
    ("slot_15", "sidebar.team.red"),
    ("slot_16", "sidebar.team.light_purple"),
    ("slot_17", "sidebar.team.yellow"),
    ("slot_18", "sidebar.team.white"),
];

pub struct ScoreboardDisplaySlotFix {
    base: DataFixBase,
}

impl ScoreboardDisplaySlotFix {
    pub fn new(output_schema: &Arc<Schema>) -> ScoreboardDisplaySlotFix {
        ScoreboardDisplaySlotFix { base: DataFixBase::new(output_schema, false) }
    }
}

fn rename(key: &str) -> Option<&'static str> {
    SLOT_RENAMES.iter().find(|(from, _)| *from == key).map(|(_, to)| *to)
}

impl DataFix for ScoreboardDisplaySlotFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let scoreboard_type = self.get_input_schema().get_type(SAVED_DATA_SCOREBOARD);
        let root_tag_finder = find_field(&scoreboard_type, "data");
        self.fix_type_everywhere_typed("Scoreboard DisplaySlot rename", scoreboard_type, move |input| {
            input.update_typed(&root_tag_finder, None, |scoreboard_info| {
                scoreboard_info.update_remainder(|tag| {
                    tag.update("DisplaySlots", |slots| {
                        slots.update_map_values(|(key, value)| {
                            let renamed = key.as_string().into_result().ok().and_then(|slot| rename(&slot)).map(|slot| key.create_string(slot));
                            (renamed.unwrap_or(key), value)
                        })
                    })
                })
            })
        })
    }
}
