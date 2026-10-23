use std::collections::HashMap;
use std::sync::Arc;

use crate::datafixers::data_fix_utils;
use crate::datafixers::dsl::{self, TemplateSupplier, TypeReference, TypeTemplate, TypeTemplates};
use crate::datafixers::types::{self, RecursiveTypeFamily, Type};

pub type RegisterTypes = fn(parent: Option<&Arc<Schema>>, registry: &mut Registry, entity_types: &Arc<TypeTemplates>, block_entity_types: &Arc<TypeTemplates>);
pub type RegisterChoices = fn(parent: Option<&Arc<Schema>>) -> TypeTemplates;

pub struct SchemaClass {
    pub namespaced: bool,
    pub register_types: Option<RegisterTypes>,
    pub register_entities: Option<RegisterChoices>,
    pub register_block_entities: Option<RegisterChoices>,
}

impl SchemaClass {
    pub const NAMESPACED: SchemaClass = SchemaClass { namespaced: true, register_types: None, register_entities: None, register_block_entities: None };
}

#[derive(Default)]
pub struct Registry {
    recursive_types: Vec<String>,
    type_templates: HashMap<String, TemplateSupplier>,
}

impl Registry {
    pub fn register_type(&mut self, recursive: bool, ty: TypeReference, template: impl Fn(&Schema) -> TypeTemplate + Send + Sync + 'static) {
        self.type_templates.insert(ty.type_name().to_string(), Arc::new(template));
        if recursive && !self.recursive_types.iter().any(|name| name == ty.type_name()) {
            self.recursive_types.push(ty.type_name().to_string());
        }
    }
}

pub fn register(map: &mut TypeTemplates, name: &str, template: impl Fn(&Schema) -> TypeTemplate + Send + Sync + 'static) {
    let supplier: TemplateSupplier = Arc::new(template);
    match map.iter_mut().find(|(key, _)| key == name) {
        Some((_, slot)) => *slot = supplier,
        None => map.push((name.to_string(), supplier)),
    }
}

pub fn register_simple(map: &mut TypeTemplates, name: &str) {
    register(map, name, |_| dsl::remainder());
}

pub struct Schema {
    pub version_key: i32,
    pub name: String,
    pub parent: Option<Arc<Schema>>,
    class: &'static SchemaClass,
    registry: Registry,
    types: HashMap<String, Arc<Type>>,
    family: Arc<RecursiveTypeFamily>,
}

impl Schema {
    pub fn new(version_key: i32, parent: Option<Arc<Schema>>, class: &'static SchemaClass) -> Arc<Schema> {
        let sub_version = data_fix_utils::get_sub_version(version_key);
        let name = if sub_version == 0 {
            format!("V{}", data_fix_utils::get_version(version_key))
        } else {
            format!("V{}.{sub_version}", data_fix_utils::get_version(version_key))
        };
        let entity_types = Arc::new(dispatch_entities(class, parent.as_ref()));
        let block_entity_types = Arc::new(dispatch_block_entities(class, parent.as_ref()));
        let mut registry = Registry::default();
        dispatch_types(class, parent.as_ref(), &mut registry, &entity_types, &block_entity_types);
        let family = RecursiveTypeFamily::new(name.clone(), registry.recursive_types.len());
        let mut schema = Schema { version_key, name, parent, class, registry, types: HashMap::new(), family };
        schema.types = schema.build_types();
        Arc::new(schema)
    }

    fn build_types(&self) -> HashMap<String, Arc<Type>> {
        for (index, name) in self.registry.recursive_types.iter().enumerate() {
            self.family.set_member(index, types::instantiate(&self.get_template(name), &self.family, self));
        }
        let mut built = HashMap::new();
        for name in self.registry.type_templates.keys() {
            let ty = match self.recursive_index(name) {
                Some(index) => Arc::new(Type::RecursivePoint(self.family.clone(), index)),
                None => types::instantiate(&self.get_template(name), &self.family, self),
            };
            built.insert(name.clone(), ty);
        }
        built
    }

    fn recursive_index(&self, name: &str) -> Option<usize> {
        self.registry.recursive_types.iter().position(|recursive| recursive == name)
    }

    pub fn resolve_template(&self, name: &str) -> TypeTemplate {
        match self.registry.type_templates.get(name) {
            Some(supplier) => supplier(self),
            None => panic!("Unknown type: {name}"),
        }
    }

    pub fn id(&self, name: &str) -> TypeTemplate {
        match self.recursive_index(name) {
            Some(index) => dsl::id(index),
            None => self.get_template(name),
        }
    }

    fn get_template(&self, name: &str) -> TypeTemplate {
        dsl::named(name, self.resolve_template(name))
    }

    pub fn types(&self) -> impl Iterator<Item = &str> {
        self.types.keys().map(String::as_str)
    }

    pub fn get_type_raw(&self, ty: TypeReference) -> Arc<Type> {
        match self.types.get(ty.type_name()) {
            Some(ty) => ty.clone(),
            None => panic!("Unknown type: {}", ty.type_name()),
        }
    }

    pub fn get_type(&self, ty: TypeReference) -> Arc<Type> {
        self.get_type_raw(ty).unfold()
    }

    pub fn find_choice_type(&self, ty: TypeReference) -> Arc<Type> {
        self.get_type(ty).find_choice_type().expect("Not a choice type")
    }

    pub fn get_choice_type(&self, ty: TypeReference, choice_name: &str) -> Arc<Type> {
        let choice_name = if self.class.namespaced { crate::util::datafix::schemas::namespaced_schema::ensure_namespaced(choice_name) } else { choice_name.to_string() };
        let choice_type = self.find_choice_type(ty);
        match choice_type.choices().get(&choice_name) {
            Some(ty) => ty.clone(),
            None => panic!("Data fixer not registered for: {choice_name} in {}", ty.type_name()),
        }
    }

    pub fn get_parent(&self) -> &Arc<Schema> {
        self.parent.as_ref().expect("a parent schema")
    }

    pub fn register_entities(&self) -> TypeTemplates {
        dispatch_entities(self.class, self.parent.as_ref())
    }

    pub fn register_block_entities(&self) -> TypeTemplates {
        dispatch_block_entities(self.class, self.parent.as_ref())
    }

    pub fn register_types(&self, registry: &mut Registry, entity_types: &Arc<TypeTemplates>, block_entity_types: &Arc<TypeTemplates>) {
        dispatch_types(self.class, self.parent.as_ref(), registry, entity_types, block_entity_types);
    }
}

fn dispatch_entities(class: &SchemaClass, parent: Option<&Arc<Schema>>) -> TypeTemplates {
    match class.register_entities {
        Some(register) => register(parent),
        None => parent.expect("a parent schema").register_entities(),
    }
}

fn dispatch_block_entities(class: &SchemaClass, parent: Option<&Arc<Schema>>) -> TypeTemplates {
    match class.register_block_entities {
        Some(register) => register(parent),
        None => parent.expect("a parent schema").register_block_entities(),
    }
}

fn dispatch_types(class: &SchemaClass, parent: Option<&Arc<Schema>>, registry: &mut Registry, entity_types: &Arc<TypeTemplates>, block_entity_types: &Arc<TypeTemplates>) {
    match class.register_types {
        Some(register) => register(parent, registry, entity_types, block_entity_types),
        None => parent.expect("a parent schema").register_types(registry, entity_types, block_entity_types),
    }
}
