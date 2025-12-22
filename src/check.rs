use crate::{CompilerError, ast::{Action, ActionG, Arg, ArgList, BoolExpr, CodeItem, CodeItemType, Condition, Expr, ExprList, ExprType, Field, FieldList, FieldValue, Id, IdList, Rule, RuleBlock, Rules, Struct, Tag, TagG, TitleId, Type, TypeList}};
use std::{collections::{HashMap, HashSet}, ops::{Deref, DerefMut}};


#[derive(Debug,Default)]
pub(crate) struct Identifiers {
    tag_names: HashSet<String>,
    tag_group_names: HashSet<String>,
    struct_names: HashSet<String>,
    action_names: HashSet<String>,
    action_group_names: HashSet<String>,
    rule_block_names: HashSet<String>,
} 
impl Identifiers {
    fn tag_exists(&self, tag: &String) -> bool {
        self.tag_names.contains(tag) || self.tag_group_names.contains(tag)
    }
    fn struct_exists(&self, s: &String) -> bool {
        self.struct_names.contains(s)
    }
    fn action_exists(&self, action: &String) -> bool {
        self.action_names.contains(action) || self.action_group_names.contains(action)
    }
    fn rule_exists(&self, rule: &String) -> bool {
        self.rule_block_names.contains(rule)
    }
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
                if let Some(cat) = cat_map.get(name) {
                    return Err(CompilerError::AlreadyDefined(format!("Tag {name} already defined as {cat:?} elsewhere.")));
                }
                cat_map.insert(name.clone(), CodeItemType::Tag);
                coll.tags.push(t);
                ids.tag_names.insert(name.clone());
            }
            CodeItem::TagG(t) => {
                let TagG(Id(name), _) = t;
                if let Some(cat) = cat_map.get(name) {
                    return Err(CompilerError::AlreadyDefined(format!("TagG {name} already defined as {cat:?} elsewhere.")));
                }
                cat_map.insert(name.clone(), CodeItemType::TagG);
                coll.tag_groups.push(t);
                ids.tag_group_names.insert(name.clone());
            }
            CodeItem::Struct(s) => {
                let Struct(TitleId(name), ..) = s;
                if let Some(cat) = cat_map.get(name) {
                    return Err(CompilerError::AlreadyDefined(format!("Struct {name} already defined as {cat:?} elsewhere.")));
                }
                cat_map.insert(name.clone(), CodeItemType::Struct);
                coll.structs.push(s);
                ids.struct_names.insert(name.clone());
            }
            CodeItem::Action(a) => {
                let Action(Id(name), _) = a;
                if let Some(cat) = cat_map.get(name) { // safe because rule_blocks aren't added here.
                    return Err(CompilerError::AlreadyDefined(format!("Action {name} already defined as {cat:?} elsewhere.")));
                }
                cat_map.insert(name.clone(), CodeItemType::Action);
                coll.actions.push(a);
                ids.action_names.insert(name.clone());
            }
            CodeItem::ActionG(a) => {
                let ActionG(Id(name), _) = a;
                if let Some(cat) = cat_map.get(name) { // safe because rule_blocks aren't added here.
                    return Err(CompilerError::AlreadyDefined(format!("ActionG {name} already defined as {cat:?} elsewhere.")));
                }
                cat_map.insert(name.clone(), CodeItemType::ActionG);
                coll.action_groups.push(a);
                ids.action_group_names.insert(name.clone());
            }
            CodeItem::RuleBlock(r) => {
                let RuleBlock(Id(name), ..) = r;
                if let Some(cat) = cat_map.get(name) && *cat != CodeItemType::Action && *cat != CodeItemType::ActionG { 
                    return Err(CompilerError::TypeError(format!("RuleBlock {name} defined as {cat:?} not Action or ActionGroup.")));
                }
                if !ids.rule_block_names.insert(name.clone()) {
                    return Err(CompilerError::AlreadyDefined(format!("RuleBlock {name} defined multiple times.")));
                }
                coll.rule_blocks.push(r);
                ids.rule_block_names.insert(name.clone());
            }
        }
    }

    // rule block names are actions/action_groups
    // v12.
    for name in &ids.rule_block_names {
        if !ids.action_exists(name) {
            return Err(CompilerError::Undefined(format!("RuleBlock {name} is not a defined Action or ActionGroup.")));
        }
    }

    Ok((coll, ids))
}

