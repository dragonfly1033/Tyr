use crate::CompilerError;

pub fn is_keyword(word: &str) -> bool {
    matches!(
        word,
        "tag"
            | "taggroup"
            | "struct"
            | "action"
            | "actiongroup"
            | "rules"
            | "fallback"
            | "allow"
            | "deny"
            | "warn"
            | "when"
            | "apply"
            | "always"
            | "never"
            | "str"
            | "int"
            | "bool"
            | "and"
            | "or"
            | "not"
            | "true"
            | "false"
            | "allowed"
            | "matches"
            | "contains"
            | "contains_any"
            | "contains_all"
            | "lacks"
            | "lacks_any"
            | "lacks_all"
            | "any_arg"
            | "every_arg"
    )
}

#[allow(unused)]
#[derive(Debug, PartialEq, Clone)]
pub struct Id(pub String);
#[allow(unused)]
#[derive(Debug, PartialEq)]
pub struct IdList(pub Vec<Id>);
#[allow(unused)]
#[derive(Debug, PartialEq, Hash, Eq, Clone)]
pub struct TitleId(pub String);
#[allow(unused)]
#[derive(Debug, PartialEq)]
pub struct Tag(pub Id);
#[allow(unused)]
#[derive(Debug, PartialEq)]
pub struct TagG(pub Id, pub IdList);

#[allow(unused)]
#[derive(Debug, PartialEq, Hash, Eq, Clone)]
pub enum Type {
    Struct(TitleId),
    String,
    Int,
    Bool,
}
impl From<&Type> for String {
    fn from(value: &Type) -> Self {
        match value {
            Type::Struct(TitleId(s)) => s.clone(),
            Type::String => String::from("str"),
            Type::Int => String::from("int"),
            Type::Bool => String::from("bool"),
        }
    }
}
#[allow(unused)]
#[derive(Debug, PartialEq)]
pub struct TypeList(pub Vec<Type>);
#[allow(unused)]
#[derive(Debug, PartialEq, Clone)]
pub struct Field(pub Id, pub Type);
#[allow(unused)]
#[derive(Debug, PartialEq, Clone)]
pub struct FieldList(pub Vec<Field>);
#[allow(unused)]
#[derive(Debug, PartialEq)]
pub struct Struct(pub TitleId, pub IdList, pub FieldList);

#[allow(unused)]
#[derive(Debug, PartialEq)]
pub struct Action(pub Id, pub TypeList, pub Type, pub Fallback);
#[allow(unused)]
#[derive(Debug, PartialEq)]
pub struct ActionG(pub Id, pub IdList);

#[allow(unused)]
#[derive(Debug, PartialEq, Clone)]
pub enum Fallback {
    Allow,
    Deny,
    Warn,
}
#[allow(unused)]
#[derive(Debug, PartialEq)]
pub enum Rule {
    Allow(Condition),
    Deny(Condition),
    Apply(IdList, Condition),
}
#[allow(unused)]
#[derive(Debug, PartialEq)]
pub struct Rules(pub Vec<Rule>);
#[allow(unused)]
#[derive(Debug, PartialEq)]
pub struct RuleBlock(pub Id, pub FieldList, pub Rules);

#[allow(unused)]
#[derive(Debug, PartialEq)]
pub enum Condition {
    When(Box<BoolExpr>),
    Always,
    Never,
}

#[allow(unused)]
#[derive(Debug, PartialEq)]
pub enum BoolExpr {
    And(Box<BoolExpr>, Box<BoolExpr>),
    Or(Box<BoolExpr>, Box<BoolExpr>),
    Not(Box<BoolExpr>),

    Rule(Id, ExprList),

    Gt(Box<Expr>, Box<Expr>),
    Lt(Box<Expr>, Box<Expr>),
    Gte(Box<Expr>, Box<Expr>),
    Lte(Box<Expr>, Box<Expr>),

    Eq(Box<Expr>, Box<Expr>),
    Neq(Box<Expr>, Box<Expr>),

    Match(Box<Expr>, Box<Expr>),

    Contains(Box<Expr>, Id),
    ContainsAll(Box<Expr>, IdList),
    ContainsAny(Box<Expr>, IdList),
    Lacks(Box<Expr>, Id),
    LacksAll(Box<Expr>, IdList),
    LacksAny(Box<Expr>, IdList),

    True,
    False,
}

#[allow(unused)]
#[derive(Debug, PartialEq, Clone)]
pub struct FieldValue(pub Vec<Id>);
#[allow(unused)]
#[derive(Debug, PartialEq)]
pub enum Expr {
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Div(Box<Expr>, Box<Expr>),
    Neg(Box<Expr>),
    Num(i32),

    Field(FieldValue),

    String(String),
    Regex(String),

