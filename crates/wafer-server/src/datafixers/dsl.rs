use std::sync::Arc;

use super::dynamic::Dynamic;
use super::schemas::schema::Schema;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct TypeReference(pub &'static str);

impl TypeReference {
    pub fn type_name(self) -> &'static str {
        self.0
    }

    pub fn in_schema(self, schema: &Schema) -> TypeTemplate {
        schema.id(self.0)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConstType {
    Bool,
    Int,
    Long,
    Byte,
    Short,
    Float,
    Double,
    String,
    NamespacedString,
}

pub type HookFunction = fn(Dynamic) -> Dynamic;

pub fn identity_hook(value: Dynamic) -> Dynamic {
    value
}

pub type TemplateSupplier = Arc<dyn Fn(&Schema) -> TypeTemplate + Send + Sync>;

pub type TypeTemplates = Vec<(String, TemplateSupplier)>;

#[derive(Clone)]
pub enum TypeTemplate {
    Const(ConstType),
    Remainder,
    EmptyPart,
    Named(Arc<str>, Box<TypeTemplate>),
    Field(Arc<str>, Box<TypeTemplate>),
    Product(Box<TypeTemplate>, Box<TypeTemplate>),
    Sum(Box<TypeTemplate>, Box<TypeTemplate>),
    List(Box<TypeTemplate>),
    CompoundList(Box<TypeTemplate>, Box<TypeTemplate>),
    TaggedChoice(Arc<str>, ConstType, Arc<TypeTemplates>),
    Hook(Box<TypeTemplate>, HookFunction, HookFunction),
    Id(usize),
}

pub fn const_type(ty: ConstType) -> TypeTemplate {
    TypeTemplate::Const(ty)
}

pub fn bool_type() -> TypeTemplate {
    TypeTemplate::Const(ConstType::Bool)
}

pub fn int_type() -> TypeTemplate {
    TypeTemplate::Const(ConstType::Int)
}

pub fn string() -> TypeTemplate {
    TypeTemplate::Const(ConstType::String)
}

pub fn remainder() -> TypeTemplate {
    TypeTemplate::Remainder
}

pub fn empty_part() -> TypeTemplate {
    TypeTemplate::EmptyPart
}

pub fn named(name: &str, element: TypeTemplate) -> TypeTemplate {
    TypeTemplate::Named(name.into(), Box::new(element))
}

pub fn field(name: &str, element: TypeTemplate) -> TypeTemplate {
    TypeTemplate::Field(name.into(), Box::new(element))
}

pub fn list(element: TypeTemplate) -> TypeTemplate {
    TypeTemplate::List(Box::new(element))
}

pub fn compound_list(element: TypeTemplate) -> TypeTemplate {
    compound_list_with_key(TypeTemplate::Const(ConstType::String), element)
}

pub fn compound_list_with_key(key: TypeTemplate, element: TypeTemplate) -> TypeTemplate {
    and(TypeTemplate::CompoundList(Box::new(key), Box::new(element)), remainder())
}

pub fn or(left: TypeTemplate, right: TypeTemplate) -> TypeTemplate {
    TypeTemplate::Sum(Box::new(left), Box::new(right))
}

pub fn optional(value: TypeTemplate) -> TypeTemplate {
    or(value, empty_part())
}

pub fn and(first: TypeTemplate, second: TypeTemplate) -> TypeTemplate {
    TypeTemplate::Product(Box::new(first), Box::new(second))
}

pub fn and_all(types: Vec<TypeTemplate>) -> TypeTemplate {
    let mut types = types.into_iter().rev();
    let last = types.next().expect("Must have at least one type");
    types.fold(last, |result, ty| and(ty, result))
}

pub fn all_with_remainder(types: Vec<TypeTemplate>) -> TypeTemplate {
    let mut types = types;
    types.push(remainder());
    and_all(types)
}

pub fn fields(fields: Vec<(&str, TypeTemplate)>) -> TypeTemplate {
    all_with_remainder(fields.into_iter().map(|(name, element)| field(name, element)).collect())
}

pub fn fields_with_rest(fields: Vec<(&str, TypeTemplate)>, rest: TypeTemplate) -> TypeTemplate {
    let mut types: Vec<TypeTemplate> = fields.into_iter().map(|(name, element)| field(name, element)).collect();
    types.push(rest);
    and_all(types)
}

pub fn optional_fields(fields: Vec<(&str, TypeTemplate)>) -> TypeTemplate {
    all_with_remainder(fields.into_iter().map(|(name, element)| optional(field(name, element))).collect())
}

pub fn optional_fields_with_rest(fields: Vec<(&str, TypeTemplate)>, rest: TypeTemplate) -> TypeTemplate {
    let mut types: Vec<TypeTemplate> = fields.into_iter().map(|(name, element)| optional(field(name, element))).collect();
    types.push(rest);
    and_all(types)
}

pub fn tagged_choice_lazy(name: &str, key_type: ConstType, templates: Arc<TypeTemplates>) -> TypeTemplate {
    TypeTemplate::TaggedChoice(name.into(), key_type, templates)
}

pub fn hook(template: TypeTemplate, pre_read: HookFunction, post_write: HookFunction) -> TypeTemplate {
    TypeTemplate::Hook(Box::new(template), pre_read, post_write)
}

pub fn id(index: usize) -> TypeTemplate {
    TypeTemplate::Id(index)
}