struct NameMap(HashMap<String, HashSet<String>>);
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

        let names:Vec<String> = self.keys().into_iter().map(|s| s.clone()).collect();
            
        let mut visited: HashSet<String> = HashSet::new();
        
        for name in names {
            let mut stack: Vec<(String, Option<String>)> = Vec::new();
            let mut path: HashSet<String> = HashSet::new();
            let mut parent_map: HashMap<String, String> = HashMap::new();
    
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
    
                if let Some(nodes) = self.get(&current) {
                    for node in nodes {
                        stack.push((node.clone(), Some(current.clone())));
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
}

struct ActionMap(HashMap<String, (Vec<Type>, HashSet<String>)>);
impl Deref for ActionMap {
    type Target = HashMap<String, (Vec<Type>, HashSet<String>)>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for ActionMap {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl ActionMap {
    fn new() -> Self {
        ActionMap(HashMap::new())
    }
}

pub(crate) struct GenerationContext {
    tag_mappings: NameMap, // name -> members
    action_mappings: ActionMap, // (name -> (Arg list, members))
}

impl GenerationContext {
    fn new() -> Self {
        Self {
            tag_mappings: NameMap::new(),
            action_mappings: ActionMap::new(),
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
// Side Effects (building CompilerContext, GenerationContext to save work)
// s1. tag_mappings: map ids of Tags and TagGroups to expanded list of Tags
// s2. field_types: map (struct, field) to a type
// s3. types: set of valid types
// s4. action_groups: map ids of Actions and ActionGroups to expanded list of Actions and Type list of args
// s5. action_signatures: map ids of Actions and ActionGroups to Type list of args
pub fn validate_code(code: &CollectedCode, ids: Identifiers) -> Result<GenerationContext, CompilerError> {
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
    if let Some(cycle) = group_to_all_map.group_only(|n| (&ctx).ids.tag_group_names.contains(n)).check_cycles() {
        return Err(CompilerError::Cycle(format!("Cycle in tag groups {cycle:?}.")))
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
    for s in &code.structs {
        let (name, types, fields) = validate_struct(s, &ctx)?;
        let struct_as_type = Type::Struct(TitleId(name.clone()));
        
        type_graph.insert(name.clone(), types);
        
        // s2.
        for (fname, ftype) in fields {
            ctx.field_types.insert((struct_as_type.clone(), fname), ftype);
        }

        // s3.
        ctx.types.insert(struct_as_type);
    }

    // v7. 
    if let Some(cycle) = type_graph.group_only(|n| (&ctx).ids.struct_exists(n)).check_cycles() {
        return Err(CompilerError::Cycle(format!("Cycle in struct field type definitions {cycle:?}.")))
    }

    // Actions --------------------------------------------------------------
    let mut action_arg_types: HashMap<String, Vec<Type>> = HashMap::new();
    for action in &code.actions {
        let (name, types) = validate_action(action, &ctx)?;
        action_arg_types.insert(name.clone(), types.clone());
        // s4. actions only
        let mut tmp: HashSet<String> = HashSet::new();
        tmp.insert(name.clone());
        gen_ctx.action_mappings.insert(name.clone(), (types.clone(), tmp));
        // s5. actions only
        ctx.action_signatures.insert(name.clone(), types.clone());
    } 

    // ActionGroups --------------------------------------------------------------
    let mut group_to_all_map: NameMap = NameMap::new();
    for action in &code.action_groups {
        let (name, actions) = validate_action_group(action, &ctx)?;
        group_to_all_map.insert(name, actions);
    }

    // v10.
    if let Some(cycle) = group_to_all_map.group_only(|n| (&ctx).ids.action_group_names.contains(n)).check_cycles() {
        return Err(CompilerError::Cycle(format!("Cycle in action groups {cycle:?}.")))
    }

    // Flatten groups
    let flat_map = group_to_all_map.flatten_graph();
    
    // v11.
    for (group_name, actions) in &flat_map {
        let mut group_signature: Option<Vec<Type>> = None;

        for action in actions {
            if let Some(this_action_sig) = action_arg_types.get(action) {
                if let Some(group_sig) = group_signature && this_action_sig != &group_sig {
                    return Err(CompilerError::TypeError(format!("ActionGroup {group_name} has argument types {group_sig:?} but member {action} has argument types {this_action_sig:?}. ActionGroup members must share the same argument signature.")));
                } else {
                    group_signature = Some(this_action_sig.clone());
                }
            } else {
                panic!("No action {action} in action_arg_types. All actions should be defined by now.");
            }
        }
        // s4. action_group part (action part done earlier)
        let sig = group_signature.unwrap_or(Vec::new());
        gen_ctx.action_mappings.insert(group_name.clone(), (sig.clone(), actions.clone()));
        // s5. action_group part (action part done earlier)
        ctx.action_signatures.insert(group_name.clone(), sig);
    }

    // RuleBlocks --------------------------------------------------------------
    for rule_block in &code.rule_blocks {
        validate_rule_block(rule_block, &ctx)?;
    }
    // --------------------------------------------------------------------------

    Ok(gen_ctx)
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

fn validate_struct(s: &&Struct, ctx: &CompilerContext) 
    -> Result<(String, HashSet<String>, HashMap<String, Type>), CompilerError> {
    
    let &Struct(TitleId(name), tags, fields) = s;
    
    // v5.
    let IdList(tags) = tags;
    for Id(tag) in tags {
        if !ctx.ids.tag_exists(tag) {
            return Err(CompilerError::Undefined(format!("Struct {name} tag {tag} undefined.")));
        }
    }

    let mut field_types: HashSet<String> = HashSet::new(); 
    let mut fields_map: HashMap<String, Type> = HashMap::new(); 

    let FieldList(fields) = fields;
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
        fields_map.insert(fname.clone(), t.clone());
    }

    Ok((name.clone(), field_types, fields_map))
}

fn validate_action(action: &&Action, ctx: &CompilerContext) -> Result<(String, Vec<Type>), CompilerError> {
    let &Action(Id(name), types) = action;

    let TypeList(types) = types;
    let types = types.clone();

    // v8.
    for typ in &types {
        if !ctx.types.contains(typ) {
            return Err(CompilerError::TypeError(format!("Action {name} has argument of unknown type {typ:?}.")));
        }
    }

    Ok((name.clone(), types))
}

fn validate_action_group(action_group: &&ActionG, ctx: &CompilerContext) -> Result<(String, HashSet<String>), CompilerError> {
    let &ActionG(Id(name), IdList(actions)) = action_group;
    let mut items: HashSet<String> = HashSet::new();
    
    // v9.
    for Id(action) in actions {
        if !ctx.ids.action_exists(action) {
            return Err(CompilerError::Undefined(format!("ActionGroup {name} member {action} undefined.")));
        }
        items.insert(action.clone());
    }

    Ok((name.clone(), items))
}


struct RuleBlockContext {
    arg_types: HashMap<String, Type>, // name -> type
    name: String
}
impl RuleBlockContext {
    fn new(name: String) -> Self {
        Self {
            arg_types: HashMap::new(),
            name: name,
        }
    }
}

fn get_field_type(field_list: FieldValue, rule_ctx: &RuleBlockContext, ctx: &CompilerContext) -> Result<Type, CompilerError> {
    let FieldValue(parts) = field_list.clone();
    let mut parts = parts.iter();
    let Id(field_name) = parts.next()
                            .expect("FieldValue cannot be empty.");
    let Some(field_type) = rule_ctx.arg_types.get(field_name) else {
        return Err(CompilerError::Undefined(format!("RuleBlock {}, field {field_name}is referenced but not defined.", rule_ctx.name)));
    };
    let mut field_type = field_type.clone();

    while let Some(Id(field_name)) = &parts.next() {
        match ctx.field_types.get(&(field_type.clone(), field_name.clone())) {
            Some(v) => { field_type = v.clone(); },
            None => {
                return Err(CompilerError::TypeError(format!("Field {field_list:?} is badly typed. No field {field_name} of type {field_type:?}")));
            }
        };
    }

    Ok(field_type)
}


fn validate_rule_block(rule_block: &&RuleBlock, ctx: &CompilerContext) -> Result<(), CompilerError> {
    let &RuleBlock(Id(name), args, _, Rules(rules)) = rule_block;
    let mut block_ctx = RuleBlockContext::new(name.clone());

    let ArgList(args) = args;

    let mut arg_types: Vec<Type> = Vec::new();

    for Arg(typ, Id(name)) in args {
        arg_types.push(typ.clone());
        block_ctx.arg_types.insert(name.clone(), typ.clone());
    }
    
    let types = ctx.action_signatures.get(name).expect("action_signatures should be populated and name should be defined here.");
    
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
                    if !ctx.ids.tag_exists(tag) {
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
            if !ctx.ids.action_exists(name) {
                return Err(CompilerError::Undefined(format!("In RuleBlock {}, Action {name} not defined for \"<rule> allowed\" condition.", block_ctx.name)));
            }

            // v18.
            if !ctx.ids.rule_exists(name) {
                return Err(CompilerError::Undefined(format!("In RuleBlock {}, Action {name} exists but has no defined RuleBlock.", block_ctx.name)));
            }

            let action_types = ctx.action_signatures.get(name).expect("name should be a key here.");
            let mut these_arg_types: Vec<Type> = Vec::new();
            let ExprList(args) = args;

            // v19.
            for expr in args {
                these_arg_types.push(
                    validate_expr(expr, block_ctx, ctx)?.try_into()?
                );
            }
            // v19.1
            if action_types != &these_arg_types {
                return Err(CompilerError::TypeError(format!("RuleBlock {} takes args of type {these_arg_types:?} but Action of this name takes args {action_types:?}", block_ctx.name)));
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
            
            if t1 != ExprType::TagList && matches!(t1, ExprType::Struct(_)) {
                return Err(CompilerError::TypeError(format!("RuleBlock {} condition {a:?} contains {b:?} is invalid. {a:?} must be any_arg, every_arg or a struct type arg.", block_ctx.name)));
            }

            if !ctx.ids.tag_exists(b) {
                return Err(CompilerError::Undefined(format!("RuleBlock {} condition {a:?} contains {b:?} is invalid. {b:?} is an undefined tag.", block_ctx.name)));
            }
            
            Ok(())
        },
        BoolExpr::ContainsAny(a, IdList(tags)) => {
            let t1 = validate_expr(a.as_ref(), block_ctx, ctx)?;
            
            if t1 != ExprType::TagList && matches!(t1, ExprType::Struct(_)) {
                return Err(CompilerError::TypeError(format!("RuleBlock {} condition {a:?} contains_any {tags:?} is invalid. {a:?} must be any_arg, every_arg or a struct type arg.", block_ctx.name)));
            }

            for Id(tag) in tags {
                if !ctx.ids.tag_exists(tag) {
                    return Err(CompilerError::Undefined(format!("RuleBlock {} condition {a:?} contains_any {tags:?} is invalid. {tag:?} is an undefined tag.", block_ctx.name)));
                }
            }
            
            Ok(())
        },
        BoolExpr::ContainsAll(a, IdList(tags)) => {
            let t1 = validate_expr(a.as_ref(), block_ctx, ctx)?;
            
            if t1 != ExprType::TagList && matches!(t1, ExprType::Struct(_)) {
                return Err(CompilerError::TypeError(format!("RuleBlock {} condition {a:?} contains_all {tags:?} is invalid. {a:?} must be any_arg, every_arg or a struct type arg.", block_ctx.name)));
            }

            for Id(tag) in tags {
                if !ctx.ids.tag_exists(tag) {
                    return Err(CompilerError::Undefined(format!("RuleBlock {} condition {a:?} contains_all {tags:?} is invalid. {tag:?} is an undefined tag.", block_ctx.name)));
                }
            }
            
            Ok(())
        },
        BoolExpr::Lacks(a, Id(b)) => {
            let t1 = validate_expr(a.as_ref(), block_ctx, ctx)?;
            
            if t1 != ExprType::TagList && matches!(t1, ExprType::Struct(_)) {
                return Err(CompilerError::TypeError(format!("RuleBlock {} condition {a:?} lacks {b:?} is invalid. {a:?} must be any_arg, every_arg or a struct type arg.", block_ctx.name)));
            }

            if !ctx.ids.tag_exists(b) {
                return Err(CompilerError::Undefined(format!("RuleBlock {} condition {a:?} lacks {b:?} is invalid. {b:?} is an undefined tag.", block_ctx.name)));
            }
            
            Ok(())
        },
        BoolExpr::LacksAny(a, IdList(tags)) => {
            let t1 = validate_expr(a.as_ref(), block_ctx, ctx)?;
            
            if t1 != ExprType::TagList && matches!(t1, ExprType::Struct(_)) {
                return Err(CompilerError::TypeError(format!("RuleBlock {} condition {a:?} lacks_any {tags:?} is invalid. {a:?} must be any_arg, every_arg or a struct type arg.", block_ctx.name)));
            }

            for Id(tag) in tags {
                if !ctx.ids.tag_exists(tag) {
                    return Err(CompilerError::Undefined(format!("RuleBlock {} condition {a:?} lacks_any {tags:?} is invalid. {tag:?} is an undefined tag.", block_ctx.name)));
                }
            }
            
            Ok(())
        },
        BoolExpr::LacksAll(a, IdList(tags)) => {
            let t1 = validate_expr(a.as_ref(), block_ctx, ctx)?;
            
            if t1 != ExprType::TagList && matches!(t1, ExprType::Struct(_)) {
                return Err(CompilerError::TypeError(format!("RuleBlock {} condition {a:?} lacks_all {tags:?} is invalid. {a:?} must be any_arg, every_arg or a struct type arg.", block_ctx.name)));
            }

            for Id(tag) in tags {
                if !ctx.ids.tag_exists(tag) {
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
            let t: ExprType = get_field_type(f.clone(), block_ctx, ctx)?.into();
            match &t {
                ExprType::Bool | ExprType::String | ExprType::Int => Ok(t),
                ExprType::Struct(s) => {
                    if ctx.ids.struct_exists(s) {
                        Ok(t)
                    } else {
                        Err(CompilerError::Undefined(format!("RuleBlock {}, field expression {f:?} of type Struct({s}) is undefined.", block_ctx.name)))
                    }
                }
                ExprType::Regex | ExprType::TagList => panic!("This shouldn't happen from Type::<ExprType>::into")
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