    AnyArg,
    EveryArg,
}
#[allow(unused)]
#[derive(Debug, PartialEq)]
pub struct ExprList(pub Vec<Box<Expr>>);

#[allow(unused)]
#[derive(Debug, PartialEq)]
pub enum ExprType {
    Int,
    Bool,
    Struct(String),
    String,
    Regex,
    TagList,
}
impl TryFrom<ExprType> for Type {
    type Error = CompilerError;

    fn try_from(value: ExprType) -> Result<Self, Self::Error> {
        match value {
            ExprType::Bool => Ok(Type::Bool),
            ExprType::String => Ok(Type::String),
            ExprType::Int => Ok(Type::Int),
            ExprType::Struct(s) => Ok(Type::Struct(TitleId(s))),
            ExprType::Regex => Err(CompilerError::TypeError(format!(
                "Regex expressions cannot be passed as values."
            ))),
            ExprType::TagList => Err(CompilerError::TypeError(format!(
                "TagList expressions cannot be passed as values."
            ))),
        }
    }
}
impl From<Type> for ExprType {
    fn from(value: Type) -> Self {
        match value {
            Type::Bool => ExprType::Bool,
            Type::String => ExprType::String,
            Type::Int => ExprType::Int,
            Type::Struct(TitleId(s)) => ExprType::Struct(s),
        }
    }
}

#[allow(unused)]
#[derive(Debug, PartialEq)]
pub enum CodeItem {
    Tag(Tag),
    TagG(TagG),
    Struct(Struct),
    Action(Action),
    ActionG(ActionG),
    RuleBlock(RuleBlock),
}

#[derive(Debug, PartialEq)]
pub enum CodeItemType {
    Tag,
    TagG,
    Struct,
    Action,
    ActionG,
    #[allow(unused)]
    RuleBlock,
}

#[allow(unused)]
#[derive(Debug, PartialEq)]
pub struct Code(pub Vec<CodeItem>);

#[cfg(test)]
mod test {
    use crate::{
        ast::{
            Action, ActionG, BoolExpr, CodeItem, Condition, Expr, ExprList, Fallback, Field,
            FieldList, FieldValue, Id, IdList, Rule, RuleBlock, Rules, Struct, Tag, TagG, TitleId,
            Type, TypeList,
        },
        grammar,
    };

    #[test]
    fn test_id_parser() {
        let id_parser = grammar::IdParser::new();

        let test_valid_word = |word: &str| {
            let res = id_parser.parse(word);
            assert_eq!(
                res,
                Ok(Id(String::from(word))),
                "Id parser failed on: {:?}, got: {:?}",
                word,
                res
            );
        };
        let test_invalid_word = |word: &str| {
            let res = id_parser.parse(word);
            assert!(
                res.is_err(),
                "Id parser incorrectly passed on: {:?}, got: {:?}",
                word,
                res
            );
        };

        test_valid_word("this");
        test_valid_word("this_is_valid");
        test_valid_word("s015Th1s");
        test_invalid_word("But_not_this");
        test_invalid_word("_or_this");
        test_invalid_word("or this");
        test_invalid_word("norTh|s");
        test_invalid_word("tag");
        test_invalid_word("struct");
        test_invalid_word("");
    }

    #[test]
    fn test_title_id_parser() {
        let title_id_parser = grammar::TitleIdParser::new();

        let test_valid_word = |word: &str| {
            let res = title_id_parser.parse(word);
            assert_eq!(
                res,
                Ok(TitleId(String::from(word))),
                "TitleId parser failed on: {:?}, got: {:?}",
                word,
                res
            );
        };
        let test_invalid_word = |word: &str| {
            let res = title_id_parser.parse(word);
            assert!(
                res.is_err(),
                "TitleId parser incorrectly passed on: {:?}, got: {:?}",
                word,
                res
            );
        };

        test_valid_word("This");
        test_valid_word("This_is_valid");
        test_valid_word("S015Th1s");
        test_invalid_word("but_not_this");
        test_invalid_word("_or_this");
        test_invalid_word("or this");
        test_invalid_word("norTh|s");
        test_invalid_word("");
    }

    #[test]
    fn test_id_list_parser() {
        let id_list_parser = grammar::IdListParser::new();

        let test_valid_word = |word: &str, expected: IdList| {
            let res = id_list_parser.parse(word);
            assert_eq!(
                res,
                Ok(expected),
                "IdList parser failed on: {:?}, got: {:?}",
                word,
                res
            );
        };
        let test_invalid_word = |word: &str| {
            let res = id_list_parser.parse(word);
            assert!(
                res.is_err(),
                "IdList parser incorrectly passed on: {:?}, got: {:?}",
                word,
                res
            );
        };

        test_valid_word("this", IdList(vec![Id(String::from("this"))]));
        test_valid_word("this,", IdList(vec![Id(String::from("this"))]));
        test_valid_word(
            "this,that,other",
            IdList(vec![
                Id(String::from("this")),
                Id(String::from("that")),
                Id(String::from("other")),
            ]),
        );
        test_valid_word(
            "this , that,   other,",
            IdList(vec![
                Id(String::from("this")),
                Id(String::from("that")),
                Id(String::from("other")),
            ]),
        );

        test_invalid_word(",this");
        test_invalid_word(",");
        test_invalid_word("");
    }

