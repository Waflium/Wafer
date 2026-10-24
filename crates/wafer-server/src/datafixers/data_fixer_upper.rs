use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Arc, Mutex};

use super::data_fix::{DataFix, EverywhereRule, RuleFunction};
use super::data_fix_utils;
use super::dsl::TypeReference;
use super::dynamic::Dynamic;
use super::schemas::schema::{Schema, SchemaClass};
use super::typed::Typed;
use super::types::{self, RecursiveTypeFamily, Type, equals};
use super::value::{self, Value};

struct Plan {
    steps: Vec<Step>,
    segments: Vec<Segment>,
    new_type: Arc<Type>,
}

struct Step {
    rule: Arc<EverywhereRule>,
    ty: Arc<Type>,
}

enum Segment {
    Single(usize),
    Fused(FusedRun),
}

struct FusedRun {
    ty: Arc<Type>,
    rules: Vec<Arc<EverywhereRule>>,
    clusters: Vec<(Arc<RecursiveTypeFamily>, Vec<Vec<usize>>)>,
}

pub struct DataFixerUpper {
    schemas: BTreeMap<i32, Arc<Schema>>,
    global_list: Vec<Box<dyn DataFix>>,
    fixer_versions: BTreeSet<i32>,
    plans: Mutex<HashMap<(&'static str, i32, i32), Arc<Plan>>>,
}

impl DataFixerUpper {
    pub fn update(&self, ty: TypeReference, input: Dynamic, version: i32, new_version: i32) -> Dynamic {
        if version >= new_version {
            return input;
        }
        let data_type = self.get_type(ty, version);
        let plan = self.plan(ty, version, new_version, &data_type);
        if plan.steps.is_empty() {
            return input;
        }
        let decoded = value::decode(&data_type, input.value.clone());
        let Some((mut value, rest)) = decoded.value else {
            error!("{}", decoded.error.unwrap_or_default());
            return input;
        };
        if !types::equals(&self.get_type(ty, new_version), &plan.new_type, true) {
            error!("Rewritten type doesn't match");
            return input;
        }
        for segment in plan.segments.iter() {
            let result = match segment {
                Segment::Single(index) => apply_everywhere(&plan.steps[*index].ty, &mut value, &plan.steps[*index].rule),
                Segment::Fused(run) => apply_fused(&run.ty, &mut value, run),
            };
            if let Err(message) = result {
                error!("{message}");
                return input;
            }
        }
        let encoded = value::encode(&plan.new_type, &value, rest);
        let message = match (decoded.error, encoded.error) {
            (Some(first), Some(second)) => Some(format!("{first}; {second}")),
            (first, second) => first.or(second),
        };
        if let Some(message) = message {
            error!("{message}");
        }
        match encoded.value {
            Some(result) => Dynamic::of(result),
            None => input,
        }
    }

    fn plan(&self, ty: TypeReference, version: i32, new_version: i32, data_type: &Arc<Type>) -> Arc<Plan> {
        let key = (ty.type_name(), version, new_version);
        if let Some(plan) = self.plans.lock().unwrap().get(&key) {
            return plan.clone();
        }
        let mut steps = Vec::new();
        let mut current = data_type.clone();
        let expanded_version = self.get_lowest_fix_same_version(data_fix_utils::make_key(version));
        for fix in &self.global_list {
            let expanded_fix_version = fix.get_version_key();
            let fix_version = data_fix_utils::get_version(expanded_fix_version);
            if expanded_fix_version > expanded_version && fix_version <= new_version {
                let mut rules = Vec::new();
                fix.get_rule().everywheres(&mut rules);
                for rule in rules {
                    if types::contains(&current, &rule.ty) {
                        let next = if Arc::ptr_eq(&rule.ty, &rule.new_type) { current.clone() } else { types::rewrite_everywhere(&current, &rule.ty, &rule.new_type) };
                        steps.push(Step { rule, ty: current });
                        current = next;
                    }
                }
            }
        }
        let segments = build_segments(&steps);
        let plan = Arc::new(Plan { steps, segments, new_type: current });
        self.plans.lock().unwrap().insert(key, plan.clone());
        plan
    }

