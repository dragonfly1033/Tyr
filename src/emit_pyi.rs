use crate::{
    ast::{Field, FieldList, TitleId, Type},
    ir::{ActionName, ActionRules, Code, Struct, StructName, Tag},
};

fn py_type(typ: &Type) -> &str {
    match typ {
        Type::Bool => "bool",
        Type::Int => "int",
        Type::String => "str",
        Type::Struct(TitleId(s)) => s.as_str(),
    }
}

fn emit_struct(s: &Struct) -> String {
    let Struct(StructName(name), _, FieldList(fields)) = s;

    let fields_str: Vec<String> = fields
        .iter()
        .map(|Field(crate::ast::Id(n), typ)| format!("    {}: {}", n, py_type(typ)))
        .collect();

    let ctor_params: Vec<String> = fields
        .iter()
        .map(|Field(crate::ast::Id(n), typ)| format!("{}: {}", n, py_type(typ)))
        .collect();

    let ctor_params_with_tags = if ctor_params.is_empty() {
        "tags: list[Tag] | None = None".to_string()
    } else {
        format!("{}, tags: list[Tag] | None = None", ctor_params.join(", "))
    };

    let body = if fields_str.is_empty() {
        "    ...".to_string()
    } else {
        fields_str.join("\n")
    };

    format!(
        "class {name}:\n{body}\n    def __init__(self, {ctor_params_with_tags}) -> None: ...\n"
    )
}

fn emit_action(action: &ActionRules) -> String {
    let ActionRules(ActionName(name), FieldList(fields), ret, _, _, _, _) = action;

    // Parameters of the wrapped callable
    let param_types: Vec<&str> = fields.iter().map(|Field(_, typ)| py_type(typ)).collect();
    let ret_type = py_type(ret);

    // The callable type the decorator accepts and returns
    let callable = if param_types.is_empty() {
        format!("Callable[[], {ret_type}]")
    } else {
        format!("Callable[[{}], {ret_type}]", param_types.join(", "))
    };

    format!("def try_{name}(func: {callable}) -> {callable}: ...\n")
}

pub fn emit_pyi(code: &Code) -> String {
    let mut out = String::new();

    out.push_str("from __future__ import annotations\n");
    out.push_str("import enum\n");
    out.push_str("from collections.abc import Callable\n");
    out.push('\n');

    // Exceptions
    out.push_str("class PolicyDenied(Exception): ...\n");
    out.push_str("class PolicyWarned(Exception): ...\n");
    out.push('\n');

    // Tag enum — #[repr(usize)] so values are 0, 1, 2, ...
    out.push_str("class Tag(enum.IntEnum):\n");
    if code.tags.is_empty() {
        out.push_str("    ...\n");
    } else {
        for (i, Tag(name)) in code.tags.iter().enumerate() {
            out.push_str(&format!("    {} = {}\n", name, i));
        }
    }
    out.push('\n');

    // Structs
    for s in &code.structs {
        out.push_str(&emit_struct(s));
        out.push('\n');
    }

    // Decorator functions
    for action in &code.action_rules {
        out.push_str(&emit_action(action));
    }

    out
}