    #[test]
    fn test_tag_parser() {
        let tag_parser = grammar::TagParser::new();

        let test_valid_word_tag = |word: &str, expected: Tag| {
            let res = tag_parser.parse(word);
            assert_eq!(
                res,
                Ok(expected),
                "Tag parser failed on: {:?}, got: {:?}",
                word,
                res
            );
        };
        let test_invalid_word_tag = |word: &str| {
            let res = tag_parser.parse(word);
            assert!(
                res.is_err(),
                "Tag parser incorrectly passed on: {:?}, got: {:?}",
                word,
                res
            );
        };

        test_valid_word_tag("tag this", Tag(Id(String::from("this"))));
        test_valid_word_tag("tag   this", Tag(Id(String::from("this"))));

        test_invalid_word_tag("tagthis");
        test_invalid_word_tag("this");
        test_invalid_word_tag("");
        test_invalid_word_tag("tag this that");
        test_invalid_word_tag("tag tag this");
        test_invalid_word_tag("tag tag");
    }

    #[test]
    fn test_tag_group_parser() {
        let tagg_parser = grammar::TagGParser::new();

        let test_valid_word_tagg = |word: &str, expected: TagG| {
            let res = tagg_parser.parse(word);
            assert_eq!(
                res,
                Ok(expected),
                "TagG parser failed on: {:?}, got: {:?}",
                word,
                res
            );
        };
        let test_invalid_word_tagg = |word: &str| {
            let res = tagg_parser.parse(word);
            assert!(
                res.is_err(),
                "TagG parser incorrectly passed on: {:?}, got: {:?}",
                word,
                res
            );
        };

        test_valid_word_tagg(
            "taggroup name = this",
            TagG(
                Id(String::from("name")),
                IdList(vec![Id(String::from("this"))]),
            ),
        );
        test_valid_word_tagg(
            "taggroup name = this, that, other",
            TagG(
                Id(String::from("name")),
                IdList(vec![
                    Id(String::from("this")),
                    Id(String::from("that")),
                    Id(String::from("other")),
                ]),
            ),
        );
        test_valid_word_tagg(
            "taggroup name =this,that ,  other,",
            TagG(
                Id(String::from("name")),
                IdList(vec![
                    Id(String::from("this")),
                    Id(String::from("that")),
                    Id(String::from("other")),
                ]),
            ),
        );

        test_invalid_word_tagg("tag this = that");
        test_invalid_word_tagg("taggroupthis");
        test_invalid_word_tagg("taggroup taggroup = this");
        test_invalid_word_tagg("taggroup this =");
        test_invalid_word_tagg("taggroup this = ,");
        test_invalid_word_tagg("");
    }

    #[test]
    fn test_type_parser() {
        let parser = grammar::TypeParser::new();

        let test_valid_word = |word: &str, expected: Type| {
            let res = parser.parse(word);
            assert_eq!(
                res,
                Ok(expected),
                "Type parser failed on: {:?}, got: {:?}",
                word,
                res
            );
        };
        let test_invalid_word = |word: &str| {
            let res = parser.parse(word);
            assert!(
                res.is_err(),
                "Type parser incorrectly passed on: {:?}, got: {:?}",
                word,
                res
            );
        };

        test_valid_word("str", Type::String);
        test_valid_word("int", Type::Int);
        test_valid_word("bool", Type::Bool);
        test_valid_word("This", Type::Struct(TitleId(String::from("This"))));
        test_valid_word("ThatType", Type::Struct(TitleId(String::from("ThatType"))));
        test_invalid_word("this");
        test_invalid_word("this that");
        test_invalid_word("This that");
        test_invalid_word("");
    }

    #[test]
    fn test_type_list_parser() {
        let parser = grammar::TypeListParser::new();

        let test_valid_word = |word: &str, expected: TypeList| {
            let res = parser.parse(word);
            assert_eq!(
                res,
                Ok(expected),
                "TypeList parser failed on: {:?}, got: {:?}",
                word,
                res
            );
        };
        let test_invalid_word = |word: &str| {
            let res = parser.parse(word);
            assert!(
                res.is_err(),
                "TypeList parser incorrectly passed on: {:?}, got: {:?}",
                word,
                res
            );
        };

        test_valid_word(
            "str,int,bool",
            TypeList(vec![Type::String, Type::Int, Type::Bool]),
        );
        test_valid_word(
            "str , int,  bool,",
            TypeList(vec![Type::String, Type::Int, Type::Bool]),
        );
        test_invalid_word("");
        test_invalid_word("str int");
        test_invalid_word(",str");
    }

