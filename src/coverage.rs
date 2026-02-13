use std::collections::HashMap;
use std::fmt;

use z3::ast::Ast;
use z3::{self, SatResult};

use crate::ast::{Fallback, FieldValue, Id};
use crate::ir::{
    self, ActionName, ActionRules, BoolExpr, Code, Condition, Expr, MathExpr, Tag, TagExpr, TagSetId
};


/// Creates a Z3 solver with the specified timeout in milliseconds.
fn create_solver_with_timeout(timeout_ms: u32) -> z3::Solver {
    let solver = z3::Solver::new();
    let mut params = z3::Params::new();
    params.set_u32("timeout", timeout_ms);
    solver.set_params(&params);
    solver
}

struct CoverageCtx<'a> {
    code: &'a Code,
    int_vars: HashMap<String, z3::ast::Int>,
    bool_vars: HashMap<String, z3::ast::Bool>,
    opaque_counter: usize,
}

impl<'a> CoverageCtx<'a> {
    fn new(code: &'a Code) -> Self {
        Self {
            code,
            int_vars: HashMap::new(),
            bool_vars: HashMap::new(),
            opaque_counter: 0,
        }
    }

    fn int_var(&mut self, name: &str) -> z3::ast::Int {
        if let Some(v) = self.int_vars.get(name) {
            v.clone()
        } else {
            let v = z3::ast::Int::new_const(name);
            self.int_vars.insert(name.to_string(), v.clone());
            v
        }
    }

    fn bool_var(&mut self, name: &str) -> z3::ast::Bool {
        if let Some(v) = self.bool_vars.get(name) {
            v.clone()
        } else {
            let v = z3::ast::Bool::new_const(name);
            self.bool_vars.insert(name.to_string(), v.clone());
            v
        }
    }

    fn fresh_opaque(&mut self, hint: &str) -> z3::ast::Bool {
        let name = format!("opaque_{hint}_{}", self.opaque_counter);
        self.opaque_counter += 1;
        z3::ast::Bool::new_const(name.as_str())
    }

    fn field_path(fv: &FieldValue) -> String {
        let FieldValue(ids) = fv;
        ids.iter()
            .map(|Id(s)| s.as_str())
            .collect::<Vec<_>>()
            .join(".")
    }

    fn tag_field_var(&mut self, field_path: &str, tag: &Tag) -> z3::ast::Bool {
        let Tag(t) = tag;
        let name = format!("{field_path}#{t}");
        self.bool_var(&name)
    }

    fn translate_math_expr(&mut self, expr: &MathExpr) -> z3::ast::Int {
        match expr {
            MathExpr::Num(n) => z3::ast::Int::from_i64(*n as i64),
            MathExpr::Field(fv) => {
                let path = Self::field_path(fv);
                self.int_var(&path)
            }
            MathExpr::Add(a, b) => {
                let a = self.translate_math_expr(a);
                let b = self.translate_math_expr(b);
                z3::ast::Int::add(&[&a, &b])
            }
            MathExpr::Sub(a, b) => {
                let a = self.translate_math_expr(a);
                let b = self.translate_math_expr(b);
                z3::ast::Int::sub(&[&a, &b])
            }
            MathExpr::Mul(a, b) => {
                let a = self.translate_math_expr(a);
                let b = self.translate_math_expr(b);
                z3::ast::Int::mul(&[&a, &b])
            }
            MathExpr::Div(a, b) => {
                let a = self.translate_math_expr(a);
                let b = self.translate_math_expr(b);
                a.div(&b)
            }
            MathExpr::Neg(a) => {
                let a = self.translate_math_expr(a);
                a.unary_minus()
            }
        }
    }

    fn translate_tag_op_single(
        &mut self,
        tag_expr: &TagExpr,
        tag: &Tag,
        negate: bool,
    ) -> z3::ast::Bool {
        match tag_expr {
            TagExpr::Field(fv) => {
                let path = Self::field_path(fv);
                let v = self.tag_field_var(&path, tag);
                if negate { v.not() } else { v }
            }
            TagExpr::Any(ids) => {
                let parts: Vec<z3::ast::Bool> = ids
                    .iter()
                    .map(|ir::Id(id)| {
                        let v = self.tag_field_var(id, tag);
                        if negate { v.not() } else { v }
                    })
                    .collect();
                let refs: Vec<&z3::ast::Bool> = parts.iter().collect();
                z3::ast::Bool::or(&refs)
            }
            TagExpr::Every(ids) => {
                let parts: Vec<z3::ast::Bool> = ids
                    .iter()
                    .map(|ir::Id(id)| {
                        let v = self.tag_field_var(id, tag);
                        if negate { v.not() } else { v }
                    })
                    .collect();
                let refs: Vec<&z3::ast::Bool> = parts.iter().collect();
                z3::ast::Bool::and(&refs)
            }
        }
    }

