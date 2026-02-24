use std::collections::HashMap;

use crate::{
    ast::{self, Fallback, FieldList, Type},
    check::GenerationContext,
    collect::CollectedCode,
    ir,
};

fn expand_tag(tag: &String, ctx: &GenerationContext) -> Vec<ir::Tag> {
    ctx.tag_mappings
        .get(tag)
        .expect("Tag should exist")
        .iter()
        .map(|s| ir::Tag(s.clone()))
        .collect()
}

fn collect_arg_names(fields: &ast::FieldList) -> Vec<ir::Id> {
    let ast::FieldList(fields) = fields;
    fields
        .iter()
        .map(|ast::Field(ast::Id(id), _)| ir::Id(id.clone()))
        .collect()
}

pub fn compile_ir(code: &CollectedCode, ctx: GenerationContext) -> ir::Code {
    let tags = compile_tags(&code.tags);
    let structs = compile_structs(&code.structs, &ctx);
    let (regexes, tag_sets, action_rules) = compile_actions(&code.rule_blocks, &ctx);

    ir::Code {
        regexes,
        tags,
        tag_sets,
        structs,
        action_rules,
    }
}

fn compile_tags(tags: &Vec<&ast::Tag>) -> Vec<ir::Tag> {
    tags.iter()
        .map(|ast::Tag(ast::Id(t))| ir::Tag(t.clone()))
        .collect()
}

fn compile_tags_from_ids(tags: &Vec<ast::Id>, ctx: &GenerationContext) -> Vec<ir::Tag> {
    tags.iter()
        .flat_map(|ast::Id(t)| expand_tag(t, ctx))
        .collect()
}

fn compile_structs(structs: &Vec<&ast::Struct>, ctx: &GenerationContext) -> Vec<ir::Struct> {
    structs
        .iter()
        // We ignore the tags because we use the pre-calculated expanded intrinsics
        .map(|ast::Struct(ast::TitleId(name), _, fields)| {
            let tags: Vec<ast::Id> = ctx
                .struct_intrinsics
                .get(name)
                .expect("Struct should exist")
                .iter()
                .map(|t| ast::Id(t.clone()))
                .collect();
            ir::Struct(
                ir::StructName(name.clone()),
                ir::TagList(compile_tags_from_ids(&tags, ctx)),
                fields.clone(),
            )
        })
        .collect()
}

fn compile_actions(
    blocks: &Vec<&ast::RuleBlock>,
    ctx: &GenerationContext,
) -> (Vec<ir::Regex>, Vec<Vec<ir::Tag>>, Vec<ir::ActionRules>) {
    let mut regexes: Vec<ir::Regex> = Vec::new();
    let mut tag_sets: Vec<Vec<ir::Tag>> = Vec::new();

    let mut action_rules_info: HashMap<
        String,
        (
            FieldList,
            Type,
            Fallback,
            Vec<ir::Condition>,
            Vec<ir::LabelledCondition>,
            ir::Applications,
        ),
    > = HashMap::new();

    for ast::RuleBlock(ast::Id(name), fields, ast::Rules(rules)) in blocks {
        let args = collect_arg_names(fields);

        let (allow_conds, deny_conds, appls, mut regs, mut tags) = compile_rules(&rules, &args, ctx);

        regexes.append(&mut regs);
        tag_sets.append(&mut tags);

        if let Some(actions) = ctx.action_mappings.get(name) {
            for action in actions {
                if let Some((_, _, _, allows, denys, applications)) = action_rules_info.get_mut(action)
                {
                    allows.append(&mut allow_conds.clone());
                    denys.append(&mut deny_conds.clone());
                    applications.join(&mut appls.clone());
                } else {
                    action_rules_info.insert(
                        action.clone(),
                        (
                            fields.clone(),
                            ctx.action_return.get(action).expect("Action {action} should have a known return value").clone(),
                            ctx.action_fallback.get(action).expect("Action {action} should have a known fallback value").clone(),
                            allow_conds.clone(),
                            deny_conds.clone(),
                            appls.clone(),
                        ),
                    );
                }
            }
        }
    }

    let mut action_rules: Vec<ir::ActionRules> = Vec::new();

    for (name, (fields, ret, fallback, allows, denys, applications)) in action_rules_info {
        action_rules.push(ir::ActionRules(
            ir::ActionName(name),
            fields,
            ret,
            fallback,
            allows,
            denys,
            applications,
        ));
    }

    (regexes, tag_sets, action_rules)
}