    #[test]
    fn test_field_parser() {
        let parser = grammar::FieldParser::new();

        let test_valid_word = |word: &str, expected: Field| {
            let res = parser.parse(word);
            assert_eq!(
                res,
                Ok(expected),
                "Field parser failed on: {:?}, got: {:?}",
                word,
                res
            );
        };
        let test_invalid_word = |word: &str| {
            let res = parser.parse(word);
            assert!(
                res.is_err(),
                "Field parser incorrectly passed on: {:?}, got: {:?}",
                word,
                res
            );
        };

        test_valid_word("this: bool", Field(Id(String::from("this")), Type::Bool));
        test_valid_word(
            "this: This",
            Field(
                Id(String::from("this")),
                Type::Struct(TitleId(String::from("This"))),
            ),
        );
        test_invalid_word("");
        test_invalid_word("feild this: bool");
        test_invalid_word("this bool");
        test_invalid_word("this: bool;");
        test_invalid_word("");
    }

    #[test]
    fn test_field_list_parser() {
        let parser = grammar::FieldListParser::new();

        let test_valid_word = |word: &str, expected: FieldList| {
            let res = parser.parse(word);
            assert_eq!(
                res,
                Ok(expected),
                "FieldList parser failed on: {:?}, got: {:?}",
                word,
                res
            );
        };
        let test_invalid_word = |word: &str| {
            let res = parser.parse(word);
            assert!(
                res.is_err(),
                "FieldList parser incorrectly passed on: {:?}, got: {:?}",
                word,
                res
            );
        };

        test_valid_word(
            "this: bool,that: int",
            FieldList(vec![
                Field(Id(String::from("this")), Type::Bool),
                Field(Id(String::from("that")), Type::Int),
            ]),
        );
        test_valid_word(
            "this: This,",
            FieldList(vec![Field(
                Id(String::from("this")),
                Type::Struct(TitleId(String::from("This"))),
            )]),
        );
        test_invalid_word("");
        test_invalid_word(",this: bool,");
        test_invalid_word("this: bool;");
    }

    #[test]
    fn test_action_parser() {
        let parser = grammar::ActionParser::new();

        let test_valid_word = |word: &str, expected: Action| {
            let res = parser.parse(word);
            assert_eq!(
                res,
                Ok(expected),
                "Action parser failed on: {:?}, got: {:?}",
                word,
                res
            );
        };
        let test_invalid_word = |word: &str| {
            let res = parser.parse(word);
            assert!(
                res.is_err(),
                "Action parser incorrectly passed on: {:?}, got: {:?}",
                word,
                res
            );
        };

        test_valid_word(
            "action this(str, int) -> That fallback deny",
            Action(
                Id(String::from("this")),
                TypeList(vec![Type::String, Type::Int]),
                Type::Struct(TitleId(String::from("That"))),
                Fallback::Deny,
            ),
        );
        test_valid_word(
            "action this() -> That fallback deny",
            Action(
                Id(String::from("this")), 
                TypeList(Vec::new()),
                Type::Struct(TitleId(String::from("That"))),
                Fallback::Deny,
            ),
        );
        test_valid_word(
            "action this(bool,This,str) -> That fallback deny",
            Action(
                Id(String::from("this")),
                TypeList(vec![
                    Type::Bool,
                    Type::Struct(TitleId(String::from("This"))),
                    Type::String,
                ]),
                Type::Struct(TitleId(String::from("That"))),
                Fallback::Deny,
            ),
        );
        test_invalid_word("acton this()");
        test_invalid_word("action This(that)");
        test_invalid_word("action wow(wrong,bool)");
        test_invalid_word("action wow(bool);");
        test_invalid_word("action this(str, int)");
        test_invalid_word("action this(str, int) -> That");
        test_invalid_word("action this(str, int) ->");
        test_invalid_word("action this(str, int) fallback deny");
        test_invalid_word("");
    }

