use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::STRUCTURE_FEATURE;

pub struct IglooMetadataRemovalFix {
    base: DataFixBase,
}

impl IglooMetadataRemovalFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> IglooMetadataRemovalFix {
        IglooMetadataRemovalFix { base: DataFixBase::new(output_schema, changes_type) }
    }
}

impl DataFix for IglooMetadataRemovalFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let ty = self.get_input_schema().get_type(STRUCTURE_FEATURE);
        self.fix_type_everywhere_typed("IglooMetadataRemovalFix", ty, |typed| typed.update_remainder(fix_tag))
    }
}

fn fix_tag(input: Dynamic) -> Dynamic {
    let is_igloo_only = input.get("Children").as_stream_opt().into_result().map(|children| children.iter().all(is_igloo_piece)).unwrap_or(false);
    if is_igloo_only {
        let id = input.create_string("Igloo");
        input.set("id", id).remove("Children")
    } else {
        input.update("Children", remove_igloo_pieces)
    }
}

fn remove_igloo_pieces(children: Dynamic) -> Dynamic {
    match children.as_stream_opt().into_result() {
        Ok(pieces) => children.create_list(pieces.into_iter().filter(|piece| !is_igloo_piece(piece))),
        Err(_) => children,
    }
}

fn is_igloo_piece(tag: &Dynamic) -> bool {
    tag.get("id").as_string_or("") == "Iglu"
}
