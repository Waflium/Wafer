use std::sync::Arc;

use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::attributes_rename_fix::AttributesRenameFix;
use crate::util::datafix::schemas::namespaced_schema::ensure_namespaced;

const PREFIXES: [&str; 4] = ["generic.", "horse.", "player.", "zombie."];

pub struct AttributeIdPrefixFix;

impl AttributeIdPrefixFix {
    pub fn new(output_schema: &Arc<Schema>) -> AttributesRenameFix {
        AttributesRenameFix::with_format(output_schema, "AttributeIdPrefixFix", replace_id, true)
    }
}

fn replace_id(id: &str) -> String {
    let namespaced_id = ensure_namespaced(id);
    for prefix in PREFIXES {
        let namespaced_prefix = ensure_namespaced(prefix);
        if let Some(rest) = namespaced_id.strip_prefix(&namespaced_prefix) {
            return format!("minecraft:{rest}");
        }
    }
    id.to_string()
}
