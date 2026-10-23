pub mod choice_order;

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex, OnceLock};

use super::dsl::{ConstType, HookFunction, TypeTemplate};
use super::schemas::schema::Schema;

pub struct RecursiveTypeFamily {
    pub name: String,
    members: Vec<OnceLock<Arc<Type>>>,
}

impl RecursiveTypeFamily {
    pub fn new(name: String, size: usize) -> Arc<RecursiveTypeFamily> {
        Arc::new(RecursiveTypeFamily { name, members: (0..size).map(|_| OnceLock::new()).collect() })
    }

    pub fn set_member(&self, index: usize, ty: Arc<Type>) {
        let _ = self.members[index].set(ty);
    }

    pub fn unfold(&self, index: usize) -> &Arc<Type> {
        self.members[index].get().expect("an instantiated recursive type")
    }

    pub fn size(&self) -> usize {
        self.members.len()
    }
}

pub struct TaggedChoiceType {
    pub name: Arc<str>,
    pub key_type: ConstType,
    pub types: Vec<(String, Arc<Type>)>,
}

impl TaggedChoiceType {
    pub fn get(&self, key: &str) -> Option<&Arc<Type>> {
        self.types.iter().find(|(k, _)| k == key).map(|(_, ty)| ty)
    }
}

pub enum Type {
    Const(ConstType),
    Remainder,
    EmptyPart,
    Named(Arc<str>, Arc<Type>),
    Field(Arc<str>, Arc<Type>),
    Product(Arc<Type>, Arc<Type>),
    Sum(Arc<Type>, Arc<Type>),
    List(Arc<Type>),
    CompoundList(Arc<Type>, Arc<Type>),
    TaggedChoice(Arc<TaggedChoiceType>),
    Hook(Arc<Type>, HookFunction, HookFunction),
    RecursivePoint(Arc<RecursiveTypeFamily>, usize),
}

impl std::fmt::Debug for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::Const(ty) => write!(f, "{ty:?}"),
            Type::Remainder => write!(f, "EmptyPartPassthrough"),
            Type::EmptyPart => write!(f, "EmptyPart"),
            Type::Named(name, element) => write!(f, "NamedType[\"{name}\", {element:?}]"),
            Type::Field(name, element) => write!(f, "Tag[\"{name}\", {element:?}]"),
            Type::Product(first, second) => write!(f, "({first:?}, {second:?})"),
            Type::Sum(first, second) => write!(f, "({first:?} | {second:?})"),
            Type::List(element) => write!(f, "List[{element:?}]"),
            Type::CompoundList(key, element) => write!(f, "CompoundList[{key:?} -> {element:?}]"),
            Type::TaggedChoice(choice) => write!(f, "TaggedChoiceType[{}, {} choices]", choice.name, choice.types.len()),
            Type::Hook(element, _, _) => write!(f, "HookType[{element:?}]"),
            Type::RecursivePoint(family, index) => write!(f, "MuType[{}_{index}]", family.name),
        }
    }
}

impl Type {
    pub fn choices(&self) -> &TaggedChoiceType {
        match self {
            Type::TaggedChoice(choice) => choice,
            other => panic!("Not a choice type: {other:?}"),
        }
    }

    pub fn unfold(self: &Arc<Type>) -> Arc<Type> {
        match &**self {
            Type::RecursivePoint(family, index) => family.unfold(*index).clone(),
            _ => self.clone(),
        }
    }

    pub fn find_choice_type(self: &Arc<Type>) -> Option<Arc<Type>> {
        match &**self {
            Type::TaggedChoice(choice) if &*choice.name == "id" => Some(self.clone()),
            Type::Named(_, element) | Type::Hook(element, _, _) => element.find_choice_type(),
            Type::Product(first, second) | Type::Sum(first, second) => first.find_choice_type().or_else(|| second.find_choice_type()),
            Type::RecursivePoint(..) => self.unfold().find_choice_type(),
            _ => None,
        }
    }

    pub fn find_field_type(self: &Arc<Type>, name: &str) -> Option<Arc<Type>> {
        match &**self {
            Type::Field(field_name, element) => (**field_name == *name).then(|| element.clone()),
            Type::Named(_, element) | Type::Hook(element, _, _) => element.find_field_type(name),
            Type::Product(first, second) | Type::Sum(first, second) => first.find_field_type(name).or_else(|| second.find_field_type(name)),
            Type::TaggedChoice(choice) => choice.types.iter().find_map(|(_, ty)| ty.find_field_type(name)),
            Type::RecursivePoint(..) => self.unfold().find_field_type(name),
            _ => None,
        }
    }
}

