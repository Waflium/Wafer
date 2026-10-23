use std::sync::Arc;

use super::dynamic::{DataResult, Dynamic};
use super::types::{Type, choice_order, equals};
use super::value::{self, Value};

#[derive(Clone)]
pub enum OpticFinder {
    Remainder,
    Type(Arc<Type>),
    Field(String, Arc<Type>),
    NamedChoice(String, Arc<Type>),
}

pub fn remainder_finder() -> OpticFinder {
    OpticFinder::Remainder
}

pub fn type_finder(ty: Arc<Type>) -> OpticFinder {
    OpticFinder::Type(ty)
}

pub fn field_finder(name: &str, ty: Arc<Type>) -> OpticFinder {
    OpticFinder::Field(name.to_string(), ty)
}

pub fn named_choice(name: &str, ty: Arc<Type>) -> OpticFinder {
    OpticFinder::NamedChoice(name.to_string(), ty)
}

pub fn find_field(ty: &Arc<Type>, name: &str) -> OpticFinder {
    match ty.find_field_type(name) {
        Some(field_type) => OpticFinder::Field(name.to_string(), field_type),
        None => panic!("Field not found: {name}"),
    }
}

impl OpticFinder {
    pub fn ty(&self) -> Arc<Type> {
        match self {
            OpticFinder::Remainder => Arc::new(Type::Remainder),
            OpticFinder::Type(ty) | OpticFinder::Field(_, ty) | OpticFinder::NamedChoice(_, ty) => ty.clone(),
        }
    }

    fn matches(&self, target: &Arc<Type>) -> Match {
        match self {
            OpticFinder::Remainder => match &**target {
                Type::Remainder => Match::Found(target.clone()),
                Type::Field(..) => Match::Stop,
                _ => Match::Continue,
            },
            OpticFinder::Type(ty) => {
                if equals(ty, target, true) {
                    Match::Found(target.clone())
                } else if matches!(&**target, Type::Field(..)) {
                    Match::Stop
                } else {
                    Match::Continue
                }
            }
            OpticFinder::Field(name, ty) => match &**target {
                Type::Field(field_name, element) if **field_name == **name => {
                    if equals(ty, element, false) { Match::Found(element.clone()) } else { Match::Stop }
                }
                Type::Field(..) => Match::Stop,
                Type::TaggedChoice(choice) if *choice.name == **name => match &**ty {
                    Type::Const(key_type) if *key_type == choice.key_type => Match::Found(ty.clone()),
                    _ => Match::Stop,
                },
                _ => Match::Continue,
            },
            OpticFinder::NamedChoice(name, ty) => match &**target {
                Type::TaggedChoice(choice) => match choice.get(name) {
                    Some(element) if equals(ty, element, false) => Match::Found(element.clone()),
                    Some(_) => Match::Stop,
                    None => Match::Continue,
                },
                Type::Field(..) => Match::Stop,
                _ => Match::Continue,
            },
        }
    }
}

enum Match {
    Found(Arc<Type>),
    Continue,
    Stop,
}

fn resolves(ty: &Arc<Type>, finder: &OpticFinder) -> bool {
    match finder.matches(ty) {
        Match::Found(_) => return true,
        Match::Stop => return false,
        Match::Continue => {}
    }
    match &**ty {
        Type::Named(_, element) | Type::Hook(element, _, _) | Type::List(element) => resolves(element, finder),
        Type::RecursivePoint(..) => resolves(&ty.unfold(), finder),
        Type::Product(first, second) | Type::Sum(first, second) | Type::CompoundList(first, second) => resolves(first, finder) || resolves(second, finder),
        Type::TaggedChoice(choice) => choice.types.iter().any(|(_, element)| resolves(element, finder)),
        _ => false,
    }
}

fn review(ty: &Arc<Type>, finder: &OpticFinder, new_value: &Value) -> Option<Value> {
    if let Match::Found(_) = finder.matches(ty) {
        return Some(new_value.clone());
    }
    match &**ty {
        Type::Named(_, element) | Type::Hook(element, _, _) => review(element, finder, new_value),
        Type::RecursivePoint(..) => review(&ty.unfold(), finder, new_value),
        Type::Sum(first, _) if resolves(first, finder) => review(first, finder, new_value).map(|built| Value::Left(Box::new(built))),
        Type::Sum(_, second) if resolves(second, finder) => review(second, finder, new_value).map(|built| Value::Right(Box::new(built))),
        _ => None,
    }
}