    fn translate_tag_op_set(
        &mut self,
        tag_expr: &TagExpr,
        tag_set_id: &TagSetId,
        conjunction: bool,
        negate_each: bool,
    ) -> z3::ast::Bool {
        let TagSetId(idx) = tag_set_id;
        let tags = self.code.tag_sets[*idx].clone();
        let parts: Vec<z3::ast::Bool> = tags
            .iter()
            .map(|tag| self.translate_tag_op_single(tag_expr, tag, negate_each))
            .collect();
        let refs: Vec<&z3::ast::Bool> = parts.iter().collect();
        if conjunction {
            z3::ast::Bool::and(&refs)
        } else {
            z3::ast::Bool::or(&refs)
        }
    }

    fn translate_expr_to_int(&mut self, expr: &Expr) -> Option<z3::ast::Int> {
        match expr {
            Expr::Maths(m) => Some(self.translate_math_expr(m)),
            Expr::Field(fv) => {
                let path = Self::field_path(fv);
                Some(self.int_var(&path))
            }
            Expr::String(_) => None,
        }
    }

    fn translate_bool_expr(&mut self, expr: &BoolExpr) -> z3::ast::Bool {
        match expr {
            BoolExpr::True => z3::ast::Bool::from_bool(true),
            BoolExpr::False => z3::ast::Bool::from_bool(false),
            BoolExpr::And(a, b) => {
                let a = self.translate_bool_expr(a);
                let b = self.translate_bool_expr(b);
                z3::ast::Bool::and(&[&a, &b])
            }
            BoolExpr::Or(a, b) => {
                let a = self.translate_bool_expr(a);
                let b = self.translate_bool_expr(b);
                z3::ast::Bool::or(&[&a, &b])
            }
            BoolExpr::Not(a) => {
                let a = self.translate_bool_expr(a);
                a.not()
            }
            BoolExpr::Gt(a, b) => {
                let a = self.translate_math_expr(a);
                let b = self.translate_math_expr(b);
                a.gt(&b)
            }
            BoolExpr::Lt(a, b) => {
                let a = self.translate_math_expr(a);
                let b = self.translate_math_expr(b);
                a.lt(&b)
            }
            BoolExpr::Gte(a, b) => {
                let a = self.translate_math_expr(a);
                let b = self.translate_math_expr(b);
                a.ge(&b)
            }
            BoolExpr::Lte(a, b) => {
                let a = self.translate_math_expr(a);
                let b = self.translate_math_expr(b);
                a.le(&b)
            }
            BoolExpr::Eq(a, b) => {
                let ai = self.translate_expr_to_int(a);
                let bi = self.translate_expr_to_int(b);
                if let (Some(ai), Some(bi)) = (ai, bi) {
                    ai.eq(&bi)
                } else {
                    self.fresh_opaque("eq")
                }
            }
            BoolExpr::Neq(a, b) => {
                let ai = self.translate_expr_to_int(a);
                let bi = self.translate_expr_to_int(b);
                if let (Some(ai), Some(bi)) = (ai, bi) {
                    ai.eq(&bi).not()
                } else {
                    self.fresh_opaque("neq")
                }
            }
            BoolExpr::Match(_, _) => self.fresh_opaque("match"),
            BoolExpr::Rule(ir::Id(id), _) => {
                if let Some(action) = self.code.action_rules.iter().find(|ActionRules(ActionName(name), _, _, _, _, _, _)| name == id) {
                    self.accept_formula(action)
                } else {
                    self.fresh_opaque(&format!("rule_{id}"))
                }
            }
            BoolExpr::Contains(tag_expr, tag) => {
                self.translate_tag_op_single(tag_expr, tag, false)
            }
            BoolExpr::ContainsAll(tag_expr, tag_set_id) => {
                self.translate_tag_op_set(tag_expr, tag_set_id, true, false)
            }
            BoolExpr::ContainsAny(tag_expr, tag_set_id) => {
                self.translate_tag_op_set(tag_expr, tag_set_id, false, false)
            }
            BoolExpr::Lacks(tag_expr, tag) => {
                self.translate_tag_op_single(tag_expr, tag, true)
            }
            BoolExpr::LacksAll(tag_expr, tag_set_id) => {
                self.translate_tag_op_set(tag_expr, tag_set_id, true, true)
            }
            BoolExpr::LacksAny(tag_expr, tag_set_id) => {
                self.translate_tag_op_set(tag_expr, tag_set_id, false, true)
            }
        }
    }