pub fn const_type(ty: ConstType) -> Arc<Type> {
    Arc::new(Type::Const(ty))
}

pub fn remainder_type() -> Arc<Type> {
    Arc::new(Type::Remainder)
}

pub fn empty_part_type() -> Arc<Type> {
    Arc::new(Type::EmptyPart)
}

pub fn named(name: &str, element: Arc<Type>) -> Arc<Type> {
    Arc::new(Type::Named(name.into(), element))
}

pub fn field(name: &str, element: Arc<Type>) -> Arc<Type> {
    Arc::new(Type::Field(name.into(), element))
}

pub fn list(element: Arc<Type>) -> Arc<Type> {
    Arc::new(Type::List(element))
}

pub fn compound_list(key: Arc<Type>, element: Arc<Type>) -> Arc<Type> {
    Arc::new(Type::CompoundList(key, element))
}

pub fn or(first: Arc<Type>, second: Arc<Type>) -> Arc<Type> {
    Arc::new(Type::Sum(first, second))
}

pub fn optional(element: Arc<Type>) -> Arc<Type> {
    or(element, empty_part_type())
}

pub fn and(first: Arc<Type>, second: Arc<Type>) -> Arc<Type> {
    Arc::new(Type::Product(first, second))
}

pub fn and_all(types: Vec<Arc<Type>>) -> Arc<Type> {
    let mut types = types.into_iter().rev();
    let last = types.next().expect("Must have at least one type");
    types.fold(last, |result, ty| and(ty, result))
}

type FamilyPair = (usize, usize);

type FamilyEqualityCache = HashMap<FamilyPair, (Arc<RecursiveTypeFamily>, Arc<RecursiveTypeFamily>, bool)>;

static FAMILY_EQUALITY: LazyLock<Mutex<FamilyEqualityCache>> = LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn equals(a: &Arc<Type>, b: &Arc<Type>, ignore_recursion_points: bool) -> bool {
    equals_assuming(a, b, ignore_recursion_points, &mut Vec::new())
}

fn equals_assuming(a: &Arc<Type>, b: &Arc<Type>, ignore_recursion_points: bool, assumed: &mut Vec<FamilyPair>) -> bool {
    if Arc::ptr_eq(a, b) {
        return true;
    }
    match (&**a, &**b) {
        (Type::Const(x), Type::Const(y)) => x == y,
        (Type::Remainder, Type::Remainder) | (Type::EmptyPart, Type::EmptyPart) => true,
        (Type::Named(n1, e1), Type::Named(n2, e2)) | (Type::Field(n1, e1), Type::Field(n2, e2)) => n1 == n2 && equals_assuming(e1, e2, ignore_recursion_points, assumed),
        (Type::Product(a1, b1), Type::Product(a2, b2)) | (Type::Sum(a1, b1), Type::Sum(a2, b2)) | (Type::CompoundList(a1, b1), Type::CompoundList(a2, b2)) => {
            equals_assuming(a1, a2, ignore_recursion_points, assumed) && equals_assuming(b1, b2, ignore_recursion_points, assumed)
        }
        (Type::List(e1), Type::List(e2)) => equals_assuming(e1, e2, ignore_recursion_points, assumed),
        (Type::TaggedChoice(c1), Type::TaggedChoice(c2)) => {
            Arc::ptr_eq(c1, c2)
                || (c1.name == c2.name
                    && c1.key_type == c2.key_type
                    && c1.types.len() == c2.types.len()
                    && c1.types.iter().all(|(key, ty)| c2.get(key).is_some_and(|other| equals_assuming(ty, other, ignore_recursion_points, assumed))))
        }
        (Type::Hook(e1, pre1, post1), Type::Hook(e2, pre2, post2)) => {
            std::ptr::fn_addr_eq(*pre1, *pre2) && std::ptr::fn_addr_eq(*post1, *post2) && equals_assuming(e1, e2, ignore_recursion_points, assumed)
        }
        (Type::RecursivePoint(f1, i1), Type::RecursivePoint(f2, i2)) => i1 == i2 && (ignore_recursion_points || families_equal(f1, f2, assumed)),
        _ => false,
    }
}

