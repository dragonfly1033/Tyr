use std::collections::HashMap;
use std::env;
use std::fs;
use std::process;
use lalrpop_util::lalrpop_mod;
use regex;
use std::collections::HashSet;

use crate::ast::CodeItemType;
use crate::ast::TitleId;
use crate::ast::{Action, ActionG, Code, CodeItem, Id, RuleBlock, Struct, Tag, TagG};

lalrpop_mod!(
    #[allow(clippy::ptr_arg)]
    #[rustfmt::skip]
    grammar
);

mod ast;
mod check;

#[derive(Debug)]
pub(crate) enum CompilerError {
    #[allow(dead_code)]
    RegexError(regex::Error),
    #[allow(dead_code)]
    AlreadyDefined(String),
    #[allow(dead_code)]
    Undefined(String),
    #[allow(dead_code)]
    TagCycle(String),
    #[allow(dead_code)]
    TypeError(String),
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
fn collect_code(lines: &Vec<CodeItem>) -> Result<(CollectedCode, Identifiers), CompilerError> {
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

fn pre_process(dirty_input: String) -> Result<String, CompilerError> {
    // Remove Comments
    let comments_re = regex::Regex::new(r"//[^\n\r]*[\n\r]*").map_err(|e| CompilerError::RegexError(e))?;
    let clean_input = comments_re.replace_all(&dirty_input, "").into_owned();
    Ok(clean_input)
}

fn compile(file_input: String) {
    let cleaned_input = pre_process(file_input).inspect_err(|e| {
        panic!("Failed to pre-process: {e:?}");
    }).unwrap();

    let code_parser = grammar::CodeParser::new();
    let Code(lines) = code_parser.parse(&cleaned_input).inspect_err(|e| {
        panic!("Failed to parse: {e:?}");
    }).unwrap();

    println!("{lines:?}");

    let (collected_code, identifiers) = collect_code(&lines).inspect_err(|e| {
        panic!("Failed to parse: {e:?}");
    }).unwrap();

    let _ = check::validate_code(&collected_code, identifiers).inspect_err(|e| {
        panic!("Type check failed: {e:?}");
    }).unwrap();


    // let rust_code = translate::transpile(ast);
    // save_as_module(rust_code);
}





fn main() {
    // Collect command-line arguments
    let args: Vec<String> = env::args().collect();

    // Expect exactly one argument: the filename
    if args.len() != 2 {
        usage();
        process::exit(1);
    }

    let filename = &args[1];

    // Read the file contents
    let contents = fs::read_to_string(filename).inspect_err(|e| {
        exit_err(format!("Error reading '{filename}': {e:?}"));
    }).unwrap();

    compile(contents);
}

fn usage() {
    eprintln!("Usage: tyrc <filename>");
}

fn exit_err(msg: String) {
    eprintln!("{msg}");
    process::exit(1);
}