fn compile_rules(
    rules: &Vec<ast::Rule>,
    args: &Vec<ir::Id>,
    ctx: &GenerationContext,
) -> (
    Vec<ir::Condition>,
    Vec<ir::LabelledCondition>,
    ir::Applications,
    Vec<ir::Regex>,
    Vec<Vec<ir::Tag>>,
) {
    let mut allows: Vec<ir::Condition> = Vec::new();
    let mut denys: Vec<ir::LabelledCondition> = Vec::new();
    let mut applications: Vec<(ir::TagList, ir::Condition)> = Vec::new();

    let mut regexes: Vec<ir::Regex> = Vec::new();
    let mut tag_sets: Vec<Vec<ir::Tag>> = Vec::new();

    for rule in rules {
        match rule {
            ast::Rule::Allow(c) => {
                allows.push(compile_condition(c, &mut regexes, &mut tag_sets, args, ctx));
            }
            ast::Rule::Deny(c) => {
                denys.push(ir::LabelledCondition {
                    label: c.to_string(),
                    condition: compile_condition(c, &mut regexes, &mut tag_sets, args, ctx),
                });
            }
            ast::Rule::Apply(ast::IdList(tags), c) => {
                applications.push((
                    ir::TagList(compile_tags_from_ids(tags, ctx)),
                    compile_condition(c, &mut regexes, &mut tag_sets, args, ctx),
                ));
            }
        }
    }

    (
        allows,
        denys,
        ir::Applications(applications),
        regexes,
        tag_sets,
    )
}

fn compile_condition(
    cond: &ast::Condition,
    regexes: &mut Vec<ir::Regex>,
    tag_sets: &mut Vec<Vec<ir::Tag>>,
    args: &Vec<ir::Id>,
    ctx: &GenerationContext,
) -> ir::Condition {
    match cond {
        ast::Condition::Always => ir::Condition::Always,
        ast::Condition::Never => ir::Condition::Never,
        ast::Condition::When(e) => {
            ir::Condition::When(compile_bool_expr(e, regexes, tag_sets, args, ctx))
        }
    }
}