    pub fn get_schema(&self, key: i32) -> &Arc<Schema> {
        &self.schemas[&self.get_lowest_schema_same_version(key)]
    }

    fn get_type(&self, ty: TypeReference, version: i32) -> Arc<Type> {
        self.get_schema(data_fix_utils::make_key(version)).get_type_raw(ty)
    }

    fn get_lowest_schema_same_version(&self, version_key: i32) -> i32 {
        lowest_schema_same_version(&self.schemas, version_key)
    }

    fn get_lowest_fix_same_version(&self, version_key: i32) -> i32 {
        let first = *self.fixer_versions.first().expect("a fixer version");
        if version_key < first { first - 1 } else { *self.fixer_versions.range(..=version_key).next_back().expect("a fixer version") }
    }
}

fn lowest_schema_same_version(schemas: &BTreeMap<i32, Arc<Schema>>, version_key: i32) -> i32 {
    let first = *schemas.keys().next().expect("a schema");
    if version_key < first { first } else { *schemas.range(..=version_key).next_back().expect("a schema").0 }
}

pub struct DataFixerBuilder {
    data_version: i32,
    schemas: BTreeMap<i32, Arc<Schema>>,
    global_list: Vec<Box<dyn DataFix>>,
    fixer_versions: BTreeSet<i32>,
}

impl DataFixerBuilder {
    pub fn new(data_version: i32) -> DataFixerBuilder {
        DataFixerBuilder { data_version, schemas: BTreeMap::new(), global_list: Vec::new(), fixer_versions: BTreeSet::new() }
    }

    pub fn add_schema(&mut self, version: i32, class: &'static SchemaClass) -> Arc<Schema> {
        self.add_schema_with_sub_version(version, 0, class)
    }

    pub fn add_schema_with_sub_version(&mut self, version: i32, sub_version: i32, class: &'static SchemaClass) -> Arc<Schema> {
        let key = data_fix_utils::make_key_with_sub_version(version, sub_version);
        let parent = if self.schemas.is_empty() { None } else { Some(self.schemas[&lowest_schema_same_version(&self.schemas, key - 1)].clone()) };
        let schema = Schema::new(key, parent, class);
        self.schemas.insert(schema.version_key, schema.clone());
        schema
    }

    pub fn add_fixer(&mut self, fix: impl DataFix + 'static) {
        let version = data_fix_utils::get_version(fix.get_version_key());
        if version > self.data_version {
            warn!("Ignored fix registered for version: {version} as the DataVersion of the game is: {}", self.data_version);
        } else {
            self.fixer_versions.insert(fix.get_version_key());
            self.global_list.push(Box::new(fix));
        }
    }