fn set_value(ty: &Arc<Type>, value: &mut Value, finder: &OpticFinder, new_value: &Value) {
    if let Match::Found(_) = finder.matches(ty) {
        visit(ty, value, finder, &mut |_, focus| *focus = new_value.clone());
        return;
    }
    match (&**ty, &mut *value) {
        (Type::Named(_, element) | Type::Hook(element, _, _), value) => set_value(element, value, finder, new_value),
        (Type::RecursivePoint(..), value) => set_value(&ty.unfold(), value, finder, new_value),
        (Type::Product(first, second), Value::Pair(first_value, second_value)) => {
            if resolves(first, finder) {
                set_value(first, first_value, finder, new_value);
            } else {
                set_value(second, second_value, finder, new_value);
            }
        }
        (Type::Sum(first, _), Value::Left(inner)) if resolves(first, finder) => set_value(first, inner, finder, new_value),
        (Type::Sum(_, second), Value::Right(inner)) if resolves(second, finder) => set_value(second, inner, finder, new_value),
        (Type::Sum(first, _), Value::Right(_)) => {
            if resolves(first, finder)
                && let Some(built) = review(first, finder, new_value)
            {
                *value = Value::Left(Box::new(built));
            }
        }
        (Type::Sum(_, second), Value::Left(_)) => {
            if resolves(second, finder)
                && let Some(built) = review(second, finder, new_value)
            {
                *value = Value::Right(Box::new(built));
            }
        }
        _ => visit(ty, value, finder, &mut |_, focus| *focus = new_value.clone()),
    }
}

fn visit(ty: &Arc<Type>, value: &mut Value, finder: &OpticFinder, f: &mut dyn FnMut(&Arc<Type>, &mut Value)) {
    if let Match::Found(focus) = finder.matches(ty) {
        match (finder, &**ty, value) {
            (OpticFinder::NamedChoice(name, _), Type::TaggedChoice(_), Value::Choice(key, inner)) => {
                if key == name {
                    f(&focus, inner);
                }
            }
            (OpticFinder::NamedChoice(..), _, _) => {}
            (OpticFinder::Field(..), Type::TaggedChoice(_), Value::Choice(key, _)) => {
                let mut key_value = Value::Prim(wafer_protocol::nbt::Nbt::String(std::mem::take(key)));
                f(&focus, &mut key_value);
                if let Value::Prim(wafer_protocol::nbt::Nbt::String(new_key)) = key_value {
                    *key = new_key;
                }
            }
            (_, _, value) => f(&focus, value),
        }
        return;
    }
    match (&**ty, value) {
        (Type::Named(_, element) | Type::Hook(element, _, _), value) => visit(element, value, finder, f),
        (Type::RecursivePoint(..), value) => visit(&ty.unfold(), value, finder, f),
        (Type::Product(first, second), Value::Pair(first_value, second_value)) => {
            if resolves(first, finder) {
                visit(first, first_value, finder, f);
            } else {
                visit(second, second_value, finder, f);
            }
        }
        (Type::Sum(first, _), Value::Left(inner)) => {
            if resolves(first, finder) {
                visit(first, inner, finder, f);
            }
        }
        (Type::Sum(_, second), Value::Right(inner)) => {
            if resolves(second, finder) {
                visit(second, inner, finder, f);
            }
        }
        (Type::List(element), Value::List(values)) => {
            for value in values {
                visit(element, value, finder, f);
            }
        }
        (Type::CompoundList(key, element), Value::CompoundList(entries)) => {
            if resolves(key, finder) {
                for (key_value, _) in entries {
                    visit(key, key_value, finder, f);
                }
            } else if resolves(element, finder) {
                for (_, entry_value) in entries {
                    visit(element, entry_value, finder, f);
                }
            }
        }
        (Type::TaggedChoice(choice), Value::Choice(key, inner)) => {
            if let Some(element) = choice.get(key)
                && resolves(element, finder)
            {
                visit(element, inner, finder, f);
            }
        }
        _ => {}
    }
}