fn compile_bool_expr(
    expr: &Box<ast::BoolExpr>,
    regexes: &mut Vec<ir::Regex>,
    tag_sets: &mut Vec<Vec<ir::Tag>>,
    args: &Vec<ir::Id>,
    ctx: &GenerationContext,
) -> Box<ir::BoolExpr> {
    match expr.as_ref() {
        ast::BoolExpr::Or(a, b) => Box::new(ir::BoolExpr::Or(
            compile_bool_expr(a, regexes, tag_sets, args, ctx),
            compile_bool_expr(b, regexes, tag_sets, args, ctx),
        )),
        ast::BoolExpr::And(a, b) => Box::new(ir::BoolExpr::And(
            compile_bool_expr(a, regexes, tag_sets, args, ctx),
            compile_bool_expr(b, regexes, tag_sets, args, ctx),
        )),
        ast::BoolExpr::Not(a) => Box::new(ir::BoolExpr::Not(compile_bool_expr(
            a, regexes, tag_sets, args, ctx,
        ))),
        ast::BoolExpr::Rule(ast::Id(id), args) => Box::new(ir::BoolExpr::Rule(
            ir::Id(id.clone()),
            compile_expr_list(args),
        )),
        ast::BoolExpr::Gt(a, b) => {
            Box::new(ir::BoolExpr::Gt(compile_math_expr(a), compile_math_expr(b)))
        }
        ast::BoolExpr::Lt(a, b) => {
            Box::new(ir::BoolExpr::Lt(compile_math_expr(a), compile_math_expr(b)))
        }
        ast::BoolExpr::Gte(a, b) => Box::new(ir::BoolExpr::Gte(
            compile_math_expr(a),
            compile_math_expr(b),
        )),
        ast::BoolExpr::Lte(a, b) => Box::new(ir::BoolExpr::Lte(
            compile_math_expr(a),
            compile_math_expr(b),
        )),
        ast::BoolExpr::Eq(a, b) => Box::new(ir::BoolExpr::Eq(compile_expr(a), compile_expr(b))),
        ast::BoolExpr::Neq(a, b) => Box::new(ir::BoolExpr::Neq(compile_expr(a), compile_expr(b))),
        ast::BoolExpr::Match(s, r) => Box::new(ir::BoolExpr::Match(
            compile_string_expr(s),
            compile_regex_id(r, regexes),
        )),
        ast::BoolExpr::Contains(e, ast::Id(tag)) => {
            // We know tag is not a group from type checking
            Box::new(ir::BoolExpr::Contains(
                compile_tag_expr(e, args),
                ir::Tag(tag.clone()),
            ))
        }
        ast::BoolExpr::ContainsAll(e, ast::IdList(tags)) => {
            tag_sets.push(compile_tags_from_ids(tags, ctx));
            Box::new(ir::BoolExpr::ContainsAll(
                compile_tag_expr(e, args),
                ir::TagSetId(tag_sets.len() - 1),
            ))
        }
        ast::BoolExpr::ContainsAny(e, ast::IdList(tags)) => {
            tag_sets.push(compile_tags_from_ids(tags, ctx));
            Box::new(ir::BoolExpr::ContainsAny(
                compile_tag_expr(e, args),
                ir::TagSetId(tag_sets.len() - 1),
            ))
        }
        ast::BoolExpr::Lacks(e, ast::Id(tag)) => {
            // We know tag is not a group from type checking
            Box::new(ir::BoolExpr::Lacks(
                compile_tag_expr(e, args),
                ir::Tag(tag.clone()),
            ))
        }
        ast::BoolExpr::LacksAll(e, ast::IdList(tags)) => {
            tag_sets.push(compile_tags_from_ids(tags, ctx));
            Box::new(ir::BoolExpr::LacksAll(
                compile_tag_expr(e, args),
                ir::TagSetId(tag_sets.len() - 1),
            ))
        }
        ast::BoolExpr::LacksAny(e, ast::IdList(tags)) => {
            tag_sets.push(compile_tags_from_ids(tags, ctx));
            Box::new(ir::BoolExpr::LacksAny(
                compile_tag_expr(e, args),
                ir::TagSetId(tag_sets.len() - 1),
            ))
        }
        ast::BoolExpr::True => Box::new(ir::BoolExpr::True),
        ast::BoolExpr::False => Box::new(ir::BoolExpr::False),
    }
}

