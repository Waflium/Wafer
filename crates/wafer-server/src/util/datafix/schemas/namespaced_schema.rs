use crate::datafixers::schemas::schema::SchemaClass;

pub static NAMESPACED_SCHEMA: SchemaClass = SchemaClass::NAMESPACED;

pub fn ensure_namespaced(input: &str) -> String {
    crate::resources::identifier::parse(input).unwrap_or_else(|_| input.to_string())
}