pub fn with_focus_type(ty: &Arc<Type>, finder: &OpticFinder, new_focus: &Arc<Type>) -> Arc<Type> {
    if let Match::Found(_) = finder.matches(ty) {
        return match (finder, &**ty) {
            (OpticFinder::Field(..), Type::Field(name, _)) => Arc::new(Type::Field(name.clone(), new_focus.clone())),
            (OpticFinder::NamedChoice(name, _), Type::TaggedChoice(choice)) => Arc::new(Type::TaggedChoice(Arc::new(crate::datafixers::types::TaggedChoiceType {
                name: choice.name.clone(),
                key_type: choice.key_type,
                types: choice.types.iter().map(|(key, element)| (key.clone(), if key == name { new_focus.clone() } else { element.clone() })).collect(),
            }))),
            (OpticFinder::Field(..), Type::TaggedChoice(_)) => ty.clone(),
            _ => new_focus.clone(),
        };
    }
    let sub = |element: &Arc<Type>| if resolves(element, finder) { with_focus_type(element, finder, new_focus) } else { element.clone() };
    match &**ty {
        Type::Named(name, element) => Arc::new(Type::Named(name.clone(), sub(element))),
        Type::Hook(element, pre_read, post_write) => Arc::new(Type::Hook(sub(element), *pre_read, *post_write)),
        Type::List(element) => Arc::new(Type::List(sub(element))),
        Type::RecursivePoint(..) => with_focus_type(&ty.unfold(), finder, new_focus),
        Type::Product(first, second) => {
            if resolves(first, finder) { Arc::new(Type::Product(sub(first), second.clone())) } else { Arc::new(Type::Product(first.clone(), sub(second))) }
        }
        Type::Sum(first, second) => Arc::new(Type::Sum(sub(first), sub(second))),
        Type::CompoundList(key, element) => {
            if resolves(key, finder) { Arc::new(Type::CompoundList(sub(key), element.clone())) } else { Arc::new(Type::CompoundList(key.clone(), sub(element))) }
        }
        Type::TaggedChoice(choice) => Arc::new(Type::TaggedChoice(Arc::new(crate::datafixers::types::TaggedChoiceType {
            name: choice.name.clone(),
            key_type: choice.key_type,
            types: choice.types.iter().map(|(key, element)| (key.clone(), sub(element))).collect(),
        }))),
        _ => ty.clone(),
    }
}

pub fn point(ty: &Arc<Type>) -> Option<Value> {
    match &**ty {
        Type::Const(_) => None,
        Type::Remainder => Some(Value::Dynamic(Dynamic::empty())),
        Type::EmptyPart => Some(Value::Unit),
        Type::Named(_, element) | Type::Field(_, element) | Type::Hook(element, _, _) => point(element),
        Type::RecursivePoint(..) => point(&ty.unfold()),
        Type::Product(first, second) => Some(Value::Pair(Box::new(point(first)?), Box::new(point(second)?))),
        Type::Sum(first, second) => point(second).map(|value| Value::Right(Box::new(value))).or_else(|| point(first).map(|value| Value::Left(Box::new(value)))),
        Type::List(_) => Some(Value::List(Vec::new())),
        Type::CompoundList(..) => Some(Value::CompoundList(Vec::new())),
        Type::TaggedChoice(choice) => {
            let keys: Vec<&str> = choice.types.iter().map(|(key, _)| key.as_str()).collect();
            choice_order::point_order(&keys).into_iter().find_map(|item| {
                let (key, element) = &choice.types[item];
                point(element).map(|value| Value::Choice(key.clone(), Box::new(value)))
            })
        }
    }
}

#[derive(Clone, Debug)]
pub struct Typed {
    pub ty: Arc<Type>,
    pub value: Value,
}

impl Typed {
    pub fn new(ty: Arc<Type>, value: Value) -> Typed {
        Typed { ty, value }
    }

    pub fn read_typed(ty: &Arc<Type>, input: Dynamic) -> DataResult<(Typed, Dynamic)> {
        value::decode(ty, input.value).map(|(value, rest)| (Typed::new(ty.clone(), value), Dynamic::of(rest)))
    }

    pub fn get_type(&self) -> &Arc<Type> {
        &self.ty
    }

    pub fn write(&self) -> DataResult<Dynamic> {
        value::encode_start(&self.ty, &self.value).map(Dynamic::of)
    }

    fn check(&self, finder: &OpticFinder) {
        if !resolves(&self.ty, finder) {
            panic!("Not found: {:?} (in type: {:?})", finder.ty(), self.ty);
        }
    }