    #[test]
    fn test_action_group_parser() {
        let parser = grammar::ActionGParser::new();

        let test_valid_word = |word: &str, expected: ActionG| {
            let res = parser.parse(word);
            assert_eq!(
                res,
                Ok(expected),
                "ActionG parser failed on: {:?}, got: {:?}",
                word,
                res
            );
        };
        let test_invalid_word = |word: &str| {
            let res = parser.parse(word);
            assert!(
                res.is_err(),
                "ActionG parser incorrectly passed on: {:?}, got: {:?}",
                word,
                res
            );
        };

        test_valid_word(
            "actiongroup this = that,other",
            ActionG(
                Id(String::from("this")),
                IdList(vec![Id(String::from("that")), Id(String::from("other"))]),
            ),
        );
        test_valid_word(
            "actiongroup this = that,",
            ActionG(
                Id(String::from("this")),
                IdList(vec![Id(String::from("that"))]),
            ),
        );
        test_valid_word(
            "actiongroup this=that",
            ActionG(
                Id(String::from("this")),
                IdList(vec![Id(String::from("that"))]),
            ),
        );
        test_invalid_word("actiongroup this=");
        test_invalid_word("actiongroup =");
        test_invalid_word("actiongroup =that");
        test_invalid_word("actiongroup this=,that");
        test_invalid_word("taggroup this=that");
        test_invalid_word("");
    }

    #[test]
    fn test_fallback_parser() {
        let parser = grammar::FallbackParser::new();

        let test_valid_word = |word: &str, expected: Fallback| {
            let res = parser.parse(word);
            assert_eq!(
                res,
                Ok(expected),
                "Fallback parser failed on: {:?}, got: {:?}",
                word,
                res
            );
        };
        let test_invalid_word = |word: &str| {
            let res = parser.parse(word);
            assert!(
                res.is_err(),
                "Fallback parser incorrectly passed on: {:?}, got: {:?}",
                word,
                res
            );
        };

        test_valid_word("allow", Fallback::Allow);
        test_valid_word("deny", Fallback::Deny);
        test_valid_word("warn", Fallback::Warn);
        test_invalid_word("");
        test_invalid_word("allow this");
    }

    #[test]
    fn test_expr_parser() {
        let parser = grammar::ExprParser::new();

        let test_valid_word = |word: &str, expected: Expr| {
            let res = parser.parse(word);
            assert_eq!(
                res,
                Ok(Box::new(expected)),
                "Expr parser failed on: {:?}, got: {:?}",
                word,
                res
            );
        };
        let test_invalid_word = |word: &str| {
            let res = parser.parse(word);
            assert!(
                res.is_err(),
                "Expr parser incorrectly passed on: {:?}, got: {:?}",
                word,
                res
            );
        };

        test_valid_word("93", Expr::Num(93));
        test_valid_word("-5", Expr::Neg(Box::new(Expr::Num(5))));
        test_valid_word(
            "1+2",
            Expr::Add(Box::new(Expr::Num(1)), Box::new(Expr::Num(2))),
        );
        test_valid_word(
            "10/2",
            Expr::Div(Box::new(Expr::Num(10)), Box::new(Expr::Num(2))),
        );
        test_valid_word(
            "1+2*3",
            Expr::Add(
                Box::new(Expr::Num(1)),
                Box::new(Expr::Mul(Box::new(Expr::Num(2)), Box::new(Expr::Num(3)))),
            ),
        );
        test_valid_word(
            "(1+2)*3",
            Expr::Mul(
                Box::new(Expr::Add(Box::new(Expr::Num(1)), Box::new(Expr::Num(2)))),
                Box::new(Expr::Num(3)),
            ),
        );
        test_valid_word(
            "10-3-2",
            Expr::Sub(
                Box::new(Expr::Sub(Box::new(Expr::Num(10)), Box::new(Expr::Num(3)))),
                Box::new(Expr::Num(2)),
            ),
        );
        test_valid_word(
            "-1+2",
            Expr::Add(
                Box::new(Expr::Neg(Box::new(Expr::Num(1)))),
                Box::new(Expr::Num(2)),
            ),
        );
        test_valid_word(
            "--1",
            Expr::Neg(Box::new(Expr::Neg(Box::new(Expr::Num(1))))),
        );
        test_valid_word(
            "1--2",
            Expr::Sub(
                Box::new(Expr::Num(1)),
                Box::new(Expr::Neg(Box::new(Expr::Num(2)))),
            ),
        );
        test_valid_word(
            "alsd",
            Expr::Field(FieldValue(vec![Id(String::from("alsd"))])),
        );
        test_valid_word(
            "alsd--2",
            Expr::Sub(
                Box::new(Expr::Field(FieldValue(vec![Id(String::from("alsd"))]))),
                Box::new(Expr::Neg(Box::new(Expr::Num(2)))),
            ),
        );
        test_valid_word(
            "this+2",
            Expr::Add(
                Box::new(Expr::Field(FieldValue(vec![Id(String::from("this"))]))),
                Box::new(Expr::Num(2)),
            ),
        );
        test_valid_word(
            "this+that.other",
            Expr::Add(
                Box::new(Expr::Field(FieldValue(vec![Id(String::from("this"))]))),
                Box::new(Expr::Field(FieldValue(vec![
                    Id(String::from("that")),
                    Id(String::from("other")),
                ]))),
            ),
        );
        test_valid_word(
            "\"this is a string09123 *&(£ 2938\"",
            Expr::String(String::from("this is a string09123 *&(£ 2938")),
        );
        test_valid_word(
            "`this is a valid regex`",
            Expr::Regex(String::from("this is a valid regex")),
        );
        test_valid_word(
            "`^(([a-f0-9]{32})+([a-zA-Z0-9=])?)+$`",
            Expr::Regex(String::from("^(([a-f0-9]{32})+([a-zA-Z0-9=])?)+$")),
        );
        test_valid_word("any_arg", Expr::AnyArg);
        test_valid_word("every_arg", Expr::EveryArg);

        test_invalid_word("1+");
        test_invalid_word("/2");
        test_invalid_word("(1+2)+3-4*((1-2+3)");
        test_invalid_word("");
        test_invalid_word("-");
        test_invalid_word("not+allow£d");
        test_invalid_word("invlid.");
        test_invalid_word(".tag");
        test_invalid_word("\"this is not a `valid string\"");
        test_invalid_word("\"this is not a \"valid string\"");
        test_invalid_word("\"this is not a \"valid regex\"");
        test_invalid_word("`this is not a `valid regex`");
    }