fn families_equal(f1: &Arc<RecursiveTypeFamily>, f2: &Arc<RecursiveTypeFamily>, assumed: &mut Vec<FamilyPair>) -> bool {
    if Arc::ptr_eq(f1, f2) {
        return true;
    }
    let key = (Arc::as_ptr(f1) as usize, Arc::as_ptr(f2) as usize);
    if assumed.contains(&key) {
        return true;
    }
    let outermost = assumed.is_empty();
    if outermost && let Some((_, _, equal)) = FAMILY_EQUALITY.lock().unwrap().get(&key) {
        return *equal;
    }
    let equal = f1.size() == f2.size() && {
        assumed.push(key);
        let equal = (0..f1.size()).all(|index| equals_assuming(f1.unfold(index), f2.unfold(index), false, assumed));
        assumed.pop();
        equal
    };
    if outermost {
        FAMILY_EQUALITY.lock().unwrap().insert(key, (f1.clone(), f2.clone(), equal));
    }
    equal
}

#[cfg(test)]
pub fn canonical(ty: &Arc<Type>) -> String {
    match &**ty {
        Type::Const(kind) => format!("{kind:?}"),
        Type::Remainder => "Rem".to_string(),
        Type::EmptyPart => "E".to_string(),
        Type::Named(name, element) => format!("N[{name}]({})", canonical(element)),
        Type::Field(name, element) => format!("F[{name}]({})", canonical(element)),
        Type::Product(first, second) => format!("({},{})", canonical(first), canonical(second)),
        Type::Sum(first, second) => format!("({}|{})", canonical(first), canonical(second)),
        Type::List(element) => format!("L[{}]", canonical(element)),
        Type::CompoundList(key, element) => format!("CL[{}->{}]", canonical(key), canonical(element)),
        Type::TaggedChoice(choice) => {
            let mut entries: Vec<(&String, String)> = choice.types.iter().map(|(key, element)| (key, canonical(element))).collect();
            entries.sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
            let body: Vec<String> = entries.into_iter().map(|(key, element)| format!("{key}:{element}")).collect();
            format!("TC[{},{:?}]{{{}}}", choice.name, choice.key_type, body.join(";"))
        }
        Type::Hook(element, _, _) => format!("H({})", canonical(element)),
        Type::RecursivePoint(family, index) => match &**family.unfold(*index) {
            Type::Named(name, _) => format!("R[{name}]"),
            _ => format!("R[{index}]"),
        },
    }
}

pub fn instantiate(template: &TypeTemplate, family: &Arc<RecursiveTypeFamily>, schema: &Schema) -> Arc<Type> {
    Arc::new(match template {
        TypeTemplate::Const(ty) => Type::Const(*ty),
        TypeTemplate::Remainder => Type::Remainder,
        TypeTemplate::EmptyPart => Type::EmptyPart,
        TypeTemplate::Named(name, element) => Type::Named(name.clone(), instantiate(element, family, schema)),
        TypeTemplate::Field(name, element) => Type::Field(name.clone(), instantiate(element, family, schema)),
        TypeTemplate::Product(first, second) => Type::Product(instantiate(first, family, schema), instantiate(second, family, schema)),
        TypeTemplate::Sum(first, second) => Type::Sum(instantiate(first, family, schema), instantiate(second, family, schema)),
        TypeTemplate::List(element) => Type::List(instantiate(element, family, schema)),
        TypeTemplate::CompoundList(key, element) => Type::CompoundList(instantiate(key, family, schema), instantiate(element, family, schema)),
        TypeTemplate::TaggedChoice(name, key_type, templates) => Type::TaggedChoice(Arc::new(TaggedChoiceType {
            name: name.clone(),
            key_type: *key_type,
            types: templates.iter().map(|(key, supplier)| (key.clone(), instantiate(&supplier(schema), family, schema))).collect(),
        })),
        TypeTemplate::Hook(element, pre_read, post_write) => Type::Hook(instantiate(element, family, schema), *pre_read, *post_write),
        TypeTemplate::Id(index) => Type::RecursivePoint(family.clone(), *index),
    })
}

pub fn rewrite_everywhere(ty: &Arc<Type>, find: &Arc<Type>, replace: &Arc<Type>) -> Arc<Type> {
    let mut families = Vec::new();
    patch(ty, find, replace, &mut families)
}

pub fn patch_sub_type(ty: &Arc<Type>, find: &Arc<Type>, replace: &Arc<Type>) -> Arc<Type> {
    let mut families = Vec::new();
    patch_children(ty, find, replace, &mut families)
}

type PatchedFamilies = Vec<(usize, Arc<RecursiveTypeFamily>)>;

fn patch(ty: &Arc<Type>, find: &Arc<Type>, replace: &Arc<Type>, families: &mut PatchedFamilies) -> Arc<Type> {
    if equals(ty, find, true) {
        return replace.clone();
    }
    patch_children(ty, find, replace, families)
}

