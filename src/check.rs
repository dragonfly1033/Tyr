use crate::{CompilerError, ast::{Action, ActionG, Arg, ArgList, BoolExpr, CodeItem, CodeItemType, Condition, Expr, ExprType, Field, FieldList, FieldValue, Id, IdList, Rule, RuleBlock, Rules, Struct, Tag, TagG, TitleId, Type, TypeList}};
use std::{collections::{HashMap, HashSet}, hash::Hash};


fn check_cycles<K>(graph: &HashMap<K, HashSet<K>>) -> Option<Vec<K>> 
where 
    K: Clone + Eq + Hash,
{

    let names:Vec<K> = graph.keys().into_iter().map(|s| s.clone()).collect();
        
    let mut visited: HashSet<K> = HashSet::new();
    
    for name in names {
        let mut stack: Vec<(K, Option<K>)> = Vec::new();
        let mut path: HashSet<K> = HashSet::new();
        let mut parent_map: HashMap<K, K> = HashMap::new();

        stack.push((name.clone(), None));

        while let Some((current, parent)) = stack.pop() {
            if path.contains(&current) {
                let mut cycle = vec![current.clone()];
                let Some(mut cur) = parent else {
                    return Some(vec![current]);
                };
                while cur != current {
                    cycle.push(cur.clone());
                    cur = parent_map.get(&cur).expect("Parent should be Some during backtracking cycle.").clone();
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

            if let Some(nodes) = graph.get(&current) {
                for node in nodes {
                    stack.push((node.clone(), Some(current.clone())));
                }
            }
        }
    } 

    None
}

fn flatten_graph<K>(graph: &HashMap<K, HashSet<K>>) -> HashMap<K, HashSet<K>>
where 
    K: Clone + Eq + Hash,
{
    let mut flat: HashMap<K, HashSet<K>> = HashMap::new();

    for node in graph.keys() {
        let mut stack: Vec<K> = Vec::new();
        let mut this_flat: HashSet<K> = HashSet::new();

        stack.push(node.clone());

        while let Some(current) = stack.pop() {
            if let Some(nebs) = graph.get(&current) {
                for neb in nebs {
                    if graph.contains_key(neb) {
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


#[derive(Debug,Default)]
pub(crate) struct Identifiers {
    tag_names: HashSet<String>,
    tag_group_names: HashSet<String>,
    struct_names: HashSet<String>,
    action_names: HashSet<String>,
    action_group_names: HashSet<String>,
    rule_block_names: HashSet<String>,
} 

#[derive(Debug,Default)]
pub(crate) struct CollectedCode<'a> {
    tags: Vec<&'a Tag>,
    tag_groups: Vec<&'a TagG>,
    structs: Vec<&'a Struct>,
    actions: Vec<&'a Action>,
    action_groups: Vec<&'a ActionG>,
    rule_blocks: Vec<&'a RuleBlock>,
}


/// Collect code into statement categories
/// Output should ensure 
///     - identifiers are not used in separate categories
///     - no re-definitions
pub(crate) fn collect_code(lines: &Vec<CodeItem>) -> Result<(CollectedCode, Identifiers), CompilerError> {
    let mut coll = CollectedCode::default();
    let mut ids = Identifiers::default();

    let mut cat_map: HashMap<String, CodeItemType> = HashMap::new();

    for item in lines {
        match item {
            CodeItem::Tag(t) => {
                let Tag(Id(name)) = t;
                if !ids.tag_names.insert(name.clone()) {
                    return Err(CompilerError::AlreadyDefined(format!("Tag {name} defined multiple times.")));
                }
                if let Some(cat) = cat_map.get(name) && *cat != CodeItemType::Tag {
                    return Err(CompilerError::AlreadyDefined(format!("Tag {name} defined as {cat:?} elsewhere.")));
                }
                cat_map.insert(name.clone(), CodeItemType::Tag);
                coll.tags.push(t);
            }
            CodeItem::TagG(t) => {
                let TagG(Id(name), _) = t;
                if !ids.tag_group_names.insert(name.clone()) {
                    return Err(CompilerError::AlreadyDefined(format!("TagG {name} defined multiple times.")));
                }
                if let Some(cat) = cat_map.get(name) && *cat != CodeItemType::TagG {
                    return Err(CompilerError::AlreadyDefined(format!("TagG {name} defined as {cat:?} elsewhere.")));
                }
                cat_map.insert(name.clone(), CodeItemType::TagG);
                coll.tag_groups.push(t);
            }
            CodeItem::Struct(s) => {
                let Struct(TitleId(name), ..) = s;
                if !ids.struct_names.insert(name.clone()) {
                    return Err(CompilerError::AlreadyDefined(format!("Struct {name} defined multiple times.")));
                }
                if let Some(cat) = cat_map.get(name) && *cat != CodeItemType::Struct {
                    return Err(CompilerError::AlreadyDefined(format!("Struct {name} defined as {cat:?} elsewhere.")));
                }
                cat_map.insert(name.clone(), CodeItemType::Struct);
                coll.structs.push(s);
            }
            CodeItem::Action(a) => {
                let Action(Id(name), _) = a;
                if !ids.action_names.insert(name.clone()) {
                    return Err(CompilerError::AlreadyDefined(format!("Action {name} defined multiple times.")));
                }
                if let Some(cat) = cat_map.get(name) && *cat != CodeItemType::Action {
                    return Err(CompilerError::AlreadyDefined(format!("Action {name} defined as {cat:?} elsewhere.")));
                }
                cat_map.insert(name.clone(), CodeItemType::Action);
                coll.actions.push(a);
            }
            CodeItem::ActionG(a) => {
                let ActionG(Id(name), _) = a;
                if !ids.action_group_names.insert(name.clone()) {
                    return Err(CompilerError::AlreadyDefined(format!("ActionG {name} defined multiple times.")));
                }
                if let Some(cat) = cat_map.get(name) && *cat != CodeItemType::ActionG {
                    return Err(CompilerError::AlreadyDefined(format!("ActionG {name} defined as {cat:?} elsewhere.")));
                }
                cat_map.insert(name.clone(), CodeItemType::ActionG);
                coll.action_groups.push(a);
            }
            CodeItem::RuleBlock(r) => {
                let RuleBlock(Id(name), ..) = r;
                if !ids.rule_block_names.insert(name.clone()) {
                    return Err(CompilerError::AlreadyDefined(format!("RuleBlock {name} defined multiple times.")));
                }
                if let Some(cat) = cat_map.get(name) && *cat != CodeItemType::RuleBlock {
                    return Err(CompilerError::AlreadyDefined(format!("RuleBlock {name} defined as {cat:?} not Action or ActionGroup.")));
                }
                cat_map.insert(name.clone(), CodeItemType::RuleBlock);
                coll.rule_blocks.push(r);
            }
        }
    }

    Ok((coll, ids))
}

pub(crate) struct CompilerContext {
    tag_mappings: HashMap<String, HashSet<String>>, // name -> members
    types: HashSet<String>,
    action_groups: HashMap<String, (Vec<String>, HashSet<String>)>, // name -> (arg types, members)
    field_types: HashMap<(String, String), String>, // (struct, field) -> type
    ids: Identifiers,
}

impl CompilerContext {
    fn new(ids: Identifiers) -> Self {
        let mut types: HashSet<String> = HashSet::new();
        types.insert(String::from("int"));
        types.insert(String::from("str"));
        types.insert(String::from("bool"));

        Self {
            tag_mappings: HashMap::new(),
            types: types,
            action_groups: HashMap::new(),
            field_types: HashMap::new(),
            ids: ids,
        }
    }
}


// Validation Requirements (v1 and v2 are handled during collection phase):
//   v1. Tags,TagGroups,Structs,Actions,ActionGroups identifiers are disjoint
//   v2. No re-definitions
//   v3. Members of TagGroups are defined as Tag or TagGroup
//   v4. TagGroup definitions are not cyclic 
//   v5. Struct tags exist
//   v6. Struct field types exist
//   v7. Struct definitions are not cyclic
//   v8. Action arg types exist
//   v9. Members of ActionGroups are defined as Action or ActionGroup 
//  v10. ActionGroup Definitions are not cyclic 
//  v11. Members of ActionGroups have the same arg signature 
//  v12. RuleBlocks identifiers are a subset of Action+ActionGroups
//  v13. RuleBlocks arg types match the arg signture of the named Action(Group)
//  v14. RuleBlocks Rules are valid
//      v15. Apply tags should exist
//      => Condition is valid (refer to validate_condition for requirements)
//
// Side Effects (building CompilerContext to save work)
// s1. tag_mappings: map ids of Tags and TagGroups to expanded list of Tags
// s2. field_type_mappings: map (struct, field) to a type
// s3. types: set of valid types
// s4. action_groups: map ids of Actions and ActionGroups to expanded list of Actions and Type list of args
pub fn validate_code(code: &CollectedCode, ids: Identifiers) -> Result<(), CompilerError> {
    let mut ctx = CompilerContext::new(ids);

    // Tags done.
    
    // TagGroups
    let mut tag_groups: HashMap<String, HashSet<String>> = HashMap::new();
    for tagg in &code.tag_groups {
        let (name, tags) = validate_tag_group(tagg, &ctx)?;
        tag_groups.insert(name, tags);
    }

    ctx.tag_mappings = flatten_tag_groups_if_no_cycles(tag_groups, &ctx)?;

    // Structs
    let mut type_graph: HashMap<String, HashSet<String>> = HashMap::new();
    let mut struct_graph: HashMap<String, HashMap<String, String>> = HashMap::new();
    for s in &code.structs {
        let (name, types, fields) = validate_struct(s, &ctx)?;
        // s3.
        ctx.types.insert(name.clone());
        type_graph.insert(name.clone(), types);
        struct_graph.insert(name, fields);
    }

    ctx.field_types = get_field_type_map_if_no_cycles(type_graph, struct_graph, &ctx)?;

    // Actions
    let mut action_args: HashMap<String, Vec<String>> = HashMap::new();
    for action in &code.actions {
        let (name, types) = validate_action(action, &ctx)?;
        action_args.insert(name, types);
    } 

    // ActionGroups
    let mut action_groups: HashMap<String, HashSet<String>> = HashMap::new();
    for action in &code.action_groups {
        let (name, actions) = validate_action_group(action, &ctx)?;
        action_groups.insert(name, actions);
    }

    ctx.action_groups = flatten_action_groups_if_no_cycles(action_groups, action_args, &ctx)?;

    // RuleBlocks
    for rule_block in &code.rule_blocks {
        validate_rule_block(rule_block, &ctx)?;
    }


    Ok(())
}

fn validate_tag_group(tagg: &&TagG, ctx: &CompilerContext) -> Result<(String, HashSet<String>), CompilerError> {
    let &TagG(Id(name), IdList(items)) = tagg;
    let mut tags: HashSet<String> = HashSet::new();

    // v3.
    for Id(item) in items {
        if !ctx.ids.tag_names.contains(item) && !ctx.ids.tag_group_names.contains(item) {
            return Err(CompilerError::Undefined(format!("TagGroup {name} member {item} undefined.")));
        }
        tags.insert(item.clone());
    }

    Ok((name.clone(), tags))
}

fn flatten_tag_groups_if_no_cycles(
    tag_groups: HashMap<String, HashSet<String>>,
    ctx: &CompilerContext) 
    -> Result<HashMap<String, HashSet<String>>, CompilerError> {

    // Construct a graph of only tag groups as nodes.
    let mut tag_group_graph: HashMap<String, HashSet<String>> = HashMap::new();
    for (name, tags) in &tag_groups {
        tag_group_graph.insert(name.clone(), HashSet::new());
        for tag in tags {
            if ctx.ids.tag_group_names.contains(tag) {
                tag_group_graph.get_mut(name).map(|tags| tags.insert(tag.clone()));
            }
        }
    }

    // v4.
    if let Some(cycle) = check_cycles(&tag_group_graph) {
        return Err(CompilerError::TagCycle(format!("Cycle in tag groups {cycle:?}.")))
    }

    // s1.
    // Flatten groups
    let mut flat_groups = flatten_graph(&tag_groups);
    // Add mappings for normal tags to themselves.
    for tag in &ctx.ids.tag_names {
        let mut tmp: HashSet<String> = HashSet::new();
        tmp.insert(tag.clone());
        flat_groups.insert(tag.clone(), tmp);
    }

    Ok(flat_groups)
}

fn validate_struct(s: &&Struct, ctx: &CompilerContext) 
    -> Result<(String, HashSet<String>, HashMap<String, String>), CompilerError> {
    
    let &Struct(TitleId(name), tags, fields) = s;
    
    // v5.
    if let Some(IdList(tags)) = tags {
        for Id(tag) in tags {
            if !ctx.tag_mappings.contains_key(tag) {
                return Err(CompilerError::Undefined(format!("Struct {name} tag {tag} undefined.")));
            }
        }
    }

    let mut field_types: HashSet<String> = HashSet::new(); 
    let mut fields_map: HashMap<String, String> = HashMap::new(); 

    if let Some(FieldList(fields)) = fields {
        for Field(Id(fname), t) in fields {
            // v6.
            match t {
                Type::Struct(TitleId(typ)) => { 
                    if !ctx.ids.struct_names.contains(typ) {
                        return Err(CompilerError::Undefined(format!("Struct {name} field {fname} type {typ} undefined.")));
                    }
                },
                Type::Bool | Type::Int | Type::String => {}
            }
            field_types.insert(t.into());
            fields_map.insert(fname.clone(), t.into());
        }
    }

    Ok((name.clone(), field_types, fields_map))
}

fn get_field_type_map_if_no_cycles(
    type_graph: HashMap<String, HashSet<String>>,
    struct_graph: HashMap<String, HashMap<String, String>>,
    ctx: &CompilerContext) 
    -> Result<HashMap<(String, String), String>, CompilerError> {

    // Construct a graph of only structs as nodes.
    let mut struct_type_graph: HashMap<String, HashSet<String>> = HashMap::new();
    for (name, types) in &type_graph {
        struct_type_graph.insert(name.clone(), HashSet::new());
        for typ in types {
            if ctx.ids.struct_names.contains(typ) {
                struct_type_graph.get_mut(name).map(|tags| tags.insert(typ.clone()));
            }
        }
    }

    // v7.
    if let Some(cycle) = check_cycles(&struct_type_graph) {
        return Err(CompilerError::TagCycle(format!("Cycle in struct field type definitions {cycle:?}.")))
    }

    // s2.
    let mut mapping: HashMap<(String, String), String> = HashMap::new();

    for (struct_name, field_map) in &struct_graph {
        for (field_name, field_type) in field_map {
            mapping.insert((struct_name.clone(), field_name.clone()), field_type.clone());
        }
    }

    Ok(mapping)
}

fn validate_action(action: &&Action, ctx: &CompilerContext) -> Result<(String, Vec<String>), CompilerError> {
    let &Action(Id(name), types) = action;
    let mut items: Vec<String> = Vec::new();

    // v8.
    if let Some(TypeList(types)) = types {
        for typ in types {
            match typ {
                Type::Struct(TitleId(typ)) => {
                    if !ctx.types.contains(typ) {
                        return Err(CompilerError::TypeError(format!("Action {name} has argument of unknown type {typ}.")));
                    }
                    items.push(typ.clone());
                }
                Type::Bool | Type::Int | Type::String => {},
            }
            items.push(typ.into());
        }
    }

    Ok((name.clone(), items))
}

fn validate_action_group(action_group: &&ActionG, ctx: &CompilerContext) -> Result<(String, HashSet<String>), CompilerError> {
    let &ActionG(Id(name), IdList(actions)) = action_group;
    let mut items: HashSet<String> = HashSet::new();
    
    // v9.
    for Id(action) in actions {
        if !ctx.ids.action_names.contains(action) && !ctx.ids.action_group_names.contains(action) {
            return Err(CompilerError::Undefined(format!("ActionGroup {name} member {action} undefined.")));
        }
        items.insert(action.clone());
    }

    Ok((name.clone(), items))
}

fn flatten_action_groups_if_no_cycles(
    action_groups: HashMap<String, HashSet<String>>,
    action_args: HashMap<String, Vec<String>>,
    ctx: &CompilerContext) 
    -> Result<HashMap<String, (Vec<String>, HashSet<String>)>, CompilerError> {

    // Construct a graph of only action groups as nodes.
    let mut action_group_graph: HashMap<String, HashSet<String>> = HashMap::new();
    for (name, actions) in &action_groups {
        action_group_graph.insert(name.clone(), HashSet::new());
        for action in actions {
            if ctx.ids.action_group_names.contains(action) {
                action_group_graph.get_mut(name).map(|actions| actions.insert(action.clone()));
            }
        }
    }

    // v10.
    if let Some(cycle) = check_cycles(&action_group_graph) {
        return Err(CompilerError::TagCycle(format!("Cycle in action groups {cycle:?}.")))
    }

    // s4.
    // Flatten groups
    let flat_groups = flatten_graph(&action_groups);
    let mut typed_flat_groups: HashMap<String, (Vec<String>, HashSet<String>)> = HashMap::new();
    
    for (group_name, actions) in flat_groups {
        let mut types: Option<&Vec<String>> = None;
        let mut members: HashSet<String> = HashSet::new();

        for action in actions {
            let this_types = action_args.get(&action).expect("Action should be defined by this point.");

            if let Some(ts) = types {
                // v11.
                if this_types != ts {
                    return Err(CompilerError::TypeError(format!("ActionGroup {group_name} has argument types {this_types:?} but member {action} has argument types {types:?}. ActionGroup members must share the same argument signature.")));
                } else {
                    members.insert(action);
                }
            } else {
                types = Some(this_types);
                members.insert(action);
            }
        }
        typed_flat_groups.insert(
            group_name, 
            (
                types.expect("should be Some because action group should never be empty.")
                    .clone(),
                members
            )
        );
    }

    // Add mappings for normal actions to themselves.
    for action in &ctx.ids.action_names {
        let types = action_args.get(action).expect("Action should be defined by this point.");
        let mut tmp: HashSet<String> = HashSet::new();
        tmp.insert(action.clone());
        typed_flat_groups.insert(action.clone(), (types.clone(), tmp));
    }

    Ok(typed_flat_groups)
}


struct RuleBlockContext {
    args: HashSet<String>,
    arg_types: HashMap<String, String>, // name -> type
    name: String
}
impl RuleBlockContext {
    fn new(name: String) -> Self {
        Self {
            args: HashSet::new(),
            arg_types: HashMap::new(),
            name: name,
        }
    }
}

fn get_field_type(field_list: FieldValue, rule_ctx: &RuleBlockContext, ctx: &CompilerContext) -> Result<String, CompilerError> {
    let FieldValue(parts) = field_list.clone();
    let mut parts = parts.iter();
    let Id(field_name) = parts.next()
                            .expect("FieldValue cannot be empty.");
    let mut field_type = rule_ctx.arg_types.get(field_name)
                                        .expect("Calling get_field_type before populating RuleContextBlock is undefined.")
                                        .clone();

    while let Some(Id(field_name)) = &parts.next() {
        match ctx.field_types.get(&(field_type.clone(), field_name.clone())) {
            Some(v) => { field_type = v.clone(); },
            None => {
                return Err(CompilerError::TypeError(format!("Field {field_list:?} is badly typed. No field {field_name} of type {field_type}")));
            }
        };
    }

    Ok(field_type)
}


fn validate_rule_block(rule_block: &&RuleBlock, ctx: &CompilerContext) -> Result<(), CompilerError> {
    let &RuleBlock(Id(name), args, _, Rules(rules)) = rule_block;
    let mut block_ctx = RuleBlockContext::new(name.clone());

    // v12.
    if !ctx.ids.action_names.contains(name) && !ctx.ids.action_group_names.contains(name) {
        return Err(CompilerError::Undefined(format!("RuleBlock {name} is not an Action or ActionGroup.")));
    }

    let mut arg_types: Vec<String> = Vec::new();
    let ArgList(args) = match args {
        None => &ArgList(Vec::new()),
        Some(a) => a,
    };
    for Arg(typ, Id(name)) in args {
        block_ctx.args.insert(name.clone());
        arg_types.push(typ.into());
        block_ctx.arg_types.insert(name.clone(), typ.into());
    }
    
    let (types, _) = ctx.action_groups.get(name).expect("action_groups should be populated an name should be defined here.");
    
    // v13.
    if &arg_types != types {
        return Err(CompilerError::TypeError(format!("RuleBlock {name} has incorrect argument types {arg_types:?}. They should be {types:?}.")));
    }

    // v14.
    for rule in rules {
        match rule {
            Rule::Allow(c) => {
                validate_condition(c, &block_ctx, &ctx)?;
            },
            Rule::Deny(c) => {
                validate_condition(c, &block_ctx, &ctx)?;
            },
            Rule::Apply(IdList(tags), c) => {
                // v15.
                for Id(tag) in tags {
                    if !ctx.tag_mappings.contains_key(tag) {
                        return Err(CompilerError::Undefined(format!("Apply Rule in {name} contains tag {tag} which is undefined.")));
                    }
                }
                validate_condition(c, &block_ctx, &ctx)?;
            },
        }
    }

    Ok(())
}


// Requirements: BoolExpr must be valid (see validate_bool_expr)
fn validate_condition(cond: &Condition, block_ctx: &RuleBlockContext, ctx: &CompilerContext) -> Result<(), CompilerError> {
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
// v19.     arg types match  
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
fn validate_bool_expr(expr: &BoolExpr, block_ctx: &RuleBlockContext, ctx: &CompilerContext) -> Result<(), CompilerError> {
    match expr {
        // v16.
        BoolExpr::And(a, b) => {
            validate_bool_expr(a.as_ref(), block_ctx, ctx)?;
            validate_bool_expr(b.as_ref(), block_ctx, ctx)?;
            Ok(())
        },
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
            if !ctx.action_groups.contains_key(name) {
                return Err(CompilerError::Undefined(format!("In RuleBlock {},Action {name} not defined for rule allowed condition.", block_ctx.name)));
            }

            // v18.
            if !ctx.ids.rule_block_names.contains(name) {
                return Err(CompilerError::Undefined(format!("In RuleBlock {},Action {name} has no defined RuleBlock.", block_ctx.name)));
            }

            // v19.
            let (types, _) = ctx.action_groups.get(name).expect("name should be a key here.");
            let mut arg_types: Vec<String> = Vec::new();
            let args = match args {
                None => &Vec::new(),
                Some(IdList(v)) => v,
            };
            for Id(i) in args {
                arg_types.push(i.clone());
            }
            if types != &arg_types {
                return Err(CompilerError::TypeError(format!("RuleBlock {} takes args of type {arg_types:?} but Action of this name takes args {types:?}", block_ctx.name)));
            }

            Ok(())
        },
        // v20, v21.
        BoolExpr::Gt(a, b) => {
            if validate_expr(a.as_ref(), block_ctx, ctx)? != ExprType::Int {
                return Err(CompilerError::TypeError(format!("RuleBlock {} condition {a:?} > {b:?} is invalid. {a:?} must be int.", block_ctx.name)));
            }
            if validate_expr(b.as_ref(), block_ctx, ctx)? != ExprType::Int {
                return Err(CompilerError::TypeError(format!("RuleBlock {} condition {a:?} > {b:?} is invalid. {b:?} must be int.", block_ctx.name)));
            }
            Ok(())
        },
        BoolExpr::Lt(a, b) => {
            if validate_expr(a.as_ref(), block_ctx, ctx)? != ExprType::Int {
                return Err(CompilerError::TypeError(format!("RuleBlock {} condition {a:?} < {b:?} is invalid. {a:?} must be int.", block_ctx.name)));
            }
            if validate_expr(b.as_ref(), block_ctx, ctx)? != ExprType::Int {
                return Err(CompilerError::TypeError(format!("RuleBlock {} condition {a:?} < {b:?} is invalid. {b:?} must be int.", block_ctx.name)));
            }
            Ok(())
        },
        BoolExpr::Gte(a, b) => {
            if validate_expr(a.as_ref(), block_ctx, ctx)? != ExprType::Int {
                return Err(CompilerError::TypeError(format!("RuleBlock {} condition {a:?} >= {b:?} is invalid. {a:?} must be int.", block_ctx.name)));
            }
            if validate_expr(b.as_ref(), block_ctx, ctx)? != ExprType::Int {
                return Err(CompilerError::TypeError(format!("RuleBlock {} condition {a:?} >= {b:?} is invalid. {b:?} must be int.", block_ctx.name)));
            }
            Ok(())
        },
        BoolExpr::Lte(a, b) => {
            if validate_expr(a.as_ref(), block_ctx, ctx)? != ExprType::Int {
                return Err(CompilerError::TypeError(format!("RuleBlock {} condition {a:?} <= {b:?} is invalid. {a:?} must be int.", block_ctx.name)));
            }
            if validate_expr(b.as_ref(), block_ctx, ctx)? != ExprType::Int {
                return Err(CompilerError::TypeError(format!("RuleBlock {} condition {a:?} <= {b:?} is invalid. {b:?} must be int.", block_ctx.name)));
            }
            Ok(())
        },
        // v22, v23.
        BoolExpr::Eq(a, b) => {
            let t1 = validate_expr(a.as_ref(), block_ctx, ctx)?;
            let t2 = validate_expr(b.as_ref(), block_ctx, ctx)?;

            if t1 != t2 {
                return Err(CompilerError::TypeError(format!("RuleBlock {} condition {a:?} == {b:?} is invalid. Left and right must be the same type but they are {t1:?}, {t2:?}.", block_ctx.name)));
            }
            Ok(())
        },
        BoolExpr::Neq(a, b) => {
            let t1 = validate_expr(a.as_ref(), block_ctx, ctx)?;
            let t2 = validate_expr(b.as_ref(), block_ctx, ctx)?;

            if t1 != t2 {
                return Err(CompilerError::TypeError(format!("RuleBlock {} condition {a:?} != {b:?} is invalid. Left and right must be the same type but they are {t1:?}, {t2:?}.", block_ctx.name)));
            }
            Ok(())
        },
        BoolExpr::Match(a, b) => {
            // v24, v25, v26.
            if validate_expr(a.as_ref(), block_ctx, ctx)? != ExprType::String {
                return Err(CompilerError::TypeError(format!("RuleBlock {} condition {a:?} matches {b:?} is invalid. {a:?} must be string.", block_ctx.name)));
            }
            if validate_expr(b.as_ref(), block_ctx, ctx)? != ExprType::Regex {
                return Err(CompilerError::TypeError(format!("RuleBlock {} condition {a:?} matches {b:?} is invalid. {b:?} must be regex.", block_ctx.name)));
            }
            Ok(())
        }
        // v27, v28, v29.
        BoolExpr::Contains(a, Id(b)) => {
            let t1 = validate_expr(a.as_ref(), block_ctx, ctx)?;
            
            if t1 != ExprType::TagList && t1 != ExprType::Struct {
                return Err(CompilerError::TypeError(format!("RuleBlock {} condition {a:?} contains {b:?} is invalid. {a:?} must be any_arg, every_arg or a struct type arg.", block_ctx.name)));
            }

            if !ctx.tag_mappings.contains_key(b) {
                return Err(CompilerError::Undefined(format!("RuleBlock {} condition {a:?} contains {b:?} is invalid. {b:?} is an undefined tag.", block_ctx.name)));
            }
            
            Ok(())
        },
        BoolExpr::ContainsAny(a, IdList(tags)) => {
            let t1 = validate_expr(a.as_ref(), block_ctx, ctx)?;
            
            if t1 != ExprType::TagList && t1 != ExprType::Struct {
                return Err(CompilerError::TypeError(format!("RuleBlock {} condition {a:?} contains_any {tags:?} is invalid. {a:?} must be any_arg, every_arg or a struct type arg.", block_ctx.name)));
            }

            for Id(tag) in tags {
                if !ctx.tag_mappings.contains_key(tag) {
                    return Err(CompilerError::Undefined(format!("RuleBlock {} condition {a:?} contains_any {tags:?} is invalid. {tag:?} is an undefined tag.", block_ctx.name)));
                }
            }
            
            Ok(())
        },
        BoolExpr::ContainsAll(a, IdList(tags)) => {
            let t1 = validate_expr(a.as_ref(), block_ctx, ctx)?;
            
            if t1 != ExprType::TagList && t1 != ExprType::Struct {
                return Err(CompilerError::TypeError(format!("RuleBlock {} condition {a:?} contains_all {tags:?} is invalid. {a:?} must be any_arg, every_arg or a struct type arg.", block_ctx.name)));
            }

            for Id(tag) in tags {
                if !ctx.tag_mappings.contains_key(tag) {
                    return Err(CompilerError::Undefined(format!("RuleBlock {} condition {a:?} contains_all {tags:?} is invalid. {tag:?} is an undefined tag.", block_ctx.name)));
                }
            }
            
            Ok(())
        },
        BoolExpr::Lacks(a, Id(b)) => {
            let t1 = validate_expr(a.as_ref(), block_ctx, ctx)?;
            
            if t1 != ExprType::TagList && t1 != ExprType::Struct {
                return Err(CompilerError::TypeError(format!("RuleBlock {} condition {a:?} lacks {b:?} is invalid. {a:?} must be any_arg, every_arg or a struct type arg.", block_ctx.name)));
            }

            if !ctx.tag_mappings.contains_key(b) {
                return Err(CompilerError::Undefined(format!("RuleBlock {} condition {a:?} lacks {b:?} is invalid. {b:?} is an undefined tag.", block_ctx.name)));
            }
            
            Ok(())
        },
        BoolExpr::LacksAny(a, IdList(tags)) => {
            let t1 = validate_expr(a.as_ref(), block_ctx, ctx)?;
            
            if t1 != ExprType::TagList && t1 != ExprType::Struct {
                return Err(CompilerError::TypeError(format!("RuleBlock {} condition {a:?} lacks_any {tags:?} is invalid. {a:?} must be any_arg, every_arg or a struct type arg.", block_ctx.name)));
            }

            for Id(tag) in tags {
                if !ctx.tag_mappings.contains_key(tag) {
                    return Err(CompilerError::Undefined(format!("RuleBlock {} condition {a:?} lacks_any {tags:?} is invalid. {tag:?} is an undefined tag.", block_ctx.name)));
                }
            }
            
            Ok(())
        },
        BoolExpr::LacksAll(a, IdList(tags)) => {
            let t1 = validate_expr(a.as_ref(), block_ctx, ctx)?;
            
            if t1 != ExprType::TagList && t1 != ExprType::Struct {
                return Err(CompilerError::TypeError(format!("RuleBlock {} condition {a:?} lacks_all {tags:?} is invalid. {a:?} must be any_arg, every_arg or a struct type arg.", block_ctx.name)));
            }

            for Id(tag) in tags {
                if !ctx.tag_mappings.contains_key(tag) {
                    return Err(CompilerError::Undefined(format!("RuleBlock {} condition {a:?} lacks_all {tags:?} is invalid. {tag:?} is an undefined tag.", block_ctx.name)));
                }
            }
            
            Ok(())
        },
        BoolExpr::True | BoolExpr::False => Ok(()),

    }    
}

fn validate_expr(expr: &Expr, block_ctx: &RuleBlockContext, ctx: &CompilerContext) -> Result<ExprType, CompilerError> {
    match expr {
        Expr::Add(a, b) => {
            if validate_expr(a.as_ref(), block_ctx, ctx)? != ExprType::Int {
                return Err(CompilerError::TypeError(format!("RuleBlock {} expression {a:?} + {b:?} is invalid. {a:?} must be int.", block_ctx.name)));
            }
            if validate_expr(b.as_ref(), block_ctx, ctx)? != ExprType::Int {
                return Err(CompilerError::TypeError(format!("RuleBlock {} expression {a:?} + {b:?} is invalid. {b:?} must be int.", block_ctx.name)));
            }
            Ok(ExprType::Int)
        },
        Expr::Sub(a, b) => {
            if validate_expr(a.as_ref(), block_ctx, ctx)? != ExprType::Int {
                return Err(CompilerError::TypeError(format!("RuleBlock {} expression {a:?} - {b:?} is invalid. {a:?} must be int.", block_ctx.name)));
            }
            if validate_expr(b.as_ref(), block_ctx, ctx)? != ExprType::Int {
                return Err(CompilerError::TypeError(format!("RuleBlock {} expression {a:?} - {b:?} is invalid. {b:?} must be int.", block_ctx.name)));
            }
            Ok(ExprType::Int)
        },
        Expr::Mul(a, b) => {
            if validate_expr(a.as_ref(), block_ctx, ctx)? != ExprType::Int {
                return Err(CompilerError::TypeError(format!("RuleBlock {} expression {a:?} * {b:?} is invalid. {a:?} must be int.", block_ctx.name)));
            }
            if validate_expr(b.as_ref(), block_ctx, ctx)? != ExprType::Int {
                return Err(CompilerError::TypeError(format!("RuleBlock {} expression {a:?} * {b:?} is invalid. {b:?} must be int.", block_ctx.name)));
            }
            Ok(ExprType::Int)
        },
        Expr::Div(a, b) => {
            if validate_expr(a.as_ref(), block_ctx, ctx)? != ExprType::Int {
                return Err(CompilerError::TypeError(format!("RuleBlock {} expression {a:?} / {b:?} is invalid. {a:?} must be int.", block_ctx.name)));
            }
            if validate_expr(b.as_ref(), block_ctx, ctx)? != ExprType::Int {
                return Err(CompilerError::TypeError(format!("RuleBlock {} expression {a:?} / {b:?} is invalid. {b:?} must be int.", block_ctx.name)));
            }
            Ok(ExprType::Int)
        },
        Expr::Neg(a) => {
            if validate_expr(a.as_ref(), block_ctx, ctx)? != ExprType::Int {
                return Err(CompilerError::TypeError(format!("RuleBlock {} expression -{a:?} is invalid. {a:?} must be int.", block_ctx.name)));
            }
            Ok(ExprType::Int)
        },
        Expr::Num(_) => {
            Ok(ExprType::Int)
        },
        Expr::Field(f) => {
            let t = get_field_type(f.clone(), block_ctx, ctx)?;
            if ctx.ids.struct_names.contains(&t) {
                return Ok(ExprType::Struct);
            } else if t == "int" {
                return Ok(ExprType::Int);
            } else if t == "str" {
                return Ok(ExprType::String);
            } else if t == "bool" {
                return Ok(ExprType::Bool);
            } else {
                panic!("Type of field should be int,bool,str or a struct");
            }
        },
        Expr::String(_) => {
            Ok(ExprType::String)
        },
        Expr::Regex(_) => {
            Ok(ExprType::Regex)
        },
        Expr::AnyArg | Expr::EveryArg => {
            Ok(ExprType::TagList)
        }
    }
}


#[cfg(test)]
mod test {

}


