use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::value::Value;
use crate::util::datafix::fixes::references::TEXT_COMPONENT;

pub struct TextComponentStringifiedFlagsFix {
    base: DataFixBase,
}

impl TextComponentStringifiedFlagsFix {
    pub fn new(output_schema: &Arc<Schema>) -> TextComponentStringifiedFlagsFix {
        TextComponentStringifiedFlagsFix { base: DataFixBase::new(output_schema, false) }
    }
}

impl DataFix for TextComponentStringifiedFlagsFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.fix_type_everywhere("TextComponentStringyFlagsFix", self.get_input_schema().get_type(TEXT_COMPONENT), |named| match named {
            Value::Right(p2) => {
                let Value::Pair(extra, p3) = *p2 else { panic!("Unexpected text component value") };
                let Value::Pair(separator, p4) = *p3 else { panic!("Unexpected text component value") };
                let Value::Pair(hover_event, remainder) = *p4 else { panic!("Unexpected text component value") };
                let Value::Dynamic(remainder) = *remainder else { panic!("Unexpected text component value") };
                let remainder = remainder
                    .update("bold", string_to_bool)
                    .update("italic", string_to_bool)
                    .update("underlined", string_to_bool)
                    .update("strikethrough", string_to_bool)
                    .update("obfuscated", string_to_bool);
                Value::Right(Box::new(Value::Pair(extra, Box::new(Value::Pair(separator, Box::new(Value::Pair(hover_event, Box::new(Value::Dynamic(remainder)))))))))
            }
            other => other,
        })
    }
}

fn string_to_bool(input: Dynamic) -> Dynamic {
    match input.as_string().into_result() {
        Ok(string) => input.create_boolean(string.eq_ignore_ascii_case("true")),
        Err(_) => input,
    }
}