fn compile_math_expr(expr: &ast::Expr) -> Box<ir::MathExpr> {
    match expr {
        ast::Expr::Add(a, b) => Box::new(ir::MathExpr::Add(
            compile_math_expr(&a),
            compile_math_expr(&b),
        )),
        ast::Expr::Sub(a, b) => Box::new(ir::MathExpr::Sub(
            compile_math_expr(&a),
            compile_math_expr(&b),
        )),
        ast::Expr::Mul(a, b) => Box::new(ir::MathExpr::Mul(
            compile_math_expr(&a),
            compile_math_expr(&b),
        )),
        ast::Expr::Div(a, b) => Box::new(ir::MathExpr::Div(
            compile_math_expr(&a),
            compile_math_expr(&b),
        )),
        ast::Expr::Neg(a) => Box::new(ir::MathExpr::Neg(compile_math_expr(&a))),
        ast::Expr::Num(a) => Box::new(ir::MathExpr::Num(*a)),
        ast::Expr::Field(a) => Box::new(ir::MathExpr::Field(a.clone())),
        ast::Expr::String(_) | ast::Expr::Regex(_) | ast::Expr::AnyArg | ast::Expr::EveryArg => {
            panic!("AST should have been type checked to prevent this.")
        }
    }
}
fn compile_string_expr(expr: &ast::Expr) -> ir::StringExpr {
    match expr {
        ast::Expr::String(a) => ir::StringExpr::String(a.clone()),
        ast::Expr::Field(a) => ir::StringExpr::Field(a.clone()),
        ast::Expr::Add(_, _)
        | ast::Expr::Sub(_, _)
        | ast::Expr::Mul(_, _)
        | ast::Expr::Div(_, _)
        | ast::Expr::Neg(_)
        | ast::Expr::Num(_)
        | ast::Expr::Regex(_)
        | ast::Expr::AnyArg
        | ast::Expr::EveryArg => panic!("AST should have been type checked to prevent this."),
    }
}
fn compile_expr(expr: &ast::Expr) -> ir::Expr {
    match expr {
        ast::Expr::Field(f) => ir::Expr::Field(f.clone()),
        ast::Expr::String(_) => ir::Expr::String(compile_string_expr(expr)),
        ast::Expr::Add(_, _) => ir::Expr::Maths(compile_math_expr(expr)),
        ast::Expr::Sub(_, _) => ir::Expr::Maths(compile_math_expr(expr)),
        ast::Expr::Mul(_, _) => ir::Expr::Maths(compile_math_expr(expr)),
        ast::Expr::Div(_, _) => ir::Expr::Maths(compile_math_expr(expr)),
        ast::Expr::Neg(_) => ir::Expr::Maths(compile_math_expr(expr)),
        ast::Expr::Num(_) => ir::Expr::Maths(compile_math_expr(expr)),
        ast::Expr::Regex(_) | ast::Expr::AnyArg | ast::Expr::EveryArg => {
            panic!("AST should have been type checked to prevent this.")
        }
    }
}
fn compile_expr_list(expr_list: &ast::ExprList) -> ir::ExprList {
    let ast::ExprList(exprs) = expr_list;
    let mut expr_list: Vec<ir::Expr> = Vec::new();

    for expr in exprs {
        expr_list.push(compile_expr(expr));
    }

    ir::ExprList(expr_list)
}
fn compile_regex_id(reg: &ast::Expr, regexes: &mut Vec<ir::Regex>) -> ir::RegexId {
    match reg {
        ast::Expr::Regex(r) => {
            regexes.push(ir::Regex(r.clone()));
            ir::RegexId(regexes.len() - 1)
        }
        ast::Expr::Field(_)
        | ast::Expr::String(_)
        | ast::Expr::Add(_, _)
        | ast::Expr::Sub(_, _)
        | ast::Expr::Mul(_, _)
        | ast::Expr::Div(_, _)
        | ast::Expr::Neg(_)
        | ast::Expr::Num(_)
        | ast::Expr::AnyArg
        | ast::Expr::EveryArg => panic!("AST should have been type checked to prevent this."),
    }
}
fn compile_tag_expr(expr: &ast::Expr, args: &Vec<ir::Id>) -> ir::TagExpr {
    match expr {
        ast::Expr::AnyArg => ir::TagExpr::Any(args.clone()),
        ast::Expr::EveryArg => ir::TagExpr::Every(args.clone()),
        ast::Expr::Field(f) => ir::TagExpr::Field(f.clone()),
        ast::Expr::Regex(_)
        | ast::Expr::String(_)
        | ast::Expr::Add(_, _)
        | ast::Expr::Sub(_, _)
        | ast::Expr::Mul(_, _)
        | ast::Expr::Div(_, _)
        | ast::Expr::Neg(_)
        | ast::Expr::Num(_) => panic!("AST should have been type checked to prevent this."),
    }
}