fn patch_children(ty: &Arc<Type>, find: &Arc<Type>, replace: &Arc<Type>, families: &mut PatchedFamilies) -> Arc<Type> {
    let mut sub = |element: &Arc<Type>| patch(element, find, replace, families);
    Arc::new(match &**ty {
        Type::Const(_) | Type::Remainder | Type::EmptyPart => return ty.clone(),
        Type::Named(name, element) => Type::Named(name.clone(), sub(element)),
        Type::Field(name, element) => Type::Field(name.clone(), sub(element)),
        Type::Product(first, second) => {
            let first = sub(first);
            Type::Product(first, sub(second))
        }
        Type::Sum(first, second) => {
            let first = sub(first);
            Type::Sum(first, sub(second))
        }
        Type::List(element) => Type::List(sub(element)),
        Type::CompoundList(key, element) => {
            let key = sub(key);
            Type::CompoundList(key, sub(element))
        }
        Type::TaggedChoice(choice) => Type::TaggedChoice(Arc::new(TaggedChoiceType {
            name: choice.name.clone(),
            key_type: choice.key_type,
            types: choice.types.iter().map(|(key, element)| (key.clone(), patch(element, find, replace, families))).collect(),
        })),
        Type::Hook(element, pre_read, post_write) => Type::Hook(sub(element), *pre_read, *post_write),
        Type::RecursivePoint(family, index) => Type::RecursivePoint(patch_family(family, find, replace, families), *index),
    })
}

fn patch_family(family: &Arc<RecursiveTypeFamily>, find: &Arc<Type>, replace: &Arc<Type>, families: &mut PatchedFamilies) -> Arc<RecursiveTypeFamily> {
    let key = Arc::as_ptr(family) as usize;
    if let Some((_, patched)) = families.iter().find(|(k, _)| *k == key) {
        return patched.clone();
    }
    let patched = RecursiveTypeFamily::new(family.name.clone(), family.size());
    families.push((key, patched.clone()));
    for index in 0..family.size() {
        patched.set_member(index, rebind(&patch(family.unfold(index), find, replace, families), &patched));
    }
    patched
}

fn rebind(ty: &Arc<Type>, family: &Arc<RecursiveTypeFamily>) -> Arc<Type> {
    let sub = |element: &Arc<Type>| rebind(element, family);
    Arc::new(match &**ty {
        Type::Const(_) | Type::Remainder | Type::EmptyPart => return ty.clone(),
        Type::RecursivePoint(point_family, _) if Arc::ptr_eq(point_family, family) => return ty.clone(),
        Type::RecursivePoint(_, index) => Type::RecursivePoint(family.clone(), *index),
        Type::Named(name, element) => Type::Named(name.clone(), sub(element)),
        Type::Field(name, element) => Type::Field(name.clone(), sub(element)),
        Type::Product(first, second) => Type::Product(sub(first), sub(second)),
        Type::Sum(first, second) => Type::Sum(sub(first), sub(second)),
        Type::List(element) => Type::List(sub(element)),
        Type::CompoundList(key, element) => Type::CompoundList(sub(key), sub(element)),
        Type::TaggedChoice(choice) => Type::TaggedChoice(Arc::new(TaggedChoiceType {
            name: choice.name.clone(),
            key_type: choice.key_type,
            types: choice.types.iter().map(|(key, element)| (key.clone(), sub(element))).collect(),
        })),
        Type::Hook(element, pre_read, post_write) => Type::Hook(sub(element), *pre_read, *post_write),
    })
}

pub fn contains(ty: &Arc<Type>, target: &Arc<Type>) -> bool {
    let mut visited = Vec::new();
    contains_in(ty, target, &mut visited)
}

fn contains_in(ty: &Arc<Type>, target: &Arc<Type>, visited: &mut Vec<(usize, usize)>) -> bool {
    if equals(ty, target, true) {
        return true;
    }
    match &**ty {
        Type::Named(_, element) | Type::Field(_, element) | Type::List(element) | Type::Hook(element, _, _) => contains_in(element, target, visited),
        Type::Product(first, second) | Type::Sum(first, second) | Type::CompoundList(first, second) => {
            contains_in(first, target, visited) || contains_in(second, target, visited)
        }
        Type::TaggedChoice(choice) => choice.types.iter().any(|(_, element)| contains_in(element, target, visited)),
        Type::RecursivePoint(family, index) => {
            let key = (Arc::as_ptr(family) as usize, *index);
            if visited.contains(&key) {
                return false;
            }
            visited.push(key);
            contains_in(family.unfold(*index), target, visited)
        }
        _ => false,
    }
}