    fn translate_condition(&mut self, cond: &Condition) -> z3::ast::Bool {
        match cond {
            Condition::Always => z3::ast::Bool::from_bool(true),
            Condition::Never => z3::ast::Bool::from_bool(false),
            Condition::When(expr) => self.translate_bool_expr(expr),
        }
    }

    fn accept_formula(&mut self, action: &ActionRules) -> z3::ast::Bool {
        let ActionRules(_, _, _, fallback, allow_conds, deny_conds, _) = action;
        let allow_cond = allow_conds.iter().fold(Condition::Never, |acc, lc| acc.join(&lc.condition));
        let deny_cond = deny_conds.iter().fold(Condition::Never, |acc, lc| acc.join(&lc.condition));
        let allow_z3 = self.translate_condition(&allow_cond);
        let deny_z3 = self.translate_condition(&deny_cond);
        let not_deny = deny_z3.not();

        match fallback {
            Fallback::Allow => not_deny,
            Fallback::Deny | Fallback::Warn => {
                z3::ast::Bool::and(&[&not_deny, &allow_z3])
            }
        }
    }
}

pub struct ActionCoverageReport {
    pub name: String,
    pub accept_description: String,
    pub status: AcceptStatus,
}

pub enum AcceptStatus {
    AlwaysAccepts,
    NeverAccepts,
    Conditional,
    Unknown, // Solver timed out or returned unknown
}

impl fmt::Display for ActionCoverageReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status = match self.status {
            AcceptStatus::AlwaysAccepts => "always accepts",
            AcceptStatus::NeverAccepts => "never accepts",
            AcceptStatus::Conditional => "conditional",
            AcceptStatus::Unknown => "unknown (solver timeout)",
        };
        write!(
            f,
            "Action: {}\n  Status: {}\n  Accept when: {}",
            self.name, status, self.accept_description
        )
    }
}

pub fn describe_coverage(code: &Code, timeout_ms: u32) -> Vec<ActionCoverageReport> {
    let _cfg = z3::Config::new();
    let mut results = Vec::new();

    for action in &code.action_rules {
        let mut cov_ctx = CoverageCtx::new(code);
        let formula = cov_ctx.accept_formula(action);
        let simplified = formula.simplify();
        let description = format!("{}", simplified);

        let solver = create_solver_with_timeout(timeout_ms);

        // Check if always true (tautology): NOT(formula) is unsat
        solver.push();
        solver.assert(&formula.not());
        let tautology_check = solver.check();
        solver.pop(1);

        // Check if never true: formula is unsat
        solver.push();
        solver.assert(&formula);
        let satisfiability_check = solver.check();
        solver.pop(1);

        let status = match (tautology_check, satisfiability_check) {
            (SatResult::Unsat, _) => AcceptStatus::AlwaysAccepts,
            (_, SatResult::Unsat) => AcceptStatus::NeverAccepts,
            (SatResult::Sat, SatResult::Sat) => AcceptStatus::Conditional,
            (SatResult::Unknown, _) | (_, SatResult::Unknown) => AcceptStatus::Unknown,
        };

        let ActionRules(ActionName(name), _, _, _, _, _, _) = action;
        results.push(ActionCoverageReport {
            name: name.clone(),
            accept_description: description,
            status,
        });
    }

    results
}

pub enum CoverageComparison {
    Equal,
    StrictlyMoreRestrictive,
    StrictlyLessRestrictive,
    Incomparable {
        only_a: Vec<ActionDiff>,
        only_b: Vec<ActionDiff>,
    },
}

