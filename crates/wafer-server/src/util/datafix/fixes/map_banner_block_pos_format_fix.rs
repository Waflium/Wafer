use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{find_field, type_finder};
use crate::datafixers::types::Type;
use crate::util::datafix::extra_data_fix_utils;
use crate::util::datafix::fixes::references::SAVED_DATA_MAP_DATA;

pub struct MapBannerBlockPosFormatFix {
    base: DataFixBase,
}

impl MapBannerBlockPosFormatFix {
    pub fn new(output_schema: &Arc<Schema>) -> MapBannerBlockPosFormatFix {
        MapBannerBlockPosFormatFix { base: DataFixBase::new(output_schema, false) }
    }
}

impl DataFix for MapBannerBlockPosFormatFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let ty = self.get_input_schema().get_type(SAVED_DATA_MAP_DATA);
        let data_f = find_field(&ty, "data");
        let banners_f = find_field(&data_f.ty(), "banners");
        let banner_f = type_finder(list_element(&banners_f.ty()));
        self.fix_type_everywhere_typed("MapBannerBlockPosFormatFix", ty, move |input| {
            input.update_typed(&data_f, None, |data| {
                data.update_typed(&banners_f, None, |banners| {
                    banners.update_typed(&banner_f, None, |banner| banner.update_remainder(|banner_tag| banner_tag.update("Pos", extra_data_fix_utils::fix_block_pos)))
                })
            })
        })
    }
}

fn list_element(ty: &Arc<Type>) -> Arc<Type> {
    match &**ty {
        Type::List(element) => element.clone(),
        other => panic!("Not a list type: {other:?}"),
    }
}
