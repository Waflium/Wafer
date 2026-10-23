use std::sync::{Arc, OnceLock};

use super::dynamic::Dynamic;
use super::schemas::schema::Schema;
use super::typed::Typed;
use super::types::Type;
use super::value::Value;

pub type TypedFunction = Arc<dyn Fn(Typed) -> Typed + Send + Sync>;
pub type ValueFunction = Arc<dyn Fn(Value) -> Value + Send + Sync>;
pub type DynamicFunction = Arc<dyn Fn(Dynamic) -> Dynamic + Send + Sync>;

#[derive(Clone)]
pub enum RuleFunction {
    Typed(TypedFunction),
    Value(ValueFunction),
    WriteFixAndRead(DynamicFunction),
}

#[derive(Clone)]
pub struct EverywhereRule {
    pub name: String,
    pub ty: Arc<Type>,
    pub new_type: Arc<Type>,
    pub function: RuleFunction,
}

#[derive(Clone)]
pub enum TypeRewriteRule {
    Nop,
    Seq(Vec<TypeRewriteRule>),
    Everywhere(Arc<EverywhereRule>),
}

impl TypeRewriteRule {
    pub fn nop() -> TypeRewriteRule {
        TypeRewriteRule::Nop
    }

    pub fn seq(rules: Vec<TypeRewriteRule>) -> TypeRewriteRule {
        TypeRewriteRule::Seq(rules)
    }

    pub fn everywheres(&self, out: &mut Vec<Arc<EverywhereRule>>) {
        match self {
            TypeRewriteRule::Nop => {}
            TypeRewriteRule::Seq(rules) => rules.iter().for_each(|rule| rule.everywheres(out)),
            TypeRewriteRule::Everywhere(rule) => out.push(rule.clone()),
        }
    }
}

pub struct DataFixBase {
    output_schema: Arc<Schema>,
    changes_type: bool,
    rule: OnceLock<TypeRewriteRule>,
}

impl DataFixBase {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> DataFixBase {
        DataFixBase { output_schema: output_schema.clone(), changes_type, rule: OnceLock::new() }
    }

    pub fn changes_type(&self) -> bool {
        self.changes_type
    }
}

pub trait DataFix: Send + Sync {
    fn base(&self) -> &DataFixBase;

    fn make_rule(&self) -> TypeRewriteRule;

    fn get_rule(&self) -> &TypeRewriteRule {
        self.base().rule.get_or_init(|| self.make_rule())
    }

    fn get_version_key(&self) -> i32 {
        self.base().output_schema.version_key
    }

    fn get_input_schema(&self) -> &Arc<Schema> {
        let base = self.base();
        if base.changes_type { base.output_schema.get_parent() } else { &base.output_schema }
    }

    fn get_output_schema(&self) -> &Arc<Schema> {
        &self.base().output_schema
    }

    fn fix_type_everywhere(&self, name: &str, ty: Arc<Type>, function: impl Fn(Value) -> Value + Send + Sync + 'static) -> TypeRewriteRule
    where
        Self: Sized,
    {
        let new_type = ty.clone();
        everywhere(name, ty, new_type, RuleFunction::Value(Arc::new(function)))
    }

    fn fix_type_everywhere_to(&self, name: &str, ty: Arc<Type>, new_type: Arc<Type>, function: impl Fn(Value) -> Value + Send + Sync + 'static) -> TypeRewriteRule
    where
        Self: Sized,
    {
        everywhere(name, ty, new_type, RuleFunction::Value(Arc::new(function)))
    }

    fn convert_unchecked(&self, name: &str, ty: Arc<Type>, new_type: Arc<Type>) -> TypeRewriteRule
    where
        Self: Sized,
    {
        everywhere(name, ty, new_type, RuleFunction::Value(Arc::new(|value| value)))
    }

    fn write_and_read(&self, name: &str, ty: Arc<Type>, new_type: Arc<Type>) -> TypeRewriteRule
    where
        Self: Sized,
    {
        self.write_fix_and_read(name, ty, new_type, |dynamic| dynamic)
    }

    fn write_fix_and_read(&self, name: &str, ty: Arc<Type>, new_type: Arc<Type>, fix: impl Fn(Dynamic) -> Dynamic + Send + Sync + 'static) -> TypeRewriteRule
    where
        Self: Sized,
    {
        everywhere(name, ty, new_type, RuleFunction::WriteFixAndRead(Arc::new(fix)))
    }

    fn fix_type_everywhere_typed(&self, name: &str, ty: Arc<Type>, function: impl Fn(Typed) -> Typed + Send + Sync + 'static) -> TypeRewriteRule
    where
        Self: Sized,
    {
        let new_type = ty.clone();
        everywhere(name, ty, new_type, RuleFunction::Typed(Arc::new(function)))
    }

    fn fix_type_everywhere_typed_to(&self, name: &str, ty: Arc<Type>, new_type: Arc<Type>, function: impl Fn(Typed) -> Typed + Send + Sync + 'static) -> TypeRewriteRule
    where
        Self: Sized,
    {
        everywhere(name, ty, new_type, RuleFunction::Typed(Arc::new(function)))
    }
}

fn everywhere(name: &str, ty: Arc<Type>, new_type: Arc<Type>, function: RuleFunction) -> TypeRewriteRule {
    TypeRewriteRule::Everywhere(Arc::new(EverywhereRule { name: name.to_string(), ty, new_type, function }))
}
