use crate::{
    ast::{Action, ActionG, CodeItem, CodeItemType, Id, RuleBlock, Struct, Tag, TagG, TitleId},
    CompilerError,
};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Default)]
pub(crate) struct Identifiers {
    pub(crate) tag_names: HashSet<String>,
    pub(crate) tag_group_names: HashSet<String>,
    pub(crate) struct_names: HashSet<String>,
    pub(crate) action_names: HashSet<String>,
    pub(crate) action_group_names: HashSet<String>,
    pub(crate) rule_block_names: HashSet<String>,
}
impl Identifiers {
    pub(crate) fn tag_exists(&self, tag: &String) -> bool {
        self.tag_names.contains(tag) || self.tag_group_names.contains(tag)
    }
    pub(crate) fn struct_exists(&self, s: &String) -> bool {
        self.struct_names.contains(s)
    }
    pub(crate) fn action_exists(&self, action: &String) -> bool {
        self.action_names.contains(action) || self.action_group_names.contains(action)
    }
    pub(crate) fn rule_exists(&self, rule: &String) -> bool {
        self.rule_block_names.contains(rule)
    }
}

#[derive(Debug, Default)]
pub(crate) struct CollectedCode<'a> {
    pub(crate) tags: Vec<&'a Tag>,
    pub(crate) tag_groups: Vec<&'a TagG>,
    pub(crate) structs: Vec<&'a Struct>,
    pub(crate) actions: Vec<&'a Action>,
    pub(crate) action_groups: Vec<&'a ActionG>,
    pub(crate) rule_blocks: Vec<&'a RuleBlock>,
}

/// Collect code into statement categories
/// Output should ensure
///     - identifiers are not used in separate categories
///     - no re-definitions
pub(crate) fn collect_code(
    lines: &Vec<CodeItem>,
) -> Result<(CollectedCode, Identifiers), CompilerError> {
    let mut coll = CollectedCode::default();
    let mut ids = Identifiers::default();

    let mut cat_map: HashMap<String, CodeItemType> = HashMap::new();

    for item in lines {
        match item {
            CodeItem::Tag(t) => {
                let Tag(Id(name)) = t;
                if let Some(cat) = cat_map.get(name) {
                    return Err(CompilerError::AlreadyDefined(format!(
                        "Tag {name} already defined as {cat:?} elsewhere."
                    )));
                }
                cat_map.insert(name.clone(), CodeItemType::Tag);
                coll.tags.push(t);
                ids.tag_names.insert(name.clone());
            }
            CodeItem::TagG(t) => {
                let TagG(Id(name), _) = t;
                if let Some(cat) = cat_map.get(name) {
                    return Err(CompilerError::AlreadyDefined(format!(
                        "TagG {name} already defined as {cat:?} elsewhere."
                    )));
                }
                cat_map.insert(name.clone(), CodeItemType::TagG);
                coll.tag_groups.push(t);
                ids.tag_group_names.insert(name.clone());
            }
            CodeItem::Struct(s) => {
                let Struct(TitleId(name), ..) = s;
                if let Some(cat) = cat_map.get(name) {
                    return Err(CompilerError::AlreadyDefined(format!(
                        "Struct {name} already defined as {cat:?} elsewhere."
                    )));
                }
                cat_map.insert(name.clone(), CodeItemType::Struct);
                coll.structs.push(s);
                ids.struct_names.insert(name.clone());
            }
            CodeItem::Action(a) => {
                let Action(Id(name), ..) = a;
                if let Some(cat) = cat_map.get(name) {
                    // safe because rule_blocks aren't added here.
                    return Err(CompilerError::AlreadyDefined(format!(
                        "Action {name} already defined as {cat:?} elsewhere."
                    )));
                }
                cat_map.insert(name.clone(), CodeItemType::Action);
                coll.actions.push(a);
                ids.action_names.insert(name.clone());
            }
            CodeItem::ActionG(a) => {
                let ActionG(Id(name), _) = a;
                if let Some(cat) = cat_map.get(name) {
                    // safe because rule_blocks aren't added here.
                    return Err(CompilerError::AlreadyDefined(format!(
                        "ActionG {name} already defined as {cat:?} elsewhere."
                    )));
                }
                cat_map.insert(name.clone(), CodeItemType::ActionG);
                coll.action_groups.push(a);
                ids.action_group_names.insert(name.clone());
            }
            CodeItem::RuleBlock(r) => {
                let RuleBlock(Id(name), ..) = r;
                if let Some(cat) = cat_map.get(name)
                    && *cat != CodeItemType::Action
                    && *cat != CodeItemType::ActionG
                {
                    return Err(CompilerError::TypeError(format!(
                        "RuleBlock {name} defined as {cat:?} not Action or ActionGroup."
                    )));
                }
                if !ids.rule_block_names.insert(name.clone()) {
                    return Err(CompilerError::AlreadyDefined(format!(
                        "RuleBlock {name} defined multiple times."
                    )));
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
            return Err(CompilerError::Undefined(format!(
                "RuleBlock {name} is not a defined Action or ActionGroup."
            )));
        }
    }

    Ok((coll, ids))
}