    // The following will contain tests which use the already tested parts of the parser to
    // generate AST subtrees. This is fine because it's bootstrapped with hand-checked examples
    // not circular dependancy so it suffices to check the structre of the otuput for composite
    // non-temrinals.

    #[test]
    fn test_struct_parser() {
        let parser = grammar::StructParser::new();
        let tag_parser = grammar::IdListParser::new();
        let field_parser = grammar::FieldListParser::new();

        let test_valid_word = |word: &str, expected: Struct| {
            let res = parser.parse(word);
            assert_eq!(
                res,
                Ok(expected),
                "Struct parser failed on: {:?}, got: {:?}",
                word,
                res
            );
        };
        let test_invalid_word = |word: &str| {
            let res = parser.parse(word);
            assert!(
                res.is_err(),
                "Struct parser incorrectly passed on: {:?}, got: {:?}",
                word,
                res
            );
        };

        test_valid_word(
            "struct Name [tag1,tag2] { f1: int, f2: int }",
            Struct(
                TitleId(String::from("Name")),
                tag_parser
                    .parse("tag1,tag2")
                    .expect("Failed to parse tags in test."),
                field_parser
                    .parse("f1: int, f2: int")
                    .expect("Failed to parse fields in test."),
            ),
        );
        test_valid_word(
            "struct Name [] { f1: int, f2: int }",
            Struct(
                TitleId(String::from("Name")),
                IdList(Vec::new()),
                field_parser
                    .parse("f1: int, f2: int")
                    .expect("Failed to parse fields in test."),
            ),
        );
        test_valid_word(
            "struct Name [] {}",
            Struct(
                TitleId(String::from("Name")),
                IdList(Vec::new()),
                FieldList(Vec::new()),
            ),
        );
        test_invalid_word("struct Name [,] {}");
        test_invalid_word("struct Name [] {,}");
        test_invalid_word("");
    }

