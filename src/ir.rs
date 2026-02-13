use crate::ast::{Fallback, FieldList, FieldValue, Type};

#[allow(unused)]
#[derive(Debug, Clone)]
pub struct Id(pub String);
#[allow(unused)]
#[derive(Debug, Clone)]
pub struct Tag(pub String);
#[allow(unused)]
#[derive(Debug, Clone)]
pub struct TagList(pub Vec<Tag>);

#[allow(unused)]
#[derive(Debug, Clone, PartialEq)]
pub struct StructName(pub String);

#[allow(unused)]
#[derive(Debug)]
pub struct Struct(pub StructName, pub TagList, pub FieldList);

#[allow(unused)]
#[derive(Debug)]
pub struct ActionName(pub String);

#[allow(unused)]
#[derive(Debug, Clone)]
pub struct Applications(pub Vec<(TagList, Condition)>);

impl Applications {
    pub(crate) fn join(&mut self, other: &mut Applications) {
        self.0.append(&mut other.0)
    }
}

#[allow(unused)]
#[derive(Debug, Clone)]
pub struct LabelledCondition {
    pub label: String,
    pub condition: Condition,
}

#[allow(unused)]
#[derive(Debug)]
pub struct ActionRules(
    pub ActionName,
    pub FieldList,
    pub Type,
    pub Fallback,
    pub Vec<LabelledCondition>,
    pub Vec<LabelledCondition>,
    pub Applications,
);

#[allow(unused)]
#[derive(Debug, Clone)]
pub enum Condition {
    When(Box<BoolExpr>),
    Always,
    Never,
}

impl Into<Box<BoolExpr>> for &Condition {
    fn into(self) -> Box<BoolExpr> {
        match self {
            Condition::When(e) => e.clone(),
            Condition::Always => Box::new(BoolExpr::True),
            Condition::Never => Box::new(BoolExpr::False),
        }
    }
}

impl Condition {
    pub(crate) fn join(&self, other: &Condition) -> Condition {
        Condition::When(Box::new(BoolExpr::Or(self.into(), other.into())))
    }
}

#[allow(unused)]
#[derive(Debug, Clone)]
pub enum BoolExpr {
    Or(Box<BoolExpr>, Box<BoolExpr>),
    And(Box<BoolExpr>, Box<BoolExpr>),
    Not(Box<BoolExpr>),

    Rule(Id, ExprList),

    Gt(Box<MathExpr>, Box<MathExpr>),
    Lt(Box<MathExpr>, Box<MathExpr>),
    Gte(Box<MathExpr>, Box<MathExpr>),
    Lte(Box<MathExpr>, Box<MathExpr>),

    Eq(Expr, Expr),
    Neq(Expr, Expr),

    Match(StringExpr, RegexId),

    Contains(TagExpr, Tag),
    ContainsAll(TagExpr, TagSetId),
    ContainsAny(TagExpr, TagSetId),
    Lacks(TagExpr, Tag),
    LacksAll(TagExpr, TagSetId),
    LacksAny(TagExpr, TagSetId),

    True,
    False,
}

#[allow(unused)]
#[derive(Debug, Clone)]
pub struct TagSetId(pub usize);

#[allow(unused)]
#[derive(Debug, Clone)]
pub enum TagBoolOp {
    Contains,
    ContainsAll,
    ContainsAny,
    Lacks,
    LacksAll,
    LacksAny,
}

#[allow(unused)]
#[derive(Debug, Clone)]
pub enum MathExpr {
    Add(Box<MathExpr>, Box<MathExpr>),
    Sub(Box<MathExpr>, Box<MathExpr>),
    Mul(Box<MathExpr>, Box<MathExpr>),
    Div(Box<MathExpr>, Box<MathExpr>),
    Neg(Box<MathExpr>),

    Num(i32),
    Field(FieldValue),
}

#[allow(unused)]
#[derive(Debug, Clone)]
pub enum StringExpr {
    String(String),
    Field(FieldValue),
}
#[allow(unused)]
#[derive(Debug, Clone, PartialEq)]
pub struct Regex(pub String);
#[allow(unused)]
#[derive(Debug, Clone)]
pub struct RegexId(pub usize);

#[allow(unused)]
#[derive(Debug, Clone)]
pub enum TagExpr {
    Field(FieldValue),
    Any(Vec<Id>),
    Every(Vec<Id>),
}

#[allow(unused)]
#[derive(Debug, Clone)]
pub enum Expr {
    Maths(Box<MathExpr>),
    String(StringExpr),
    Field(FieldValue),
}
#[allow(unused)]
#[derive(Debug, Clone)]
pub struct ExprList(pub Vec<Expr>);

#[allow(unused)]
#[derive(Debug)]
pub struct Code {
    pub regexes: Vec<Regex>,
    pub tags: Vec<Tag>,
    pub tag_sets: Vec<Vec<Tag>>,
    pub structs: Vec<Struct>,
    pub action_rules: Vec<ActionRules>,
}
