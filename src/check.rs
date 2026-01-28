use regex::Regex;

use crate::{
    CompilerError, ast::{
        Action, ActionG, BoolExpr, Condition, Expr, ExprList, ExprType, Fallback, Field, FieldList, FieldValue, Id, IdList, Rule, RuleBlock, Rules, Struct, TagG, TitleId, Type, TypeList
    }, collect::{CollectedCode, Identifiers}
};
use std::{
    collections::{HashMap, HashSet},
    ops::{Deref, DerefMut},
};

#[derive(Debug)]
pub(crate) enum CycleCheckStackValue {
    Enter((String, Option<String>)),
    Exit(String),
}

#[derive(Debug)]
pub(crate) struct NameMap(HashMap<String, HashSet<String>>);
impl Deref for NameMap {
    type Target = HashMap<String, HashSet<String>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for NameMap {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl<'a> IntoIterator for &'a NameMap {
    type IntoIter = <&'a HashMap<String, HashSet<String>> as IntoIterator>::IntoIter;
    type Item = (&'a String, &'a HashSet<String>);
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}
impl NameMap {
    fn new() -> Self {
        NameMap(HashMap::new())
    }

    fn group_only(&self, is_group: impl Fn(&String) -> bool) -> Self {
        let mut group_only: Self = NameMap::new();
        for (group, members) in &self.0 {
            group_only.insert(group.clone(), HashSet::new());
            for m in members {
                if is_group(m) {
                    group_only.get_mut(group).map(|e| e.insert(m.clone()));
                }
            }
        }
        group_only
    }

    fn check_cycles(&self) -> Option<Vec<String>> {
        let names: Vec<String> = self.keys().into_iter().map(|s| s.clone()).collect();

        let mut visited: HashSet<String> = HashSet::new();

        for name in names {
            let mut stack: Vec<CycleCheckStackValue> = Vec::new();
            let mut path: HashSet<String> = HashSet::new();
            let mut parent_map: HashMap<String, String> = HashMap::new();

            stack.push(CycleCheckStackValue::Enter((name.clone(), None)));

            while let Some(val) = stack.pop() {
                match val {
                    CycleCheckStackValue::Enter((current, parent)) => {
                        if path.contains(&current) {
                            let mut cycle = vec![current.clone()];
                            let Some(mut cur) = parent else {
                                return Some(vec![current]);
                            };
                            while cur != current {
                                cycle.push(cur.clone());
                                cur = parent_map
                                    .get(&cur)
                                    .expect("Parent should be Some during backtracking cycle.")
                                    .clone();
                            }
                            cycle.push(current);
                            cycle.reverse();
                            return Some(cycle);
                        }

                        if visited.contains(&current) {
                            continue;
                        }

                        visited.insert(current.clone());
                        path.insert(current.clone());
                        if let Some(par) = parent {
                            parent_map.insert(current.clone(), par);
                        }

                        stack.push(CycleCheckStackValue::Exit(current.clone()));

                        if let Some(nodes) = self.get(&current) {
                            for node in nodes {
                                stack.push(CycleCheckStackValue::Enter((
                                    node.clone(),
                                    Some(current.clone()),
                                )));
                            }
                        }
                    }
                    CycleCheckStackValue::Exit(v) => {
                        path.remove(&v.clone());
                    }
                }
            }
        }

        None
    }

    fn flatten_graph(&self) -> NameMap {
        let mut flat: NameMap = NameMap::new();

        for node in self.keys() {
            let mut stack: Vec<String> = Vec::new();
            let mut this_flat: HashSet<String> = HashSet::new();

            stack.push(node.clone());

            while let Some(current) = stack.pop() {
                if let Some(nebs) = self.get(&current) {
                    for neb in nebs {
                        if self.contains_key(neb) {
                            stack.push(neb.clone());
                        } else {
                            this_flat.insert(neb.clone());
                        }
                    }
                }
            }
            flat.insert(node.clone(), this_flat);
        }

        flat
    }

    fn _inner_post_order_acc(
        &self,
        node: &String,
        cache: &NameMap,
        init_fn: &impl Fn(&String) -> HashSet<String>,
    ) -> HashSet<String> {
        cache.get(node).cloned().unwrap_or_else(|| {
            let mut acc = init_fn(node);
            if let Some(nebs) = self.get(node) {
                for neb in nebs {
                    acc.extend(self._inner_post_order_acc(neb, cache, init_fn));
                }
            }
            acc
        })
    }

    fn post_order_accumulate(&self, init_fn: impl Fn(&String) -> HashSet<String>) -> NameMap {
        let mut cache: NameMap = NameMap::new();
        for node in self.keys() {
            let val = self._inner_post_order_acc(node, &cache, &init_fn);
            cache.insert(node.clone(), val);
        }
        cache
    }
}

#[derive(Debug)]
pub(crate) struct GenerationContext {
    pub(crate) tag_mappings: NameMap,      // name -> members
    pub(crate) action_mappings: NameMap,   // (name -> members)
    pub(crate) struct_intrinsics: NameMap, // name -> intrinsic tags (including inherited)
    pub(crate) action_fallback: HashMap<String, Fallback>, // name -> Fallback
    pub(crate) action_return: HashMap<String, Type>, // name -> return type
    pub(crate) ids: Identifiers,
}

impl GenerationContext {
    fn new() -> Self {
        Self {
            tag_mappings: NameMap::new(),
            action_mappings: NameMap::new(),
            struct_intrinsics: NameMap::new(),
            action_fallback: HashMap::new(),
            action_return: HashMap::new(),
            ids: Identifiers::default(),
        }
    }
}

struct CompilerContext {
    types: HashSet<Type>,
    field_types: HashMap<(Type, String), Type>, // (struct, field) -> type
    ids: Identifiers,
    action_signatures: HashMap<String, Vec<Type>>, // action(group) name -> argument signature
}

impl CompilerContext {
    fn new(ids: Identifiers) -> Self {
        let mut types: HashSet<Type> = HashSet::new();
        types.insert(Type::Bool);
        types.insert(Type::Int);
        types.insert(Type::String);

        Self {
            types: types,
            field_types: HashMap::new(),
            ids: ids,
            action_signatures: HashMap::new(),
        }
    }
}

// Validation Requirements (v1, v2 and v12 are handled during collection phase):
//   v1. Tags,TagGroups,Structs,Actions,ActionGroups identifiers are disjoint
//   v2. No re-definitions
//   v3. Members of TagGroups are defined as Tag or TagGroup
//   v4. TagGroup definitions are not cyclic
//   v5. Struct tags exist
//   v6. Struct field types exist
//      v6.1. Struct field names are a set
//   v7. Struct definitions are not cyclic
//   v8. Action arg types exist
//      v8.1. Action return type exists and is struct
//      v8.2. Action return type is struct
//   v9. Members of ActionGroups are defined as Action or ActionGroup
//  v10. ActionGroup Definitions are not cyclic
//  v11. Members of ActionGroups have the same arg signature
//  v12. RuleBlocks identifiers are a subset of Action+ActionGroups
//  v13. RuleBlocks arg types match the arg signture of the named Action(Group)
//  v14. RuleBlocks Rules are valid
//      v15. Apply tags should exist
//      => Condition is valid (refer to validate_condition for requirements)
//
// Side Effects (building CompilerContext, GenerationContext to save work)
// s1. tag_mappings: map ids of Tags and TagGroups to expanded list of Tags
// s2. field_types: map (struct, field) to a type
// s3. types: set of valid types
// s4. action_groups: map ids of Actions and ActionGroups to expanded list of Actions and Type list of args
// s5. action_signatures: map ids of Actions and ActionGroups to Type list of args
// s6. action_fallback: map ids of Actions to Fallback
// s7. action_return: map ids of Actions to return type
pub fn validate_code(
    code: &CollectedCode,
    ids: Identifiers,
) -> Result<GenerationContext, CompilerError> {
    let mut ctx = CompilerContext::new(ids);
    let mut gen_ctx = GenerationContext::new();

    // Tags --------------------------------------------------------------.
    // already done.

    // TagGroups --------------------------------------------------------------
    let mut group_to_all_map: NameMap = NameMap::new();
    for tagg in &code.tag_groups {
        let (name, tags) = validate_tag_group(tagg, &ctx)?;
        group_to_all_map.insert(name, tags);
    }

    // v4.
    if let Some(cycle) = group_to_all_map
        .group_only(|n| (&ctx).ids.tag_group_names.contains(n))
        .check_cycles()
    {
        return Err(CompilerError::Cycle(format!(
            "Cycle in tag groups {cycle:?}."
        )));
    }

    // s1.
    // Flatten groups
    gen_ctx.tag_mappings = group_to_all_map.flatten_graph();
    // Add mappings for normal tags to themselves.
    for tag in &ctx.ids.tag_names {
        let mut tmp: HashSet<String> = HashSet::new();
        tmp.insert(tag.clone());
        gen_ctx.tag_mappings.insert(tag.clone(), tmp);
    }

    // Structs --------------------------------------------------------------
    let mut type_graph: NameMap = NameMap::new();
    let mut tag_graph: NameMap = NameMap::new();
    for s in &code.structs {
        let (name, types, fields, tags) = validate_struct(s, &ctx)?;
        let struct_as_type = Type::Struct(TitleId(name.clone()));

        type_graph.insert(name.clone(), types);
        tag_graph.insert(name.clone(), tags);

        // s2.
        for (fname, ftype) in fields {
            ctx.field_types
                .insert((struct_as_type.clone(), fname), ftype);
        }

        // s3.
        ctx.types.insert(struct_as_type);
    }

    // v7.
    let group_only = type_graph.group_only(|n| (&ctx).ids.struct_exists(n));
    if let Some(cycle) = group_only.check_cycles() {
        return Err(CompilerError::Cycle(format!(
            "Cycle in struct field type definitions {cycle:?}."
        )));
    }

    gen_ctx.struct_intrinsics =
        group_only.post_order_accumulate(|n| tag_graph.get(n).expect("should work.").clone());

    // Actions --------------------------------------------------------------
    let mut action_arg_types: HashMap<String, Vec<Type>> = HashMap::new();
    for action in &code.actions {
        let (name, types, ret, fallback) = validate_action(action, &ctx)?;
        action_arg_types.insert(name.clone(), types.clone());
        // s4. actions only
        let mut tmp: HashSet<String> = HashSet::new();
        tmp.insert(name.clone());
        gen_ctx.action_mappings.insert(name.clone(), tmp);
        // s5. actions only
        ctx.action_signatures.insert(name.clone(), types.clone());
        // s6
        gen_ctx.action_fallback.insert(name.clone(), fallback);
        // s7
        gen_ctx.action_return.insert(name, ret);
    }

    // ActionGroups --------------------------------------------------------------
    let mut group_to_all_map: NameMap = NameMap::new();
    for action in &code.action_groups {
        let (name, actions) = validate_action_group(action, &ctx)?;
        group_to_all_map.insert(name, actions);
    }

    // v10.
    if let Some(cycle) = group_to_all_map
        .group_only(|n| (&ctx).ids.action_group_names.contains(n))
        .check_cycles()
    {
        return Err(CompilerError::Cycle(format!(
            "Cycle in action groups {cycle:?}."
        )));
    }

    // Flatten groups
    let flat_map = group_to_all_map.flatten_graph();

    // v11.
    for (group_name, actions) in &flat_map {
        let mut group_signature: Option<Vec<Type>> = None;

        for action in actions {
            // no actions here are groups because this is post flatten.
            let this_action_sig = action_arg_types.get(action).expect(
                "No action {action} in action_arg_types. All actions should be defined by now.",
            );

            if let Some(group_sig) = group_signature
                && this_action_sig != &group_sig
            {
                return Err(CompilerError::TypeError(format!("ActionGroup {group_name} has argument types {group_sig:?} but member {action} has argument types {this_action_sig:?}. ActionGroup members must share the same argument signature.")));
            } else {
                group_signature = Some(this_action_sig.clone());
            }
        }
        // s4. action_group part (action part done earlier)
        let sig = group_signature.unwrap_or(Vec::new());
        gen_ctx
            .action_mappings
            .insert(group_name.clone(), actions.clone());
        // s5. action_group part (action part done earlier)
        ctx.action_signatures.insert(group_name.clone(), sig);
    }

    // RuleBlocks --------------------------------------------------------------
    for rule_block in &code.rule_blocks {
        validate_rule_block(rule_block, &ctx)?;
    }
    // --------------------------------------------------------------------------

    gen_ctx.ids = ctx.ids;
    Ok(gen_ctx)
}

fn validate_tag_group(
    tagg: &&TagG,
    ctx: &CompilerContext,
) -> Result<(String, HashSet<String>), CompilerError> {
    let &TagG(Id(name), IdList(items)) = tagg;
    let mut tags: HashSet<String> = HashSet::new();

    // v3.
    for Id(item) in items {
        if !ctx.ids.tag_names.contains(item) && !ctx.ids.tag_group_names.contains(item) {
            return Err(CompilerError::Undefined(format!(
                "TagGroup {name} member {item} undefined."
            )));
        }
        tags.insert(item.clone());
    }

    Ok((name.clone(), tags))
}

fn validate_struct(
    s: &&Struct,
    ctx: &CompilerContext,
) -> Result<
    (
        String,
        HashSet<String>,
        HashMap<String, Type>,
        HashSet<String>,
    ),
    CompilerError,
> {
    let &Struct(TitleId(name), tags, fields) = s;

    let mut tags_set: HashSet<String> = HashSet::new();

    // v5.
    let IdList(tags) = tags;
    for Id(tag) in tags {
        if !ctx.ids.tag_exists(tag) {
            return Err(CompilerError::Undefined(format!(
                "Struct {name} tag {tag} undefined."
            )));
        }
        tags_set.insert(tag.clone());
    }

    let mut field_types: HashSet<String> = HashSet::new();
    let mut fields_map: HashMap<String, Type> = HashMap::new();

    let FieldList(fields) = fields;
    for Field(Id(fname), t) in fields {
        // v6.
        match t {
            Type::Struct(TitleId(typ)) => {
                if !ctx.ids.struct_names.contains(typ) {
                    return Err(CompilerError::Undefined(format!(
                        "Struct {name} field {fname} type {typ} undefined."
                    )));
                }
            }
            Type::Bool | Type::Int | Type::String => {}
        }
        field_types.insert(t.into());

        // v6.1
        if let Some(_) = fields_map.insert(fname.clone(), t.clone()) {
            return Err(CompilerError::AlreadyDefined(format!(
                "Struct {name} field {fname} already defined."
            )));
        }
    }

    Ok((name.clone(), field_types, fields_map, tags_set))
}

fn validate_action(
    action: &&Action,
    ctx: &CompilerContext,
) -> Result<(String, Vec<Type>, Type, Fallback), CompilerError> {
    let &Action(Id(name), types, ret, fallback) = action;

    let TypeList(types) = types;
    let types = types.clone();

    // v8.
    for typ in &types {
        if !ctx.types.contains(typ) {
            return Err(CompilerError::TypeError(format!(
                "Action {name} has argument of unknown type {typ:?}."
            )));
        }
    }

    // v8.1
    if !ctx.types.contains(ret) {
        return Err(CompilerError::TypeError(format!(
            "Action {name} has unknown return type {ret:?}."
        )));
    }

    // v8.2
    if !matches!(ret, Type::Struct(_)) {
        return Err(CompilerError::TypeError(format!(
            "Action {name} has non-struct return type {ret:?}."
        )));
    }

    Ok((name.clone(), types, ret.clone(), fallback.clone()))
}

fn validate_action_group(
    action_group: &&ActionG,
    ctx: &CompilerContext,
) -> Result<(String, HashSet<String>), CompilerError> {
    let &ActionG(Id(name), IdList(actions)) = action_group;
    let mut items: HashSet<String> = HashSet::new();

    // v9.
    for Id(action) in actions {
        if !ctx.ids.action_exists(action) {
            return Err(CompilerError::Undefined(format!(
                "ActionGroup {name} member {action} undefined."
            )));
        }
        items.insert(action.clone());
    }

    Ok((name.clone(), items))
}

struct RuleBlockContext {
    arg_types: HashMap<String, Type>, // name -> type
    name: String,
}
impl RuleBlockContext {
    fn new(name: String) -> Self {
        Self {
            arg_types: HashMap::new(),
            name: name,
        }
    }
}

fn get_field_type(
    field_list: FieldValue,
    rule_ctx: &RuleBlockContext,
    ctx: &CompilerContext,
) -> Result<Type, CompilerError> {
    let FieldValue(parts) = field_list.clone();
    let mut parts = parts.iter();
    let Id(field_name) = parts.next().expect("FieldValue cannot be empty.");
    let Some(field_type) = rule_ctx.arg_types.get(field_name) else {
        return Err(CompilerError::Undefined(format!(
            "RuleBlock {}, field {field_name} is referenced but not defined.",
            rule_ctx.name
        )));
    };
    let mut field_type = field_type.clone();

    while let Some(Id(field_name)) = &parts.next() {
        match ctx
            .field_types
            .get(&(field_type.clone(), field_name.clone()))
        {
            Some(v) => {
                field_type = v.clone();
            }
            None => {
                return Err(CompilerError::Undefined(format!(
                    "No field {field_name} of type {field_type:?}"
                )));
            }
        };
    }

    Ok(field_type)
}

fn validate_rule_block(
    rule_block: &&RuleBlock,
    ctx: &CompilerContext,
) -> Result<(), CompilerError> {
    let &RuleBlock(Id(name), args, Rules(rules)) = rule_block;
    let mut block_ctx = RuleBlockContext::new(name.clone());

    let FieldList(args) = args;

    let mut arg_types: Vec<Type> = Vec::new();

    for Field(Id(name), typ) in args {
        arg_types.push(typ.clone());
        if let Some(_) = block_ctx.arg_types.insert(name.clone(), typ.clone()) {
            return Err(CompilerError::AlreadyDefined(format!(
                "RuleBlock {name} argument {name:?} is defined multiple times."
            )));
        }
    }

    let types = ctx
        .action_signatures
        .get(name)
        .expect("action_signatures should be populated and name should be defined here.");

    // v13.
    if &arg_types != types {
        return Err(CompilerError::TypeError(format!("RuleBlock {name} has incorrect argument types {arg_types:?}. They should be {types:?}.")));
    }

    // v14.
    for rule in rules {
        match rule {
            Rule::Allow(c) => {
                validate_condition(c, &block_ctx, &ctx)?;
            }
            Rule::Deny(c) => {
                validate_condition(c, &block_ctx, &ctx)?;
            }
            Rule::Apply(IdList(tags), c) => {
                // v15.
                for Id(tag) in tags {
                    if !ctx.ids.tag_exists(tag) {
                        return Err(CompilerError::Undefined(format!(
                            "Apply Rule in {name} contains tag {tag} which is undefined."
                        )));
                    }
                }
                validate_condition(c, &block_ctx, &ctx)?;
            }
        }
    }

    Ok(())
}

// Requirements: BoolExpr must be valid (see validate_bool_expr)
fn validate_condition(
    cond: &Condition,
    block_ctx: &RuleBlockContext,
    ctx: &CompilerContext,
) -> Result<(), CompilerError> {
    match cond {
        Condition::Always | Condition::Never => Ok(()),
        Condition::When(c) => validate_bool_expr(c.as_ref(), block_ctx, ctx),
    }
}

// Requirements:
// v16. And,Or,Not valid if contained BoolExpr is valid.
//    . Rule valid if
// v17.     action exists
// v18.     rule block for action exists
// v19.     args are valid
//   v19.1      args types match action types
//    . Gt,Lt,Gte,Lte valid if
// v20.     contained expr valid
// v21.     contained expr nums
//    . Eq,Neq valid if
// v22.     contained expr valid
// v23.     contained expr both same type
//    . Match valid if
// v24.     contained expr valid
// v25.     Left is String
// v26.     Right is regex
//    . Contains...,Lacks... valid if
// v27.     contained expr valid
// v28.     contained expr is TagList or Struct
// v29.     tag/taglist exist
fn validate_bool_expr(
    expr: &BoolExpr,
    block_ctx: &RuleBlockContext,
    ctx: &CompilerContext,
) -> Result<(), CompilerError> {
    match expr {
        // v16.
        BoolExpr::And(a, b) => {
            validate_bool_expr(a.as_ref(), block_ctx, ctx)?;
            validate_bool_expr(b.as_ref(), block_ctx, ctx)?;
            Ok(())
        }
        BoolExpr::Or(a, b) => {
            validate_bool_expr(a.as_ref(), block_ctx, ctx)?;
            validate_bool_expr(b.as_ref(), block_ctx, ctx)?;
            Ok(())
        }
        BoolExpr::Not(a) => {
            validate_bool_expr(a.as_ref(), block_ctx, ctx)?;
            Ok(())
        }
        BoolExpr::Rule(Id(name), args) => {
            // v17.
            if !ctx.ids.action_exists(name) {
                return Err(CompilerError::Undefined(format!(
                    "In RuleBlock {}, Action {name} not defined for \"<rule> allowed\" condition.",
                    block_ctx.name
                )));
            }

            // v18.
            if !ctx.ids.rule_exists(name) {
                return Err(CompilerError::Undefined(format!(
                    "In RuleBlock {}, Action {name} exists but has no defined RuleBlock.",
                    block_ctx.name
                )));
            }

            let action_types = ctx
                .action_signatures
                .get(name)
                .expect("name should be a key here.");
            let mut these_arg_types: Vec<Type> = Vec::new();
            let ExprList(args) = args;

            // v19.
            for expr in args {
                these_arg_types.push(validate_expr(expr, block_ctx, ctx)?.try_into()?);
            }
            // v19.1
            if action_types != &these_arg_types {
                return Err(CompilerError::TypeError(format!("RuleBlock {} takes args of type {these_arg_types:?} but Action of this name takes args {action_types:?}", block_ctx.name)));
            }

            Ok(())
        }
        // v20, v21.
        BoolExpr::Gt(a, b) => {
            if validate_expr(a.as_ref(), block_ctx, ctx)? != ExprType::Int {
                return Err(CompilerError::TypeError(format!(
                    "RuleBlock {} condition {a:?} > {b:?} is invalid. {a:?} must be int.",
                    block_ctx.name
                )));
            }
            if validate_expr(b.as_ref(), block_ctx, ctx)? != ExprType::Int {
                return Err(CompilerError::TypeError(format!(
                    "RuleBlock {} condition {a:?} > {b:?} is invalid. {b:?} must be int.",
                    block_ctx.name
                )));
            }
            Ok(())
        }
        BoolExpr::Lt(a, b) => {
            if validate_expr(a.as_ref(), block_ctx, ctx)? != ExprType::Int {
                return Err(CompilerError::TypeError(format!(
                    "RuleBlock {} condition {a:?} < {b:?} is invalid. {a:?} must be int.",
                    block_ctx.name
                )));
            }
            if validate_expr(b.as_ref(), block_ctx, ctx)? != ExprType::Int {
                return Err(CompilerError::TypeError(format!(
                    "RuleBlock {} condition {a:?} < {b:?} is invalid. {b:?} must be int.",
                    block_ctx.name
                )));
            }
            Ok(())
        }
        BoolExpr::Gte(a, b) => {
            if validate_expr(a.as_ref(), block_ctx, ctx)? != ExprType::Int {
                return Err(CompilerError::TypeError(format!(
                    "RuleBlock {} condition {a:?} >= {b:?} is invalid. {a:?} must be int.",
                    block_ctx.name
                )));
            }
            if validate_expr(b.as_ref(), block_ctx, ctx)? != ExprType::Int {
                return Err(CompilerError::TypeError(format!(
                    "RuleBlock {} condition {a:?} >= {b:?} is invalid. {b:?} must be int.",
                    block_ctx.name
                )));
            }
            Ok(())
        }
        BoolExpr::Lte(a, b) => {
            if validate_expr(a.as_ref(), block_ctx, ctx)? != ExprType::Int {
                return Err(CompilerError::TypeError(format!(
                    "RuleBlock {} condition {a:?} <= {b:?} is invalid. {a:?} must be int.",
                    block_ctx.name
                )));
            }
            if validate_expr(b.as_ref(), block_ctx, ctx)? != ExprType::Int {
                return Err(CompilerError::TypeError(format!(
                    "RuleBlock {} condition {a:?} <= {b:?} is invalid. {b:?} must be int.",
                    block_ctx.name
                )));
            }
            Ok(())
        }
        // v22, v23.
        BoolExpr::Eq(a, b) => {
            let t1 = validate_expr(a.as_ref(), block_ctx, ctx)?;
            let t2 = validate_expr(b.as_ref(), block_ctx, ctx)?;

            if t1 != t2 {
                return Err(CompilerError::TypeError(format!("RuleBlock {} condition {a:?} == {b:?} is invalid. Left and right must be the same type but they are {t1:?}, {t2:?}.", block_ctx.name)));
            }
            Ok(())
        }
        BoolExpr::Neq(a, b) => {
            let t1 = validate_expr(a.as_ref(), block_ctx, ctx)?;
            let t2 = validate_expr(b.as_ref(), block_ctx, ctx)?;

            if t1 != t2 {
                return Err(CompilerError::TypeError(format!("RuleBlock {} condition {a:?} != {b:?} is invalid. Left and right must be the same type but they are {t1:?}, {t2:?}.", block_ctx.name)));
            }
            Ok(())
        }
        BoolExpr::Match(a, b) => {
            // v24, v25, v26.
            if validate_expr(a.as_ref(), block_ctx, ctx)? != ExprType::String {
                return Err(CompilerError::TypeError(format!(
                    "RuleBlock {} condition {a:?} matches {b:?} is invalid. {a:?} must be string.",
                    block_ctx.name
                )));
            }
            if validate_expr(b.as_ref(), block_ctx, ctx)? != ExprType::Regex {
                return Err(CompilerError::TypeError(format!(
                    "RuleBlock {} condition {a:?} matches {b:?} is invalid. {b:?} must be regex.",
                    block_ctx.name
                )));
            }
            Ok(())
        }
        // v27, v28, v29.
        BoolExpr::Contains(a, Id(b)) => {
            let t1 = validate_expr(a.as_ref(), block_ctx, ctx)?;

            if t1 != ExprType::TagList && !matches!(t1, ExprType::Struct(_)) {
                return Err(CompilerError::TypeError(format!("RuleBlock {} condition {a:?} contains {b:?} is invalid. {a:?} must be any_arg, every_arg or a struct type arg but is {t1:?}.", block_ctx.name)));
            }

            if !ctx.ids.tag_names.contains(b) {
                return Err(CompilerError::Undefined(format!("RuleBlock {} condition {a:?} contains {b:?} is invalid. {b:?} is an undefined tag (tag groups are not valid here).", block_ctx.name)));
            }

            Ok(())
        }
        BoolExpr::ContainsAny(a, IdList(tags)) => {
            let t1 = validate_expr(a.as_ref(), block_ctx, ctx)?;

            if t1 != ExprType::TagList && !matches!(t1, ExprType::Struct(_)) {
                return Err(CompilerError::TypeError(format!("RuleBlock {} condition {a:?} contains_any {tags:?} is invalid. {a:?} must be any_arg, every_arg or a struct type arg but is {t1:?}.", block_ctx.name)));
            }

            for Id(tag) in tags {
                if !ctx.ids.tag_exists(tag) {
                    return Err(CompilerError::Undefined(format!("RuleBlock {} condition {a:?} contains_any {tags:?} is invalid. {tag:?} is an undefined tag.", block_ctx.name)));
                }
            }

            Ok(())
        }
        BoolExpr::ContainsAll(a, IdList(tags)) => {
            let t1 = validate_expr(a.as_ref(), block_ctx, ctx)?;

            if t1 != ExprType::TagList && !matches!(t1, ExprType::Struct(_)) {
                return Err(CompilerError::TypeError(format!("RuleBlock {} condition {a:?} contains_all {tags:?} is invalid. {a:?} must be any_arg, every_arg or a struct type arg but is {t1:?}.", block_ctx.name)));
            }

            for Id(tag) in tags {
                if !ctx.ids.tag_exists(tag) {
                    return Err(CompilerError::Undefined(format!("RuleBlock {} condition {a:?} contains_all {tags:?} is invalid. {tag:?} is an undefined tag.", block_ctx.name)));
                }
            }

            Ok(())
        }
        BoolExpr::Lacks(a, Id(b)) => {
            let t1 = validate_expr(a.as_ref(), block_ctx, ctx)?;

            if t1 != ExprType::TagList && !matches!(t1, ExprType::Struct(_)) {
                return Err(CompilerError::TypeError(format!("RuleBlock {} condition {a:?} lacks {b:?} is invalid. {a:?} must be any_arg, every_arg or a struct type arg but is {t1:?}.", block_ctx.name)));
            }

            if !ctx.ids.tag_names.contains(b) {
                return Err(CompilerError::Undefined(format!("RuleBlock {} condition {a:?} lacks {b:?} is invalid. {b:?} is an undefined tag (tag groups are not valid here).", block_ctx.name)));
            }

            Ok(())
        }
        BoolExpr::LacksAny(a, IdList(tags)) => {
            let t1 = validate_expr(a.as_ref(), block_ctx, ctx)?;

            if t1 != ExprType::TagList && !matches!(t1, ExprType::Struct(_)) {
                return Err(CompilerError::TypeError(format!("RuleBlock {} condition {a:?} lacks_any {tags:?} is invalid. {a:?} must be any_arg, every_arg or a struct type arg but is {t1:?}.", block_ctx.name)));
            }

            for Id(tag) in tags {
                if !ctx.ids.tag_exists(tag) {
                    return Err(CompilerError::Undefined(format!("RuleBlock {} condition {a:?} lacks_any {tags:?} is invalid. {tag:?} is an undefined tag.", block_ctx.name)));
                }
            }

            Ok(())
        }
        BoolExpr::LacksAll(a, IdList(tags)) => {
            let t1 = validate_expr(a.as_ref(), block_ctx, ctx)?;

            if t1 != ExprType::TagList && !matches!(t1, ExprType::Struct(_)) {
                return Err(CompilerError::TypeError(format!("RuleBlock {} condition {a:?} lacks_all {tags:?} is invalid. {a:?} must be any_arg, every_arg or a struct type arg but is {t1:?}.", block_ctx.name)));
            }

            for Id(tag) in tags {
                if !ctx.ids.tag_exists(tag) {
                    return Err(CompilerError::Undefined(format!("RuleBlock {} condition {a:?} lacks_all {tags:?} is invalid. {tag:?} is an undefined tag.", block_ctx.name)));
                }
            }

            Ok(())
        }
        BoolExpr::True | BoolExpr::False => Ok(()),
    }
}

// Requirements:
// v30. Add valid if left and right are valid and ints
// v31. Sub valid if left and right are valid and ints
// v32. Mul valid if left and right are valid and ints
// v33. Div valid if left and right are valid and ints
// v34. Neg valid if right is valid and int
//    . Field is valid
// v35.     if defined
// v36.     well typed
// v37. Num is valid int
// v38. String is valid string
// v39. Regex is valid regex
// v40. Any/Every Arg is valid Taglist
fn validate_expr(
    expr: &Expr,
    block_ctx: &RuleBlockContext,
    ctx: &CompilerContext,
) -> Result<ExprType, CompilerError> {
    match expr {
        Expr::Add(a, b) => {
            // v30.
            if validate_expr(a.as_ref(), block_ctx, ctx)? != ExprType::Int {
                return Err(CompilerError::TypeError(format!(
                    "RuleBlock {} expression {a:?} + {b:?} is invalid. {a:?} must be int.",
                    block_ctx.name
                )));
            }
            if validate_expr(b.as_ref(), block_ctx, ctx)? != ExprType::Int {
                return Err(CompilerError::TypeError(format!(
                    "RuleBlock {} expression {a:?} + {b:?} is invalid. {b:?} must be int.",
                    block_ctx.name
                )));
            }
            Ok(ExprType::Int)
        }
        Expr::Sub(a, b) => {
            // v31.
            if validate_expr(a.as_ref(), block_ctx, ctx)? != ExprType::Int {
                return Err(CompilerError::TypeError(format!(
                    "RuleBlock {} expression {a:?} - {b:?} is invalid. {a:?} must be int.",
                    block_ctx.name
                )));
            }
            if validate_expr(b.as_ref(), block_ctx, ctx)? != ExprType::Int {
                return Err(CompilerError::TypeError(format!(
                    "RuleBlock {} expression {a:?} - {b:?} is invalid. {b:?} must be int.",
                    block_ctx.name
                )));
            }
            Ok(ExprType::Int)
        }
        Expr::Mul(a, b) => {
            // v32.
            if validate_expr(a.as_ref(), block_ctx, ctx)? != ExprType::Int {
                return Err(CompilerError::TypeError(format!(
                    "RuleBlock {} expression {a:?} * {b:?} is invalid. {a:?} must be int.",
                    block_ctx.name
                )));
            }
            if validate_expr(b.as_ref(), block_ctx, ctx)? != ExprType::Int {
                return Err(CompilerError::TypeError(format!(
                    "RuleBlock {} expression {a:?} * {b:?} is invalid. {b:?} must be int.",
                    block_ctx.name
                )));
            }
            Ok(ExprType::Int)
        }
        Expr::Div(a, b) => {
            // v33.
            if validate_expr(a.as_ref(), block_ctx, ctx)? != ExprType::Int {
                return Err(CompilerError::TypeError(format!(
                    "RuleBlock {} expression {a:?} / {b:?} is invalid. {a:?} must be int.",
                    block_ctx.name
                )));
            }
            if validate_expr(b.as_ref(), block_ctx, ctx)? != ExprType::Int {
                return Err(CompilerError::TypeError(format!(
                    "RuleBlock {} expression {a:?} / {b:?} is invalid. {b:?} must be int.",
                    block_ctx.name
                )));
            }
            Ok(ExprType::Int)
        }
        Expr::Neg(a) => {
            // v34.
            if validate_expr(a.as_ref(), block_ctx, ctx)? != ExprType::Int {
                return Err(CompilerError::TypeError(format!(
                    "RuleBlock {} expression -{a:?} is invalid. {a:?} must be int.",
                    block_ctx.name
                )));
            }
            Ok(ExprType::Int)
        }
        Expr::Field(f) => {
            // v35, v36.
            Ok(get_field_type(f.clone(), block_ctx, ctx)?.into())
        }
        Expr::Num(_) => {
            // v37.
            Ok(ExprType::Int)
        }
        Expr::String(_) => {
            // v38.
            Ok(ExprType::String)
        }
        Expr::Regex(r) => {
            // v39.
            if let Err(e) = Regex::new(r) {
                return Err(CompilerError::InvalidRegex(format!(
                    "RuleBlock {} expression {r:?} is invalid: {:?}",
                    block_ctx.name, e
                )));
            }
            Ok(ExprType::Regex)
        }
        Expr::AnyArg | Expr::EveryArg => {
            // v40.
            Ok(ExprType::TagList)
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::ast::*;
    use crate::collect;

    #[allow(unused)]
    macro_rules! or {
        ($a:expr, $b:expr) => {
            Box::new(BoolExpr::Or($a, $b))
        };
    }
    #[allow(unused)]
    macro_rules! and {
        ($a:expr, $b:expr) => {
            Box::new(BoolExpr::And($a, $b))
        };
    }
    #[allow(unused)]
    macro_rules! not {
        ($a:expr) => {
            Box::new(BoolExpr::Not($a))
        };
    }
    #[allow(unused)]
    macro_rules! rule {
        ($a:literal, $b:expr) => {
            Box::new(BoolExpr::Rule(Id(String::from($a)), ExprList($b)))
        };
    }
    #[allow(unused)]
    macro_rules! gt {
        ($a:expr, $b:expr) => {
            Box::new(BoolExpr::Gt($a, $b))
        };
    }
    #[allow(unused)]
    macro_rules! lt {
        ($a:expr, $b:expr) => {
            Box::new(BoolExpr::Lt($a, $b))
        };
    }
    #[allow(unused)]
    macro_rules! gte {
        ($a:expr, $b:expr) => {
            Box::new(BoolExpr::Gte($a, $b))
        };
    }
    #[allow(unused)]
    macro_rules! lte {
        ($a:expr, $b:expr) => {
            Box::new(BoolExpr::Lte($a, $b))
        };
    }
    #[allow(unused)]
    macro_rules! eq {
        ($a:expr, $b:expr) => {
            Box::new(BoolExpr::Eq($a, $b))
        };
    }
    #[allow(unused)]
    macro_rules! neq {
        ($a:expr, $b:expr) => {
            Box::new(BoolExpr::Neq($a, $b))
        };
    }
    #[allow(unused)]
    macro_rules! match_ {
        ($a:expr, $b:expr) => {
            Box::new(BoolExpr::Match($a, $b))
        };
    }
    #[allow(unused)]
    macro_rules! contains {
        ($a:expr, $b:expr) => {
            Box::new(BoolExpr::Contains($a, Id(String::from($b))))
        };
    }
    #[allow(unused)]
    macro_rules! contains_all {($a:expr, [$($b:literal),*]) => {
        Box::new(BoolExpr::ContainsAll($a, IdList(vec![$(Id(String::from($b))),*])))
    }}
    #[allow(unused)]
    macro_rules! contains_any {($a:expr, [$($b:literal),*]) => {
        Box::new(BoolExpr::ContainsAny($a, IdList(vec![$(Id(String::from($b))),*])))
    }}
    #[allow(unused)]
    macro_rules! lacks {
        ($a:expr, $b:expr) => {
            Box::new(BoolExpr::Lacks($a, Id(String::from($b))))
        };
    }
    #[allow(unused)]
    macro_rules! lacks_all {($a:expr, [$($b:literal),*]) => {
        Box::new(BoolExpr::LacksAll($a, IdList(vec![$(Id(String::from($b))),*])))
    }}
    #[allow(unused)]
    macro_rules! lacks_any {($a:expr, [$($b:literal),*]) => {
        Box::new(BoolExpr::LacksAny($a, IdList(vec![$(Id(String::from($b))),*])))
    }}
    #[allow(unused)]
    macro_rules! true_ {
        () => {
            Box::new(BoolExpr::True)
        };
    }
    #[allow(unused)]
    macro_rules! false_ {
        () => {
            Box::new(BoolExpr::False)
        };
    }
    #[allow(unused)]
    macro_rules! add {
        ($a:expr, $b:expr) => {
            Box::new(Expr::Add($a, $b))
        };
    }
    #[allow(unused)]
    macro_rules! sub {
        ($a:expr, $b:expr) => {
            Box::new(Expr::Sub($a, $b))
        };
    }
    #[allow(unused)]
    macro_rules! mul {
        ($a:expr, $b:expr) => {
            Box::new(Expr::Mul($a, $b))
        };
    }
    #[allow(unused)]
    macro_rules! div {
        ($a:expr, $b:expr) => {
            Box::new(Expr::Div($a, $b))
        };
    }
    #[allow(unused)]
    macro_rules! neg {
        ($a:expr) => {
            Box::new(Expr::Neg($a))
        };
    }
    #[allow(unused)]
    macro_rules! num {
        ($a:literal) => {
            Box::new(Expr::Num($a))
        };
    }
    #[allow(unused)]
    macro_rules! field {($($a:literal).*) => { Box::new(Expr::Field(FieldValue(vec![$(Id(String::from($a))),*]))) }}
    #[allow(unused)]
    macro_rules! string {
        ($a:literal) => {
            Box::new(Expr::String(String::from($a)))
        };
    }
    #[allow(unused)]
    macro_rules! regex {
        ($a:literal) => {
            Box::new(Expr::Regex(String::from($a)))
        };
    }
    #[allow(unused)]
    macro_rules! any_arg {
        () => {
            Box::new(Expr::AnyArg)
        };
    }
    #[allow(unused)]
    macro_rules! every_arg {
        () => {
            Box::new(Expr::EveryArg)
        };
    }
    #[allow(unused)]
    macro_rules! struct_type {
        ($a:literal) => {
            Type::Struct(TitleId(String::from($a)))
        };
    }
    #[allow(unused)]
    macro_rules! deny {
        () => {
            Fallback::Deny
        };
    }
    #[allow(unused)]
    macro_rules! allow {
        () => {
            Fallback::Allow
        };
    }
    #[allow(unused)]
    macro_rules! warn {
        () => {
            Fallback::Warn
        };
    }
    #[allow(unused)]
    macro_rules! always {
        () => {
            Condition::Always
        };
    }
    #[allow(unused)]
    macro_rules! never {
        () => {
            Condition::Never
        };
    }
    #[allow(unused)]
    macro_rules! when {
        ($a:expr) => {
            Condition::When($a)
        };
    }

    // Test mock - helper functions to build test data
    #[allow(unused)]
    mod mock {
        use super::*;

        pub fn tag(name: &str) -> CodeItem {
            CodeItem::Tag(Tag(Id(name.to_string())))
        }

        pub fn tag_group(name: &str, members: Vec<&str>) -> CodeItem {
            CodeItem::TagG(TagG(
                Id(name.to_string()),
                IdList(members.iter().map(|s| Id(s.to_string())).collect()),
            ))
        }

        pub fn struct_def(name: &str, tags: Vec<&str>, fields: Vec<(&str, Type)>) -> CodeItem {
            CodeItem::Struct(Struct(
                TitleId(name.to_string()),
                IdList(tags.iter().map(|s| Id(s.to_string())).collect()),
                FieldList(
                    fields
                        .iter()
                        .map(|(n, t)| Field(Id(n.to_string()), t.clone()))
                        .collect(),
                ),
            ))
        }

        pub fn action(name: &str, args: Vec<Type>, ret: Type, fallback: Fallback) -> CodeItem {
            CodeItem::Action(Action(Id(name.to_string()), TypeList(args), ret, fallback))
        }

        pub fn action_group(name: &str, members: Vec<&str>) -> CodeItem {
            CodeItem::ActionG(ActionG(
                Id(name.to_string()),
                IdList(members.iter().map(|s| Id(s.to_string())).collect()),
            ))
        }

        pub fn rule_block(
            name: &str,
            args: Vec<(&str, Type)>,
            rules: Vec<Rule>,
        ) -> CodeItem {
            CodeItem::RuleBlock(RuleBlock(
                Id(name.to_string()),
                FieldList(
                    args.iter()
                        .map(|(n, t)| Field(Id(n.to_string()), t.clone()))
                        .collect(),
                ),
                Rules(rules),
            ))
        }

        pub fn apply(tags: Vec<&str>, condition: Condition) -> Rule {
            Rule::Apply(
                IdList(tags.iter().map(|s| Id(s.to_string())).collect()),
                condition,
            )
        }
        pub fn allow(condition: Condition) -> Rule {
            Rule::Allow(condition)
        }
        pub fn deny(condition: Condition) -> Rule {
            Rule::Deny(condition)
        }
    }

    // Helper function to create empty Vecs
    fn v<T>() -> Vec<T> {
        Vec::new()
    }

    struct TestCase {
        desc: String,
        code: Vec<CodeItem>,
        should_pass: bool,
        error_contains: String,
    }
    impl TestCase {
        fn new_passing(desc: impl Into<String>, code: Vec<CodeItem>) -> Self {
            Self {
                desc: desc.into(),
                code: code,
                should_pass: true,
                error_contains: "".to_string(),
            }
        }

        fn new_failing(
            desc: impl Into<String>,
            code: Vec<CodeItem>,
            err_contains: impl Into<String>,
        ) -> Self {
            Self {
                desc: desc.into(),
                code: code,
                should_pass: false,
                error_contains: err_contains.into(),
            }
        }
    }

    // Helper function to run a test
    fn test(case: TestCase) {
        let result =
            collect::collect_code(&case.code).and_then(|(code, ids)| validate_code(&code, ids));

        if case.should_pass {
            assert!(
                result.is_ok(),
                "[{}] should pass but failed with {:?}.",
                case.desc,
                result
            );
        } else {
            match result {
                Ok(res) => panic!("[{}] should fail but passed with {:?}", case.desc, res),
                Err(err) => {
                    assert!(
                        format!("{:?}", err).contains(&case.error_contains),
                        "[{}] should fail with [{}] but failed with {:?}.",
                        case.desc,
                        case.error_contains,
                        err
                    );
                }
            }
        }
    }

    // Helper function to run multiple tests
    fn test_cases(cases: Vec<TestCase>) {
        for case in cases {
            test(case);
        }
    }

    #[test]
    fn test_valid_idents() {
        test_cases(vec![
            TestCase::new_passing("Different tags", vec![mock::tag("a"), mock::tag("b")]),
            TestCase::new_failing(
                "Re-definition",
                vec![mock::tag("a"), mock::tag("a")],
                "already defined",
            ),
            TestCase::new_passing(
                "Different types",
                vec![mock::tag("a"), mock::struct_def("b", v(), v())],
            ),
            TestCase::new_passing(
                "RuleBlock is action",
                vec![
                    mock::struct_def("A", v(), v()),
                    mock::action("a", v(), struct_type!("A"), deny!()),
                    mock::rule_block("a", v(), v()),
                ],
            ),
            TestCase::new_failing(
                "RuleBlock is not action",
                vec![mock::rule_block("a", v(), v())],
                "not a defined Action",
            ),
            TestCase::new_failing(
                "Name collision",
                vec![mock::tag("a"), mock::struct_def("a", v(), v())],
                "already defined",
            ),
        ]);
    }

    #[test]
    fn test_tag_group_members_exist() {
        test_cases(vec![
            TestCase::new_passing(
                "Tag group members exist",
                vec![
                    mock::tag("a"),
                    mock::tag("b"),
                    mock::tag_group("group", vec!["a", "b"]),
                ],
            ),
            TestCase::new_failing(
                "Tag group members dont exist",
                vec![mock::tag("b"), mock::tag_group("group", vec!["a", "b"])],
                "undefined",
            ),
        ]);
    }

    #[test]
    fn test_tag_group_not_cyclic() {
        test_cases(vec![
            TestCase::new_passing(
                "Tag group no cycles",
                vec![
                    mock::tag("a"),
                    mock::tag("b"),
                    mock::tag("c"),
                    mock::tag("d"),
                    mock::tag_group("group1", vec!["a", "b"]),
                    mock::tag_group("group2", vec!["group1", "c"]),
                    mock::tag_group("group3", vec!["group1", "group2", "a", "d"]),
                ],
            ),
            TestCase::new_failing(
                "Tag group with cycles",
                vec![
                    mock::tag("a"),
                    mock::tag("b"),
                    mock::tag("c"),
                    mock::tag("d"),
                    mock::tag_group("group1", vec!["a", "b", "group3"]),
                    mock::tag_group("group2", vec!["group1", "c"]),
                    mock::tag_group("group3", vec!["group2", "a", "d"]),
                ],
                "Cycle in tag groups",
            ),
        ]);
    }

    #[test]
    fn test_struct_tags_exist() {
        test_cases(vec![
            TestCase::new_passing(
                "struct tags exist",
                vec![
                    mock::tag("a"),
                    mock::tag("b"),
                    mock::struct_def("This", vec!["a", "b"], v()),
                ],
            ),
            TestCase::new_failing(
                "struct tags dont exist",
                vec![
                    mock::tag("a"),
                    mock::struct_def("This", vec!["a", "b"], v()),
                ],
                "undefined",
            ),
        ]);
    }

    #[test]
    fn test_struct_field_types_exist() {
        test_cases(vec![
            TestCase::new_passing(
                "Struct field types exist",
                vec![
                    mock::struct_def("That", v(), v()),
                    mock::struct_def(
                        "This",
                        v(),
                        vec![("a", struct_type!("That")), ("b", Type::Int)],
                    ),
                ],
            ),
            TestCase::new_failing(
                "Struct field types dont exist",
                vec![mock::struct_def(
                    "This",
                    v(),
                    vec![("a", struct_type!("That")), ("b", Type::Int)],
                )],
                "undefined",
            ),
        ]);
    }

    #[test]
    fn test_struct_field_names_valid() {
        test_cases(vec![
            TestCase::new_passing(
                "Struct field types exist",
                vec![mock::struct_def(
                    "This",
                    v(),
                    vec![("a", Type::Int), ("b", Type::Int)],
                )],
            ),
            TestCase::new_failing(
                "Struct field types exist",
                vec![mock::struct_def(
                    "This",
                    v(),
                    vec![("a", Type::Int), ("a", Type::Int)],
                )],
                "already defined",
            ),
        ]);
    }

    #[test]
    fn test_structs_not_cyclic() {
        test_cases(vec![
            TestCase::new_passing(
                "structs without cycle",
                vec![
                    mock::struct_def("Other", v(), v()),
                    mock::struct_def(
                        "That",
                        v(),
                        vec![("a", struct_type!("Other")), ("b", Type::Int)],
                    ),
                    mock::struct_def(
                        "This",
                        v(),
                        vec![("a", struct_type!("That")), ("b", struct_type!("Other"))],
                    ),
                ],
            ),
            TestCase::new_failing(
                "structs without cycle",
                vec![
                    mock::struct_def(
                        "Other",
                        v(),
                        vec![("a", struct_type!("This")), ("b", Type::Int)],
                    ),
                    mock::struct_def(
                        "That",
                        v(),
                        vec![("a", struct_type!("Other")), ("b", Type::Int)],
                    ),
                    mock::struct_def(
                        "This",
                        v(),
                        vec![("a", struct_type!("That")), ("b", struct_type!("Other"))],
                    ),
                ],
                "Cycle in struct field",
            ),
        ]);
    }

    #[test]
    fn test_action_arg_types_exist() {
        test_cases(vec![
            TestCase::new_passing(
                "action arg types exist",
                vec![
                    mock::struct_def("This", v(), v()),
                    mock::action("read", vec![struct_type!("This")], struct_type!("This"), deny!()),
                    mock::action("write", vec![Type::Int], struct_type!("This"), deny!()),
                ],
            ),
            TestCase::new_failing(
                "action arg types dont exist",
                vec![
                    mock::struct_def("A", v(), v()),
                    mock::action("read", vec![struct_type!("This")], struct_type!("A"), deny!()),
                ],
                "has argument of unknown type",
            ),
        ]);
    }

    #[test]
    fn test_action_return_type_is_existing_struct() {
        test_cases(vec![
            TestCase::new_passing(
                "action return type exists",
                vec![
                    mock::struct_def("This", v(), v()),
                    mock::action("read", vec![struct_type!("This")], struct_type!("This"), deny!()),
                ],
            ),
            TestCase::new_failing(
                "action return type doesnt exist",
                vec![
                    mock::action("a", v(), struct_type!("A"), deny!()),
                ],
                "unknown return type",
            ),
            TestCase::new_failing(
                "action return type is not a struct",
                vec![
                    mock::action("a", v(), Type::Int, deny!()),
                ],
                "non-struct return type",
            ),
        ]);
    }

    #[test]
    fn test_action_group_members_exist() {
        test_cases(vec![
            TestCase::new_passing(
                "action group members exist",
                vec![
                    mock::struct_def("AA", v(), v()),
                    mock::action("this", v(), struct_type!("AA"), deny!()),
                    mock::action("that", v(), struct_type!("AA"), deny!()),
                    mock::action("other", v(), struct_type!("AA"), deny!()),
                    mock::action_group("group", vec!["this", "that", "other"]),
                ],
            ),
            TestCase::new_failing(
                "action group members dont exist",
                vec![
                    mock::struct_def("AA", v(), v()),
                    mock::action("this", v(), struct_type!("AA"), deny!()),
                    mock::action("other", v(), struct_type!("AA"), deny!()),
                    mock::action_group("group", vec!["this", "that", "other"]),
                ],
                "undefined",
            ),
        ]);
    }

    #[test]
    fn test_action_group_not_cyclic() {
        test_cases(vec![
            TestCase::new_passing(
                "Action group no cycles",
                vec![
                    mock::struct_def("AA", v(), v()),
                    mock::action("a", v(), struct_type!("AA"), deny!()),
                    mock::action("b", v(), struct_type!("AA"), deny!()),
                    mock::action("c", v(), struct_type!("AA"), deny!()),
                    mock::action("d", v(), struct_type!("AA"), deny!()),
                    mock::action_group("group1", vec!["a", "b"]),
                    mock::action_group("group2", vec!["group1", "c"]),
                    mock::action_group("group3", vec!["group1", "group2", "a", "d"]),
                ],
            ),
            TestCase::new_failing(
                "Action group with cycles",
                vec![
                    mock::struct_def("AA", v(), v()),
                    mock::action("a", v(), struct_type!("AA"), deny!()),
                    mock::action("b", v(), struct_type!("AA"), deny!()),
                    mock::action("c", v(), struct_type!("AA"), deny!()),
                    mock::action("d", v(), struct_type!("AA"), deny!()),
                    mock::action_group("group1", vec!["a", "b", "group3"]),
                    mock::action_group("group2", vec!["group1", "c"]),
                    mock::action_group("group3", vec!["group2", "a", "d"]),
                ],
                "Cycle in action groups",
            ),
        ]);
    }

    #[test]
    fn test_action_group_member_signature() {
        test_cases(vec![
            TestCase::new_passing(
                "Action group empty args",
                vec![
                    mock::struct_def("AA", v(), v()),
                    mock::action("a", v(), struct_type!("AA"), deny!()),
                    mock::action("b", v(), struct_type!("AA"), deny!()),
                    mock::action_group("group1", vec!["a", "b"]),
                ],
            ),
            TestCase::new_passing(
                "Action group matching args",
                vec![
                    mock::struct_def("AA", v(), v()),
                    mock::struct_def("This", v(), v()),
                    mock::action("a", vec![Type::Int, struct_type!("This")], struct_type!("AA"), deny!()),
                    mock::action("b", vec![Type::Int, struct_type!("This")], struct_type!("AA"), deny!()),
                    mock::action("c", vec![Type::Int, struct_type!("This")], struct_type!("AA"), deny!()),
                    mock::action_group("group1", vec!["a", "b"]),
                    mock::action_group("group2", vec!["group1", "c"]),
                ],
            ),
            TestCase::new_failing(
                "Action group non matching members",
                vec![
                    mock::struct_def("AA", v(), v()),
                    mock::action("a", v(), struct_type!("AA"), deny!()),
                    mock::action("b", vec![Type::Int], struct_type!("AA"), deny!()),
                    mock::action_group("group1", vec!["a", "b"]),
                ],
                "must share the same argument signature",
            ),
            TestCase::new_failing(
                "Action group non matching members (nested)",
                vec![
                    mock::struct_def("AA", v(), v()),
                    mock::action("a", v(), struct_type!("AA"), deny!()),
                    mock::action("b", vec![Type::Int], struct_type!("AA"), deny!()),
                    mock::action("c", v(), struct_type!("AA"), deny!()),
                    mock::action_group("group1", vec!["a", "c"]),
                    mock::action_group("group2", vec!["group1", "b"]),
                ],
                "must share the same argument signature",
            ),
        ]);
    }

    #[test]
    fn test_rule_block_action_exists() {
        test_cases(vec![
            TestCase::new_passing(
                "valid rule block",
                vec![
                    mock::struct_def("AA", v(), v()),
                    mock::action("this", v(), struct_type!("AA"), deny!()),
                    mock::rule_block("this", v(), v()),
                ],
            ),
            TestCase::new_failing(
                "rule block name is not action",
                vec![
                    mock::struct_def("AA", v(), v()),
                    mock::tag("this"),
                    mock::rule_block("this", v(), v()),
                ],
                "not Action or ActionGroup",
            ),
            TestCase::new_failing(
                "rule block redefined",
                vec![
                    mock::struct_def("AA", v(), v()),
                    mock::action("this", v(), struct_type!("AA"), deny!()),
                    mock::rule_block("this", v(), v()),
                    mock::rule_block("this", v(), v()),
                ],
                "multiple times",
            ),
        ]);
    }

    #[test]
    fn test_rule_block_signature_exists() {
        test_cases(vec![
            TestCase::new_passing(
                "rule block signature empty",
                vec![
                    mock::struct_def("AA", v(), v()),
                    mock::action("this", v(), struct_type!("AA"), deny!()),
                    mock::rule_block("this", v(), v()),
                ],
            ),
            TestCase::new_passing(
                "rule block signature exists",
                vec![
                    mock::struct_def("AA", v(), v()),
                    mock::action("this", vec![Type::Int], struct_type!("AA"), deny!()),
                    mock::rule_block("this", vec![("a", Type::Int)], v()),
                ],
            ),
            TestCase::new_passing(
                "rule block signature exists as action group",
                vec![
                    mock::struct_def("AA", v(), v()),
                    mock::action("this", vec![Type::Int], struct_type!("AA"), deny!()),
                    mock::action_group("group", vec!["this"]),
                    mock::rule_block("group", vec![("a", Type::Int)], v()),
                ],
            ),
            TestCase::new_failing(
                "rule block signature does not exist",
                vec![
                    mock::struct_def("AA", v(), v()),
                    mock::action("this", v(), struct_type!("AA"), deny!()),
                    mock::rule_block("this", vec![("a", Type::Int)], v()),
                ],
                "incorrect argument types",
            ),
            TestCase::new_failing(
                "rule block signature exists as action group",
                vec![
                    mock::struct_def("AA", v(), v()),
                    mock::action("this", vec![Type::String], struct_type!("AA"), deny!()),
                    mock::action_group("group", vec!["this"]),
                    mock::rule_block("group", vec![("a", Type::Int)], v()),
                ],
                "incorrect argument types",
            ),
        ]);
    }

    #[test]
    fn test_rule_block_arg_names_unique() {
        test_cases(vec![
            TestCase::new_passing(
                "rule block signature empty",
                vec![
                    mock::struct_def("AA", v(), v()),
                    mock::action("this", vec![Type::Int, Type::String], struct_type!("AA"), deny!()),
                    mock::rule_block(
                        "this",
                        vec![("a", Type::Int), ("b", Type::String)],
                        v(),
                    ),
                ],
            ),
            TestCase::new_failing(
                "rule block signature exists",
                vec![
                    mock::struct_def("AA", v(), v()),
                    mock::action("this", vec![Type::Int, Type::String], struct_type!("AA"), deny!()),
                    mock::rule_block(
                        "this",
                        vec![("a", Type::Int), ("a", Type::String)],
                        v(),
                    ),
                ],
                "defined multiple times",
            ),
        ]);
    }

    #[test]
    fn test_apply_tags_exist() {
        test_cases(vec![
            TestCase::new_passing(
                "Apply tags exist",
                vec![
                    mock::struct_def("AA", v(), v()),
                    mock::tag("a"),
                    mock::action("this", v(), struct_type!("AA"), deny!()),
                    mock::rule_block(
                        "this",
                        v(),
                        vec![mock::apply(vec!["a"], always!())],
                    ),
                ],
            ),
            TestCase::new_failing(
                "Apply tags dont exist",
                vec![
                    mock::struct_def("AA", v(), v()),
                    mock::action("this", v(), struct_type!("AA"), deny!()),
                    mock::rule_block(
                        "this",
                        v(),
                        vec![mock::apply(vec!["a"], always!())],
                    ),
                ],
                "undefined",
            ),
        ]);
    }

    #[test]
    fn test_rule_condition_action_exists_with_rule_block() {
        test_cases(vec![
            TestCase::new_passing(
                "Rule condition action exists and has rule block",
                vec![
                    mock::struct_def("AA", v(), v()),
                    mock::action("this", v(), struct_type!("AA"), deny!()),
                    mock::action("other", v(), struct_type!("AA"), deny!()),
                    mock::rule_block(
                        "this",
                        v(),
                        vec![mock::allow(when!(rule!("other", v())))],
                    ),
                    mock::rule_block("other", v(), v()),
                ],
            ),
            TestCase::new_failing(
                "Rule condition action doesnt exist",
                vec![
                    mock::struct_def("AA", v(), v()),
                    mock::action("this", v(), struct_type!("AA"), deny!()),
                    mock::rule_block(
                        "this",
                        v(),
                        vec![mock::allow(when!(rule!("other", v())))],
                    ),
                ],
                "not defined",
            ),
            TestCase::new_failing(
                "Rule condition action exists but has no rule block",
                vec![
                    mock::struct_def("AA", v(), v()),
                    mock::action("this", v(), struct_type!("AA"), deny!()),
                    mock::action("other", v(), struct_type!("AA"), deny!()),
                    mock::rule_block(
                        "this",
                        v(),
                        vec![mock::allow(when!(rule!("other", v())))],
                    ),
                ],
                "no defined RuleBlock",
            ),
        ]);
    }

    #[test]
    fn test_rule_condition_args_valid() {
        test_cases(vec![
            TestCase::new_passing(
                "rule condition arg types match",
                vec![
                    mock::struct_def("AA", v(), v()),
                    mock::struct_def("This", v(), vec![("b", struct_type!("That"))]),
                    mock::struct_def("That", v(), v()),
                    mock::action("this", vec![struct_type!("This")], struct_type!("AA"), deny!()),
                    mock::action("other", vec![Type::Int, struct_type!("That")], struct_type!("AA"), deny!()),
                    mock::rule_block(
                        "this",
                        vec![("a", struct_type!("This"))],
                        vec![mock::allow(when!(rule!(
                            "other",
                            vec![num!(5), field!("a"."b")]
                        )))],
                    ),
                    mock::rule_block(
                        "other",
                        vec![("a", Type::Int), ("b", struct_type!("That"))],
                        v(),
                    ),
                ],
            ),
            TestCase::new_failing(
                "rule condition arg types dont match",
                vec![
                    mock::struct_def("AA", v(), v()),
                    mock::struct_def("This", v(), vec![("b", struct_type!("That"))]),
                    mock::struct_def("That", v(), v()),
                    mock::action("this", vec![struct_type!("This")], struct_type!("AA"), deny!()),
                    mock::action("other", vec![Type::Int, struct_type!("This")], struct_type!("AA"), deny!()),
                    mock::rule_block(
                        "other",
                        vec![("a", Type::Int), ("b", struct_type!("This"))],
                        v(),
                    ),
                    mock::rule_block(
                        "this",
                        vec![("a", struct_type!("This"))],
                        vec![mock::allow(when!(rule!(
                            "other",
                            vec![num!(5), field!("a"."b")]
                        )))],
                    ),
                ],
                "takes args of type",
            ),
        ]);
    }

    #[test]
    fn test_inequality_types_valid() {
        test_cases(vec![
            TestCase::new_passing(
                "inequality types are ints",
                vec![
                    mock::struct_def("AA", v(), v()),
                    mock::action("this", vec![Type::Int], struct_type!("AA"), deny!()),
                    mock::rule_block(
                        "this",
                        vec![("a", Type::Int)],
                        vec![mock::allow(when!(gte!(num!(5), field!("a"))))],
                    ),
                ],
            ),
            TestCase::new_failing(
                "inequality types are not ints",
                vec![
                    mock::struct_def("AA", v(), v()),
                    mock::action("this", vec![Type::Int], struct_type!("AA"), deny!()),
                    mock::rule_block(
                        "this",
                        vec![("a", Type::Int)],
                        vec![mock::allow(when!(lt!(string!("this"), field!("a"))))],
                    ),
                ],
                "must be int",
            ),
        ]);
    }

    #[test]
    fn test_equalilty_types_match() {
        test_cases(vec![
            TestCase::new_passing(
                "equality types are both ints",
                vec![
                    mock::struct_def("AA", v(), v()),
                    mock::action("this", vec![Type::Int], struct_type!("AA"), deny!()),
                    mock::rule_block(
                        "this",
                        vec![("a", Type::Int)],
                        vec![mock::allow(when!(eq!(num!(5), field!("a"))))],
                    ),
                ],
            ),
            TestCase::new_failing(
                "equality types dont match",
                vec![
                    mock::struct_def("AA", v(), v()),
                    mock::action("this", vec![Type::Int], struct_type!("AA"), deny!()),
                    mock::rule_block(
                        "this",
                        vec![("a", Type::Int)],
                        vec![mock::allow(when!(eq!(string!("this"), field!("a"))))],
                    ),
                ],
                "must be the same type",
            ),
        ]);
    }

    #[test]
    fn test_match_condition_types() {
        test_cases(vec![
            TestCase::new_passing(
                "match types are correct",
                vec![
                    mock::struct_def("AA", v(), v()),
                    mock::action("this", vec![Type::Int], struct_type!("AA"), deny!()),
                    mock::rule_block(
                        "this",
                        vec![("a", Type::Int)],
                        vec![mock::allow(when!(match_!(string!("wow"), regex!("a"))))],
                    ),
                ],
            ),
            TestCase::new_failing(
                "match on int",
                vec![
                    mock::struct_def("AA", v(), v()),
                    mock::action("this", vec![Type::Int], struct_type!("AA"), deny!()),
                    mock::rule_block(
                        "this",
                        vec![("a", Type::Int)],
                        vec![mock::allow(when!(match_!(num!(5), regex!("a"))))],
                    ),
                ],
                "must be string",
            ),
            TestCase::new_failing(
                "match to string",
                vec![
                    mock::struct_def("AA", v(), v()),
                    mock::action("this", vec![Type::Int], struct_type!("AA"), deny!()),
                    mock::rule_block(
                        "this",
                        vec![("a", Type::Int)],
                        vec![mock::allow(when!(match_!(string!("hey"), string!("a"))))],
                    ),
                ],
                "must be regex",
            ),
        ]);
    }

    #[test]
    fn test_contains_condition_types() {
        test_cases(vec![
            TestCase::new_passing(
                "contains condition on TagList",
                vec![
                    mock::struct_def("AA", v(), v()),
                    mock::tag("a"),
                    mock::action("this", v(), struct_type!("AA"), deny!()),
                    mock::rule_block(
                        "this",
                        v(),
                        vec![mock::allow(when!(contains!(any_arg!(), "a")))],
                    ),
                ],
            ),
            TestCase::new_passing(
                "contains condition on struct",
                vec![
                    mock::struct_def("AA", v(), v()),
                    mock::tag("a"),
                    mock::struct_def("That", v(), v()),
                    mock::action("this", vec![struct_type!("That")], struct_type!("AA"), deny!()),
                    mock::rule_block(
                        "this",
                        vec![("b", struct_type!("That"))],
                        vec![mock::allow(when!(contains!(field!("b"), "a")))],
                    ),
                ],
            ),
            TestCase::new_failing(
                "contains condition with tag group",
                vec![
                    mock::struct_def("AA", v(), v()),
                    mock::tag("a"),
                    mock::tag_group("group", vec!["a"]),
                    mock::action("this", v(), struct_type!("AA"), deny!()),
                    mock::rule_block(
                        "this",
                        v(),
                        vec![mock::allow(when!(contains!(any_arg!(), "group")))],
                    ),
                ],
                "tag groups are not valid here",
            ),
            TestCase::new_failing(
                "contains condition on int",
                vec![
                    mock::struct_def("AA", v(), v()),
                    mock::tag("a"),
                    mock::struct_def("That", v(), v()),
                    mock::action("this", vec![Type::Int], struct_type!("AA"), deny!()),
                    mock::rule_block(
                        "this",
                        vec![("b", Type::Int)],
                        vec![mock::allow(when!(contains!(field!("b"), "a")))],
                    ),
                ],
                "must be any_arg, every_arg or a struct",
            ),
        ]);
    }

    #[test]
    fn test_contains_condition_tags_exist() {
        test_cases(vec![
            TestCase::new_passing(
                "contains a tag that exists",
                vec![
                    mock::struct_def("AA", v(), v()),
                    mock::tag("a"),
                    mock::action("this", v(), struct_type!("AA"), deny!()),
                    mock::rule_block(
                        "this",
                        v(),
                        vec![mock::allow(when!(contains!(any_arg!(), "a")))],
                    ),
                ],
            ),
            TestCase::new_passing(
                "contains_any tag that all exist",
                vec![
                    mock::struct_def("AA", v(), v()),
                    mock::tag("a"),
                    mock::tag("b"),
                    mock::tag("c"),
                    mock::tag_group("group", vec!["a", "b"]),
                    mock::action("this", v(), struct_type!("AA"), deny!()),
                    mock::rule_block(
                        "this",
                        v(),
                        vec![mock::allow(when!(contains_any!(
                            any_arg!(),
                            ["group", "c"]
                        )))],
                    ),
                ],
            ),
            TestCase::new_failing(
                "lacks a tag that doesnt exists",
                vec![
                    mock::struct_def("AA", v(), v()),
                    mock::action("this", v(), struct_type!("AA"), deny!()),
                    mock::rule_block(
                        "this",
                        v(),
                        vec![mock::allow(when!(lacks!(any_arg!(), "a")))],
                    ),
                ],
                "undefined tag",
            ),
            TestCase::new_failing(
                "lacks_any tag that dont all exist",
                vec![
                    mock::struct_def("AA", v(), v()),
                    mock::tag("a"),
                    mock::tag("b"),
                    mock::tag_group("group", vec!["a", "b"]),
                    mock::action("this", v(), struct_type!("AA"), deny!()),
                    mock::rule_block(
                        "this",
                        v(),
                        vec![mock::allow(when!(lacks_any!(any_arg!(), ["group", "c"])))],
                    ),
                ],
                "undefined tag",
            ),
        ]);
    }

    #[test]
    fn test_math_expr_types() {
        test_cases(vec![
            TestCase::new_passing(
                "Math expr of ints",
                vec![
                    mock::struct_def("AA", v(), v()),
                    mock::action("this", vec![Type::Int], struct_type!("AA"), deny!()),
                    mock::rule_block(
                        "this",
                        vec![("a", Type::Int)],
                        vec![mock::allow(when!(gt!(
                            mul!(neg!(add!(num!(1), num!(3))), div!(num!(4), field!("a"))),
                            num!(3)
                        )))],
                    ),
                ],
            ),
            TestCase::new_failing(
                "Math expr with string",
                vec![
                    mock::struct_def("AA", v(), v()),
                    mock::action("this", vec![Type::String], struct_type!("AA"), deny!()),
                    mock::rule_block(
                        "this",
                        vec![("a", Type::String)],
                        vec![mock::allow(when!(gt!(
                            mul!(neg!(add!(num!(1), num!(3))), div!(num!(4), field!("a"))),
                            num!(3)
                        )))],
                    ),
                ],
                "must be int",
            ),
        ]);
    }

    #[test]
    fn test_field_types() {
        test_cases(vec![
            TestCase::new_passing(
                "Field exists level 1",
                vec![
                    mock::struct_def("AA", v(), v()),
                    mock::action("this", vec![Type::Int], struct_type!("AA"), deny!()),
                    mock::rule_block(
                        "this",
                        vec![("a", Type::Int)],
                        vec![mock::allow(when!(eq!(field!("a"), num!(5))))],
                    ),
                ],
            ),
            TestCase::new_passing(
                "Field exists level 2",
                vec![
                    mock::struct_def("AA", v(), v()),
                    mock::struct_def("This", v(), vec![("b", Type::Int)]),
                    mock::action("this", vec![struct_type!("This")], struct_type!("AA"), deny!()),
                    mock::rule_block(
                        "this",
                        vec![("a", struct_type!("This"))],
                        vec![mock::allow(when!(eq!(field!("a"."b"), num!(5))))],
                    ),
                ],
            ),
            TestCase::new_failing(
                "Field doesnt exist level 1",
                vec![
                    mock::struct_def("AA", v(), v()),
                    mock::action("this", vec![Type::Int], struct_type!("AA"), deny!()),
                    mock::rule_block(
                        "this",
                        vec![("c", Type::Int)],
                        vec![mock::allow(when!(eq!(field!("a"), num!(5))))],
                    ),
                ],
                "not defined",
            ),
            TestCase::new_failing(
                "Field doesnt exist level 2",
                vec![
                    mock::struct_def("AA", v(), v()),
                    mock::struct_def("This", v(), vec![("c", Type::Int)]),
                    mock::action("this", vec![struct_type!("This")], struct_type!("AA"), deny!()),
                    mock::rule_block(
                        "this",
                        vec![("a", struct_type!("This"))],
                        vec![mock::allow(when!(eq!(field!("a"."b"), num!(5))))],
                    ),
                ],
                "No field",
            ),
        ]);
    }
}