    #[test]
    fn test_bool_expr_parser() {
        let parser = grammar::BoolExprParser::new();
        let expr_list_parser = grammar::ExprListParser::new();

        let test_valid_word = |word: &str, expected: BoolExpr| {
            let res = parser.parse(word);
            assert_eq!(
                res,
                Ok(Box::new(expected)),
                "BoolExpr parser failed on: {:?}, got: {:?}",
                word,
                res
            );
        };
        let test_invalid_word = |word: &str| {
            let res = parser.parse(word);
            assert!(
                res.is_err(),
                "BoolExpr parser incorrectly passed on: {:?}, got: {:?}",
                word,
                res
            );
        };

        test_valid_word("true", BoolExpr::True);
        test_valid_word("false", BoolExpr::False);
        test_valid_word(
            "false and false",
            BoolExpr::And(Box::new(BoolExpr::False), Box::new(BoolExpr::False)),
        );
        test_valid_word(
            "false or false",
            BoolExpr::Or(Box::new(BoolExpr::False), Box::new(BoolExpr::False)),
        );
        test_valid_word("not false", BoolExpr::Not(Box::new(BoolExpr::False)));
        test_valid_word(
            "false or false and false",
            BoolExpr::Or(
                Box::new(BoolExpr::False),
                Box::new(BoolExpr::And(
                    Box::new(BoolExpr::False),
                    Box::new(BoolExpr::False),
                )),
            ),
        );
        test_valid_word(
            "(false or false) and false",
            BoolExpr::And(
                Box::new(BoolExpr::Or(
                    Box::new(BoolExpr::False),
                    Box::new(BoolExpr::False),
                )),
                Box::new(BoolExpr::False),
            ),
        );
        test_valid_word(
            "false or not false and false",
            BoolExpr::Or(
                Box::new(BoolExpr::False),
                Box::new(BoolExpr::And(
                    Box::new(BoolExpr::Not(Box::new(BoolExpr::False))),
                    Box::new(BoolExpr::False),
                )),
            ),
        );
        test_valid_word(
            "this(that, other) allowed",
            BoolExpr::Rule(
                Id(String::from("this")),
                expr_list_parser
                    .parse("that,other")
                    .expect("Failed to parse IdList in test."),
            ),
        );
        test_valid_word(
            "this() allowed",
            BoolExpr::Rule(Id(String::from("this")), ExprList(Vec::new())),
        );
        test_valid_word(
            "5<4",
            BoolExpr::Lt(Box::new(Expr::Num(5)), Box::new(Expr::Num(4))),
        );
        test_valid_word(
            "this.that>4",
            BoolExpr::Gt(
                Box::new(Expr::Field(FieldValue(vec![
                    Id(String::from("this")),
                    Id(String::from("that")),
                ]))),
                Box::new(Expr::Num(4)),
            ),
        );
        test_valid_word(
            "this matches `abcd`",
            BoolExpr::Match(
                Box::new(Expr::Field(FieldValue(vec![Id(String::from("this"))]))),
                Box::new(Expr::Regex(String::from("abcd"))),
            ),
        );
        test_valid_word(
            "\"this\" matches `abcd`",
            BoolExpr::Match(
                Box::new(Expr::String(String::from("this"))),
                Box::new(Expr::Regex(String::from("abcd"))),
            ),
        );
        test_valid_word(
            "this contains other",
            BoolExpr::Contains(
                Box::new(Expr::Field(FieldValue(vec![Id(String::from("this"))]))),
                Id(String::from("other")),
            ),
        );
        test_valid_word(
            "this contains_all other",
            BoolExpr::ContainsAll(
                Box::new(Expr::Field(FieldValue(vec![Id(String::from("this"))]))),
                IdList(vec![Id(String::from("other"))]),
            ),
        );
        test_valid_word(
            "this contains_all other,that",
            BoolExpr::ContainsAll(
                Box::new(Expr::Field(FieldValue(vec![Id(String::from("this"))]))),
                IdList(vec![Id(String::from("other")), Id(String::from("that"))]),
            ),
        );

        test_invalid_word("");
        test_invalid_word("or this");
        test_invalid_word("this and");
        test_invalid_word("<4");
        test_invalid_word("5>=");
        test_invalid_word("5===4");
        test_invalid_word("this contains");
        test_invalid_word("this contains_all ,this");
        test_invalid_word("4 > this matches that");
    }

    #[test]
    fn test_condition_parser() {
        let parser = grammar::ConditionParser::new();

        let test_valid_word = |word: &str, expected: Condition| {
            let res = parser.parse(word);
            assert_eq!(
                res,
                Ok(expected),
                "Condition parser failed on: {:?}, got: {:?}",
                word,
                res
            );
        };
        let test_invalid_word = |word: &str| {
            let res = parser.parse(word);
            assert!(
                res.is_err(),
                "Condition parser incorrectly passed on: {:?}, got: {:?}",
                word,
                res
            );
        };

        test_valid_word("always", Condition::Always);
        test_valid_word("never", Condition::Never);
        test_valid_word("when false", Condition::When(Box::new(BoolExpr::False)));
        test_valid_word(
            "when this.that > 5 and other contains wow",
            Condition::When(Box::new(BoolExpr::And(
                Box::new(BoolExpr::Gt(
                    Box::new(Expr::Field(FieldValue(vec![
                        Id(String::from("this")),
                        Id(String::from("that")),
                    ]))),
                    Box::new(Expr::Num(5)),
                )),
                Box::new(BoolExpr::Contains(
                    Box::new(Expr::Field(FieldValue(vec![Id(String::from("other"))]))),
                    Id(String::from("wow")),
                )),
            ))),
        );

        test_invalid_word("when");
        test_invalid_word("");
    }