    pub fn build(self) -> DataFixerUpper {
        DataFixerUpper { schemas: self.schemas, global_list: self.global_list, fixer_versions: self.fixer_versions, plans: Mutex::new(HashMap::new()) }
    }
}

fn apply_everywhere(ty: &Arc<Type>, value: &mut Value, rule: &EverywhereRule) -> Result<(), String> {
    if let Type::RecursivePoint(family, index) = &**ty {
        return fold(family, *index, value, rule);
    }
    let ty = apply(ty, value, rule)?;
    for_each_child(&ty, value, &mut |child_type, child| apply_everywhere(child_type, child, rule))
}

fn fold(family: &Arc<RecursiveTypeFamily>, index: usize, value: &mut Value, rule: &EverywhereRule) -> Result<(), String> {
    let layer = family.unfold(index).clone();
    for_each_recursion_point(&layer, value, &mut |family, index, child| fold(family, index, child, rule))?;
    layer_everywhere(&layer, value, rule)
}

fn layer_everywhere(ty: &Arc<Type>, value: &mut Value, rule: &EverywhereRule) -> Result<(), String> {
    if matches!(&**ty, Type::RecursivePoint(..)) {
        return Ok(());
    }
    let ty = apply(ty, value, rule)?;
    for_each_child(&ty, value, &mut |child_type, child| layer_everywhere(child_type, child, rule))
}

fn type_matches(ty: &Arc<Type>, rule_type: &Arc<Type>) -> bool {
    if matches!(&**ty, Type::RecursivePoint(..)) {
        return false;
    }
    if equals(rule_type, ty, true) {
        return true;
    }
    match &**ty {
        Type::Named(_, element) | Type::Field(_, element) | Type::Hook(element, _, _) | Type::List(element) => type_matches(element, rule_type),
        Type::Product(first, second) | Type::Sum(first, second) | Type::CompoundList(first, second) => type_matches(first, rule_type) || type_matches(second, rule_type),
        Type::TaggedChoice(choice) => choice.types.iter().any(|(_, element)| type_matches(element, rule_type)),
        _ => false,
    }
}

fn collect_families(ty: &Arc<Type>, families: &mut Vec<Arc<RecursiveTypeFamily>>) {
    match &**ty {
        Type::RecursivePoint(family, _) => {
            if !families.iter().any(|known| Arc::ptr_eq(known, family)) {
                families.push(family.clone());
            }
        }
        Type::Named(_, element) | Type::Field(_, element) | Type::Hook(element, _, _) | Type::List(element) => collect_families(element, families),
        Type::Product(first, second) | Type::Sum(first, second) | Type::CompoundList(first, second) => {
            collect_families(first, families);
            collect_families(second, families);
        }
        Type::TaggedChoice(choice) => choice.types.iter().for_each(|(_, element)| collect_families(element, families)),
        _ => {}
    }
}

fn preserves_type(step: &Step) -> bool {
    Arc::ptr_eq(&step.rule.ty, &step.rule.new_type)
}

fn is_fold_only(step: &Step) -> bool {
    !type_matches(&step.ty, &step.rule.ty)
}

fn build_segments(steps: &[Step]) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut index = 0;
    while index < steps.len() {
        let mut end = index + 1;
        if is_fold_only(&steps[index]) {
            while end < steps.len() && preserves_type(&steps[end - 1]) && Arc::ptr_eq(&steps[end].ty, &steps[index].ty) && is_fold_only(&steps[end]) {
                end += 1;
            }
        }
        if end - index > 1 {
            let rules: Vec<Arc<EverywhereRule>> = steps[index..end].iter().map(|step| step.rule.clone()).collect();
            let mut families = Vec::new();
            collect_families(&steps[index].ty, &mut families);
            let clusters = families.into_iter().map(|family| (family.clone(), fuse_clusters(&family, &rules))).collect();
            segments.push(Segment::Fused(FusedRun { ty: steps[index].ty.clone(), rules, clusters }));
        } else {
            segments.push(Segment::Single(index));
        }
        index = end;
    }
    segments
}

fn fuse_clusters(family: &Arc<RecursiveTypeFamily>, rules: &[Arc<EverywhereRule>]) -> Vec<Vec<usize>> {
    let mut clusters: Vec<Vec<usize>> = Vec::new();
    let mut accumulated: Vec<bool> = Vec::new();
    for (index, rule) in rules.iter().enumerate() {
        let set: Vec<bool> = (0..family.size()).map(|member| type_matches(family.unfold(member), &rule.ty)).collect();
        if !set.contains(&true) {
            continue;
        }
        let fusable = !clusters.is_empty() && {
            let same = accumulated == set && set.iter().filter(|matched| **matched).count() == 1;
            let different = !accumulated.iter().zip(&set).any(|(a, b)| *a && *b);
            same || different
        };
        if fusable {
            clusters.last_mut().unwrap().push(index);
            accumulated = accumulated.iter().zip(&set).map(|(a, b)| *a || *b).collect();
        } else {
            clusters.push(vec![index]);
            accumulated = set;
        }
    }
    clusters
}

fn apply_fused(ty: &Arc<Type>, value: &mut Value, run: &FusedRun) -> Result<(), String> {
    if let Type::RecursivePoint(family, index) = &**ty {
        return fold_fused(family, *index, value, run);
    }
    for_each_child(ty, value, &mut |child_type, child| apply_fused(child_type, child, run))
}