pub struct ActionDiff {
    pub action: String,
    pub witness: String,
}

impl fmt::Display for CoverageComparison {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoverageComparison::Equal => write!(f, "Policies are equivalent."),
            CoverageComparison::StrictlyMoreRestrictive => {
                write!(f, "Policy A is strictly more restrictive than Policy B.")
            }
            CoverageComparison::StrictlyLessRestrictive => {
                write!(f, "Policy A is strictly less restrictive than Policy B.")
            }
            CoverageComparison::Incomparable { only_a, only_b } => {
                writeln!(f, "Policies are incomparable.")?;
                if !only_a.is_empty() {
                    writeln!(f, "\nA accepts but B denies:")?;
                    for diff in only_a {
                        writeln!(f, "  Action {}: {}", diff.action, diff.witness)?;
                    }
                }
                if !only_b.is_empty() {
                    writeln!(f, "\nB accepts but A denies:")?;
                    for diff in only_b {
                        writeln!(f, "  Action {}: {}", diff.action, diff.witness)?;
                    }
                }
                Ok(())
            }
        }
    }
}

pub fn compare(a: &Code, b: &Code, timeout_ms: u32) -> CoverageComparison {
    let _cfg = z3::Config::new();

    let mut only_a_diffs: Vec<ActionDiff> = Vec::new();
    let mut only_b_diffs: Vec<ActionDiff> = Vec::new();
    let mut had_unknown = false;

    let a_actions: HashMap<&str, &ActionRules> = a
        .action_rules
        .iter()
        .map(|ac| (ac.0.0.as_str(), ac))
        .collect();
    let b_actions: HashMap<&str, &ActionRules> = b
        .action_rules
        .iter()
        .map(|ac| (ac.0.0.as_str(), ac))
        .collect();

    let mut all_names: Vec<&str> = Vec::new();
    for name in a_actions.keys() {
        all_names.push(name);
    }
    for name in b_actions.keys() {
        if !a_actions.contains_key(name) {
            all_names.push(name);
        }
    }

    for name in all_names {
        let a_action = a_actions.get(name);
        let b_action = b_actions.get(name);

        match (a_action, b_action) {
            (Some(aa), Some(ba)) => {
                let mut ctx_a = CoverageCtx::new(a);
                let formula_a = ctx_a.accept_formula(aa);

                let mut ctx_b = CoverageCtx::new(b);
                let formula_b = ctx_b.accept_formula(ba);

                let solver = create_solver_with_timeout(timeout_ms);

                // Check SAT(a AND NOT b)
                solver.push();
                solver.assert(&formula_a);
                solver.assert(&formula_b.not());
                let a_not_b = solver.check();
                let a_not_b_witness = match a_not_b {
                    SatResult::Sat => solver
                        .get_model()
                        .map(|m| format!("{}", m))
                        .unwrap_or_else(|| "no model".to_string()),
                    SatResult::Unknown => {
                        had_unknown = true;
                        "solver timeout".to_string()
                    }
                    SatResult::Unsat => String::new(),
                };
                solver.pop(1);

                // Check SAT(b AND NOT a)
                solver.push();
                solver.assert(&formula_b);
                solver.assert(&formula_a.not());
                let b_not_a = solver.check();
                let b_not_a_witness = match b_not_a {
                    SatResult::Sat => solver
                        .get_model()
                        .map(|m| format!("{}", m))
                        .unwrap_or_else(|| "no model".to_string()),
                    SatResult::Unknown => {
                        had_unknown = true;
                        "solver timeout".to_string()
                    }
                    SatResult::Unsat => String::new(),
                };
                solver.pop(1);

                // For Unknown, we conservatively treat it as potentially different
                if a_not_b == SatResult::Sat || a_not_b == SatResult::Unknown {
                    only_a_diffs.push(ActionDiff {
                        action: name.to_string(),
                        witness: a_not_b_witness,
                    });
                }
                if b_not_a == SatResult::Sat || b_not_a == SatResult::Unknown {
                    only_b_diffs.push(ActionDiff {
                        action: name.to_string(),
                        witness: b_not_a_witness,
                    });
                }
            }
            (Some(_), None) => {
                only_a_diffs.push(ActionDiff {
                    action: name.to_string(),
                    witness: "action only exists in policy A".to_string(),
                });
            }
            (None, Some(_)) => {
                only_b_diffs.push(ActionDiff {
                    action: name.to_string(),
                    witness: "action only exists in policy B".to_string(),
                });
            }
            (None, None) => unreachable!(),
        }
    }

    if had_unknown {
        // If any check was unknown, we can't make definitive claims
        CoverageComparison::Incomparable {
            only_a: only_a_diffs,
            only_b: only_b_diffs,
        }
    } else if only_a_diffs.is_empty() && only_b_diffs.is_empty() {
        CoverageComparison::Equal
    } else if only_a_diffs.is_empty() {
        CoverageComparison::StrictlyMoreRestrictive
    } else if only_b_diffs.is_empty() {
        CoverageComparison::StrictlyLessRestrictive
    } else {
        CoverageComparison::Incomparable {
            only_a: only_a_diffs,
            only_b: only_b_diffs,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::ast::{self, FieldList, FieldValue, Id};
    use crate::ir;

    const DEFAULT_TIMEOUT_MS: u32 = 5000;

    fn empty_code() -> Code {
        Code {
            regexes: Vec::new(),
            tags: Vec::new(),
            tag_sets: Vec::new(),
            structs: Vec::new(),
            action_rules: Vec::new(),
        }
    }

    #[test]
    fn test_simple_gt_is_satisfiable() {
        let code = empty_code();
        let mut cov = CoverageCtx::new(&code);

        let expr = BoolExpr::Gt(
            Box::new(MathExpr::Field(FieldValue(vec![Id("x".into())]))),
            Box::new(MathExpr::Num(4)),
        );
        let formula = cov.translate_bool_expr(&expr);

        let solver = create_solver_with_timeout(DEFAULT_TIMEOUT_MS);
        solver.assert(&formula);
        assert_eq!(solver.check(), SatResult::Sat);
    }

    #[test]
    fn test_contradiction_is_unsat() {
        let code = empty_code();
        let mut cov = CoverageCtx::new(&code);

        let expr = BoolExpr::And(
            Box::new(BoolExpr::Gt(
                Box::new(MathExpr::Field(FieldValue(vec![Id("x".into())]))),
                Box::new(MathExpr::Num(4)),
            )),
            Box::new(BoolExpr::Lt(
                Box::new(MathExpr::Field(FieldValue(vec![Id("x".into())]))),
                Box::new(MathExpr::Num(3)),
            )),
        );
        let formula = cov.translate_bool_expr(&expr);

        let solver = create_solver_with_timeout(DEFAULT_TIMEOUT_MS);
        solver.assert(&formula);
        assert_eq!(solver.check(), SatResult::Unsat);
    }

    #[test]
    fn test_tag_contains_satisfiable() {
        let code = empty_code();
        let mut cov = CoverageCtx::new(&code);

        let expr = BoolExpr::Contains(
            TagExpr::Field(FieldValue(vec![Id("data".into())])),
            Tag("sensitive".into()),
        );
        let formula = cov.translate_bool_expr(&expr);

        let solver = create_solver_with_timeout(DEFAULT_TIMEOUT_MS);
        solver.assert(&formula);
        assert_eq!(solver.check(), SatResult::Sat);
    }

    #[test]
    fn test_contains_and_lacks_same_tag_unsat() {
        let code = empty_code();
        let mut cov = CoverageCtx::new(&code);

        let expr = BoolExpr::And(
            Box::new(BoolExpr::Contains(
                TagExpr::Field(FieldValue(vec![Id("data".into())])),
                Tag("sensitive".into()),
            )),
            Box::new(BoolExpr::Lacks(
                TagExpr::Field(FieldValue(vec![Id("data".into())])),
                Tag("sensitive".into()),
            )),
        );
        let formula = cov.translate_bool_expr(&expr);

        let solver = create_solver_with_timeout(DEFAULT_TIMEOUT_MS);
        solver.assert(&formula);
        assert_eq!(solver.check(), SatResult::Unsat);
    }

    #[test]
    fn test_compare_stricter_policy() {
        let make_code = |threshold: i32| -> Code {
            Code {
                regexes: Vec::new(),
                tags: Vec::new(),
                tag_sets: Vec::new(),
                structs: Vec::new(),
                action_rules: vec![ir::ActionRules(
                    ir::ActionName("test".into()),
                    FieldList(vec![]),
                    ast::Type::Int,
                    Fallback::Deny,
                    vec![ir::LabelledCondition { label: "allow".into(), condition: Condition::When(Box::new(BoolExpr::Gt(
                        Box::new(MathExpr::Field(FieldValue(vec![Id("x".into())]))),
                        Box::new(MathExpr::Num(threshold)),
                    )))}],
                    vec![ir::LabelledCondition { label: "deny".into(), condition: Condition::Never}],
                    ir::Applications(Vec::new()),
                )],
            }
        };

        let a = make_code(10);
        let b = make_code(5);
        let result = compare(&a, &b, DEFAULT_TIMEOUT_MS);
        assert!(matches!(result, CoverageComparison::StrictlyMoreRestrictive));
    }

    #[test]
    fn test_compare_equal_policies() {
        let make_code = || -> Code {
            Code {
                regexes: Vec::new(),
                tags: Vec::new(),
                tag_sets: Vec::new(),
                structs: Vec::new(),
                action_rules: vec![ir::ActionRules(
                    ir::ActionName("test".into()),
                    FieldList(vec![]),
                    ast::Type::Int,
                    Fallback::Deny,
                    vec![ir::LabelledCondition {
                        label: "allow".into(),
                        condition: Condition::When(Box::new(BoolExpr::Gt(
                            Box::new(MathExpr::Field(FieldValue(vec![Id("x".into())]))),
                            Box::new(MathExpr::Num(5)),
                        ))),
                    }],
                    vec![ir::LabelledCondition { label: "deny".into(), condition: Condition::Never}],
                    ir::Applications(Vec::new()),
                )],
            }
        };

        let result = compare(&make_code(), &make_code(), DEFAULT_TIMEOUT_MS);
        assert!(matches!(result, CoverageComparison::Equal));
    }

    #[test]
    fn test_compare_incomparable() {
        let code_a = Code {
            regexes: Vec::new(),
            tags: Vec::new(),
            tag_sets: Vec::new(),
            structs: Vec::new(),
            action_rules: vec![ir::ActionRules(
                ir::ActionName("test".into()),
                FieldList(vec![]),
                ast::Type::Int,
                Fallback::Deny,
                vec![ir::LabelledCondition {
                    label: "allow".into(),
                    condition: Condition::When(Box::new(BoolExpr::Gt(
                        Box::new(MathExpr::Field(FieldValue(vec![Id("x".into())]))),
                        Box::new(MathExpr::Num(5)),
                    ))),
                }],
                vec![ir::LabelledCondition { label: "deny".into(), condition: Condition::Never}],
                ir::Applications(Vec::new()),
            )],
        };

        let code_b = Code {
            regexes: Vec::new(),
            tags: Vec::new(),
            tag_sets: Vec::new(),
            structs: Vec::new(),
            action_rules: vec![ir::ActionRules(
                ir::ActionName("test".into()),
                FieldList(vec![]),
                ast::Type::Int,
                Fallback::Deny,
                vec![ir::LabelledCondition {
                    label: "allow".into(), 
                    condition: Condition::When(Box::new(BoolExpr::Lt(
                        Box::new(MathExpr::Field(FieldValue(vec![Id("x".into())]))),
                        Box::new(MathExpr::Num(3)),
                    ))),
                }],
                vec![ir::LabelledCondition { label: "deny".into(), condition: Condition::Never}],
                ir::Applications(Vec::new()),
            )],
        };

        let result = compare(&code_a, &code_b, DEFAULT_TIMEOUT_MS);
        assert!(matches!(result, CoverageComparison::Incomparable { .. }));
    }

    #[test]
    fn test_relational_field_comparison() {
        let code = empty_code();
        let mut cov = CoverageCtx::new(&code);

        let expr = BoolExpr::Lt(
            Box::new(MathExpr::Field(FieldValue(vec![Id("x".into())]))),
            Box::new(MathExpr::Field(FieldValue(vec![Id("y".into())]))),
        );
        let formula = cov.translate_bool_expr(&expr);

        let solver = create_solver_with_timeout(DEFAULT_TIMEOUT_MS);
        solver.assert(&formula);
        assert_eq!(solver.check(), SatResult::Sat);
    }
}