    pub fn get_optional_typed(&self, finder: &OpticFinder) -> Option<Typed> {
        self.check(finder);
        let mut found = None;
        let mut value = self.value.clone();
        visit(&self.ty, &mut value, finder, &mut |ty, value| {
            if found.is_none() {
                found = Some(Typed::new(ty.clone(), value.clone()));
            }
        });
        found
    }

    pub fn get_optional(&self, finder: &OpticFinder) -> Option<Value> {
        self.get_optional_typed(finder).map(|typed| typed.value)
    }

    pub fn get_typed(&self, finder: &OpticFinder) -> Typed {
        self.get_optional_typed(finder).expect("a value at the optic")
    }

    pub fn get_or_create_typed(&self, finder: &OpticFinder) -> Typed {
        self.get_optional_typed(finder).unwrap_or_else(|| {
            let ty = finder.ty();
            match point(&ty) {
                Some(value) => Typed::new(ty, value),
                None => panic!("Could not create default value for type: {ty:?}"),
            }
        })
    }

    pub fn get_all_typed(&self, finder: &OpticFinder) -> Vec<Typed> {
        self.check(finder);
        let mut found = Vec::new();
        let mut value = self.value.clone();
        let ty = finder.ty();
        visit(&self.ty, &mut value, finder, &mut |_, value| found.push(Typed::new(ty.clone(), value.clone())));
        found
    }

    pub fn update_typed(mut self, finder: &OpticFinder, new_type: Option<&Arc<Type>>, mut updater: impl FnMut(Typed) -> Typed) -> Typed {
        self.check(finder);
        let finder_type = finder.ty();
        let expected = new_type.cloned().unwrap_or_else(|| finder_type.clone());
        visit(&self.ty.clone(), &mut self.value, finder, &mut |_, value| {
            let result = updater(Typed::new(finder_type.clone(), std::mem::replace(value, Value::Unit)));
            if !equals(&expected, &result.ty, true) {
                panic!("Function didn't update to the expected type");
            }
            *value = result.value;
        });
        if let Some(new_type) = new_type {
            self.ty = with_focus_type(&self.ty, finder, new_type);
        }
        self
    }

    pub fn update_to(mut self, finder: &OpticFinder, new_type: &Arc<Type>, mut updater: impl FnMut(Value) -> Value) -> Typed {
        self.check(finder);
        visit(&self.ty.clone(), &mut self.value, finder, &mut |_, value| {
            *value = updater(std::mem::replace(value, Value::Unit));
        });
        self.ty = with_focus_type(&self.ty, finder, new_type);
        self
    }

    pub fn update(mut self, finder: &OpticFinder, mut updater: impl FnMut(Value) -> Value) -> Typed {
        self.check(finder);
        visit(&self.ty.clone(), &mut self.value, finder, &mut |_, value| {
            *value = updater(std::mem::replace(value, Value::Unit));
        });
        self
    }

    pub fn set(mut self, finder: &OpticFinder, new_value: Typed) -> Typed {
        self.check(finder);
        set_value(&self.ty.clone(), &mut self.value, finder, &new_value.value);
        if !equals(&new_value.ty, &finder.ty(), false) {
            self.ty = with_focus_type(&self.ty, finder, &new_value.ty);
        }
        self
    }

    pub fn with_type(self, ty: Arc<Type>) -> Typed {
        Typed { ty, value: self.value }
    }

    pub fn get_remainder(&self) -> Dynamic {
        match self.get_optional(&OpticFinder::Remainder) {
            Some(Value::Dynamic(dynamic)) => dynamic,
            _ => panic!("No remainder in {:?}", self.ty),
        }
    }

    pub fn get_optional_remainder(&self) -> Option<Dynamic> {
        match self.get_optional(&OpticFinder::Remainder) {
            Some(Value::Dynamic(dynamic)) => Some(dynamic),
            _ => None,
        }
    }

    pub fn update_remainder(self, mut updater: impl FnMut(Dynamic) -> Dynamic) -> Typed {
        self.update(&OpticFinder::Remainder, |value| match value {
            Value::Dynamic(dynamic) => Value::Dynamic(updater(dynamic)),
            other => other,
        })
    }

    pub fn set_remainder(self, dynamic: Dynamic) -> Typed {
        self.update(&OpticFinder::Remainder, |_| Value::Dynamic(dynamic.clone()))
    }
}