fn fold_fused(family: &Arc<RecursiveTypeFamily>, index: usize, value: &mut Value, run: &FusedRun) -> Result<(), String> {
    match run.clusters.iter().find(|(known, _)| Arc::ptr_eq(known, family)) {
        Some((_, clusters)) => {
            for cluster in clusters {
                fold_cluster(family, index, value, &run.rules, cluster)?;
            }
            Ok(())
        }
        None => {
            for rule in &run.rules {
                fold(family, index, value, rule)?;
            }
            Ok(())
        }
    }
}

fn fold_cluster(family: &Arc<RecursiveTypeFamily>, index: usize, value: &mut Value, rules: &[Arc<EverywhereRule>], cluster: &[usize]) -> Result<(), String> {
    let layer = family.unfold(index).clone();
    for_each_recursion_point(&layer, value, &mut |family, index, child| fold_cluster(family, index, child, rules, cluster))?;
    for rule in cluster {
        layer_everywhere(&layer, value, &rules[*rule])?;
    }
    Ok(())
}

fn for_each_recursion_point(
    ty: &Arc<Type>,
    value: &mut Value,
    f: &mut dyn FnMut(&Arc<RecursiveTypeFamily>, usize, &mut Value) -> Result<(), String>,
) -> Result<(), String> {
    if let Type::RecursivePoint(family, index) = &**ty {
        return f(family, *index, value);
    }
    for_each_child(ty, value, &mut |child_type, child| for_each_recursion_point(child_type, child, f))
}

fn for_each_child(ty: &Arc<Type>, value: &mut Value, f: &mut dyn FnMut(&Arc<Type>, &mut Value) -> Result<(), String>) -> Result<(), String> {
    match (&**ty, value) {
        (Type::Named(_, element) | Type::Field(_, element) | Type::Hook(element, _, _), value) => f(element, value),
        (Type::Product(first, second), Value::Pair(first_value, second_value)) => {
            f(first, first_value)?;
            f(second, second_value)
        }
        (Type::Sum(first, _), Value::Left(inner)) => f(first, inner),
        (Type::Sum(_, second), Value::Right(inner)) => f(second, inner),
        (Type::List(element), Value::List(values)) => values.iter_mut().try_for_each(|value| f(element, value)),
        (Type::CompoundList(key, element), Value::CompoundList(entries)) => entries.iter_mut().try_for_each(|(key_value, entry_value)| {
            f(key, key_value)?;
            f(element, entry_value)
        }),
        (Type::TaggedChoice(choice), Value::Choice(key, inner)) => match choice.get(key) {
            Some(element) => f(&element.clone(), inner),
            None => Ok(()),
        },
        _ => Ok(()),
    }
}

fn apply(ty: &Arc<Type>, value: &mut Value, rule: &EverywhereRule) -> Result<Arc<Type>, String> {
    if !equals(&rule.ty, ty, true) {
        return Ok(ty.clone());
    }
    let input = std::mem::replace(value, Value::Unit);
    *value = match &rule.function {
        RuleFunction::Value(function) => function(input),
        RuleFunction::Typed(function) => {
            let result = function(Typed::new(rule.ty.clone(), input));
            if !equals(&rule.new_type, &result.ty, true) {
                return Err(format!("Dynamic type check failed: {:?} not equal to {:?}", rule.new_type, result.ty));
            }
            result.value
        }
        RuleFunction::WriteFixAndRead(fix) => {
            let patched = types::patch_sub_type(&rule.ty, &rule.ty, &rule.new_type);
            let written = value::encode_start(&patched, &input);
            if let Some(message) = &written.error {
                error!("{message}");
            }
            let Some(written) = written.value else { return Err(format!("Could not write the object in {}", rule.name)) };
            let fixed = fix(Dynamic::of(written));
            let read = value::decode(&rule.new_type, fixed.value);
            if let Some(message) = &read.error {
                error!("{message}");
            }
            match read.value {
                Some((value, _)) => value,
                None => return Err(format!("Could not read the new object in {}", rule.name)),
            }
        }
    };
    Ok(rule.new_type.clone())
}