    #[test]
    fn test_rule_parser() {
        let parser = grammar::RuleParser::new();

        let test_valid_word = |word: &str, expected: Rule| {
            let res = parser.parse(word);
            assert_eq!(
                res,
                Ok(expected),
                "Rule parser failed on: {:?}, got: {:?}",
                word,
                res
            );
        };
        let test_invalid_word = |word: &str| {
            let res = parser.parse(word);
            assert!(
                res.is_err(),
                "Rule parser incorrectly passed on: {:?}, got: {:?}",
                word,
                res
            );
        };

        test_valid_word("allow always;", Rule::Allow(Condition::Always));
        test_valid_word("deny always;", Rule::Deny(Condition::Always));
        test_valid_word(
            "apply [name] always;",
            Rule::Apply(IdList(vec![Id(String::from("name"))]), Condition::Always),
        );
        test_valid_word(
            "apply [name1,name2] when 1 != 2;",
            Rule::Apply(
                IdList(vec![Id(String::from("name1")), Id(String::from("name2"))]),
                Condition::When(Box::new(BoolExpr::Neq(
                    Box::new(Expr::Num(1)),
                    Box::new(Expr::Num(2)),
                ))),
            ),
        );

        test_invalid_word("apply [name] always");
        test_invalid_word("apply [] always;");
        test_invalid_word("");
    }

    #[test]
    fn test_rule_block_parser() {
        let parser = grammar::RuleBlockParser::new();
        let args_parser = grammar::FieldListParser::new();

        let test_valid_word = |word: &str, expected: RuleBlock| {
            let res = parser.parse(word);
            assert_eq!(
                res,
                Ok(expected),
                "RuleBlock parser failed on: {:?}, got: {:?}",
                word,
                res
            );
        };
        let test_invalid_word = |word: &str| {
            let res = parser.parse(word);
            assert!(
                res.is_err(),
                "RuleBlock parser incorrectly passed on: {:?}, got: {:?}",
                word,
                res
            );
        };

        test_valid_word("rules name(this: str, that: int, other: Wow) { allow always; deny never; }", 
            RuleBlock(
                Id(String::from("name")), 
                args_parser.parse("this: str, that: int, other:Wow")
                                .expect("Failed to parse FieldList in test."), 
                Rules(vec![
                    Rule::Allow(Condition::Always),
                    Rule::Deny(Condition::Never),
                ])
            )
        );
        test_valid_word(
            "rules name() {}",
            RuleBlock(
                Id(String::from("name")),
                FieldList(Vec::new()),
                Rules(vec![]),
            ),
        );

        test_invalid_word("");
        test_invalid_word("rules name(,) {;}");
    }

    #[test]
    fn test_code_item_parser() {
        let parser = grammar::CodeItemParser::new();
        let tag_parser = grammar::TagParser::new();
        let tagg_parser = grammar::TagGParser::new();
        let struct_parser = grammar::StructParser::new();
        let action_parser = grammar::ActionParser::new();
        let actiong_parser = grammar::ActionGParser::new();
        let rule_block_parser = grammar::RuleBlockParser::new();

        let test_valid_word = |word: &str, expected: CodeItem| {
            let res = parser.parse(word);
            assert_eq!(
                res,
                Ok(expected),
                "RuleBlock parser failed on: {:?}, got: {:?}",
                word,
                res
            );
        };
        let test_invalid_word = |word: &str| {
            let res = parser.parse(word);
            assert!(
                res.is_err(),
                "RuleBlock parser incorrectly passed on: {:?}, got: {:?}",
                word,
                res
            );
        };

        test_valid_word(
            "tag this;",
            CodeItem::Tag(
                tag_parser
                    .parse("tag this")
                    .expect("Failed to parse Tag in test."),
            ),
        );
        test_valid_word(
            "taggroup this = that,other;",
            CodeItem::TagG(
                tagg_parser
                    .parse("taggroup this = that,other")
                    .expect("Failed to parse TagG in test."),
            ),
        );
        test_valid_word(
            "struct This [] {};",
            CodeItem::Struct(
                struct_parser
                    .parse("struct This [] {}")
                    .expect("Failed to parse Struct in test."),
            ),
        );
        test_valid_word(
            "action this() -> This fallback deny;",
            CodeItem::Action(
                action_parser
                    .parse("action this() -> This fallback deny")
                    .expect("Failed to parse Action in test."),
            ),
        );
        test_valid_word(
            "actiongroup this = that,other;",
            CodeItem::ActionG(
                actiong_parser
                    .parse("actiongroup this = that,other")
                    .expect("Failed to parse ActionG in test."),
            ),
        );
        test_valid_word(
            "rules name() {};",
            CodeItem::RuleBlock(
                rule_block_parser
                    .parse("rules name() {}")
                    .expect("Failed to parse RuleBlock in test."),
            ),
        );

        test_invalid_word("tag this");
        test_invalid_word("taggroup this = that,other");
        test_invalid_word("action this() -> This fallback deny");
        test_invalid_word("actiongroup this = that,other");
        test_invalid_word("struct This [] {}");
        test_invalid_word("rules this() {}");
        test_invalid_word("");
    }
}
