use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::Ident;

use crate::{
    ast::{Fallback, Field, FieldList, FieldValue, Id, TitleId, Type},
    ir::{
        self, ActionName, ActionRules, Applications, BoolExpr, Code, Condition, Expr, ExprList,
        MathExpr, Regex, RegexId, StringExpr, Struct, StructName, Tag, TagBoolOp, TagExpr, TagList,
        TagSetId,
    },
};

fn emit_id(s: &str) -> TokenStream {
    let s = Ident::new(s, Span::call_site());
    quote! { #s }
}

fn emit_tag(t: &str) -> TokenStream {
    let t = emit_id(t);
    quote! { Tag::#t }
}

fn emit_tag_set(t: &TagSetId) -> TokenStream {
    emit_id(&format!("TAG_SET_{}", t.0))
}

fn emit_regex(i: &RegexId) -> TokenStream {
    emit_id(&format!("REGEX_{}", i.0))
}

fn emit_fallback(fallback: &Fallback) -> TokenStream {
    match fallback {
        Fallback::Allow => quote! { PolicyDecision::Allow("fallback") },
        Fallback::Deny => quote! { PolicyDecision::Deny("fallback") },
        Fallback::Warn => quote! { PolicyDecision::Warn },
    }
}
fn emit_boilerplate() -> TokenStream {
    quote! {
        #![allow(unused_parens)]

        use std::sync::LazyLock;
        use regex::Regex;

        const STATIC_REGEX_COMPILE_ERROR: &'static str = "Valid regex from transpilation";

        #[derive(Debug)]
        pub enum PolicyDecision {
            Allow(&'static str),
            Deny(&'static str),
            Warn,
        }

        macro_rules! as_item {
            ($i:item) => { $i };
        }

        macro_rules! count {
            () => { 0 };
            ($head:ident $(, $tail:ident)*) => { 1 + count! { $($tail),* } };
        }

        macro_rules! build_enum {
            () => {
                as_item! {
                    #[allow(non_camel_case_types)]
                    #[derive(Debug)]
                    pub enum Tag {}
                }
                const NUM_TAGS: usize = 0;
            };
            ($($name:ident),* $(,)?) => {
                as_item! {
                    #[allow(non_camel_case_types)]
                    #[derive(Debug)]
                    #[repr(usize)]
                    pub enum Tag { $($name),* }
                }
                const NUM_TAGS: usize = count! { $($name),* };
            };
        }

        macro_rules! build_tag_impls {
            ($($name:ident),* $(,)?) => {
                impl TryFrom<&str> for Tag {
                    type Error = String;

                    fn try_from(value: &str) -> Result<Self, Self::Error> {
                        match value {
                            $(
                                stringify!($name) => Ok(Tag::$name),
                            )*
                            _ => Err(format!("unknown tag: {}", value)),
                        }
                    }
                }
            };
        }

        macro_rules! build_tags {
            ($($body:tt)*) => {
                build_enum! {
                    $($body)*
                }
                build_tag_impls! {
                    $($body)*
                }
            };
        }

        macro_rules! build_static_set {
            ($name:ident; $($tag:ident),* $(,)?) => {
                const $name: [Tag; count! { $($tag),* }] = [$(Tag::$tag),*];
            };
        }

        macro_rules! build_static_regex {
            ($name:ident; $reg:literal) => {
                static $name: LazyLock<Regex> = LazyLock::new(|| {
                    Regex::new($reg).expect(STATIC_REGEX_COMPILE_ERROR)
                });
            };
        }

        #[derive(Debug)]
        pub struct TagSet([u8; NUM_TAGS]);

        impl From<Vec<Tag>> for TagSet {
            fn from(value: Vec<Tag>) -> Self {
                TagSet::new_with(value)
            }
        }

        impl TagSet {
            pub fn new() -> Self {
                Self([0; NUM_TAGS])
            }

            pub fn new_with(tags: Vec<Tag>) -> Self {
                let mut new = Self::new();
                for t in tags {
                    new.0[t as usize] = 1;
                }
                new
            }

            pub fn union_of(sets: &Vec<TagSet>) -> TagSet {
                let mut new = TagSet::new();
                for set in sets {
                    new.union(set);
                }
                new
            }

            pub fn add(&mut self, tag: Tag) {
                self.0[tag as usize] = 1;
            }

            pub fn union(&mut self, tags: &TagSet) {
                for (idx, tag) in tags.0.iter().enumerate() {
                    self.0[idx] |= tag;
                }
            }

            pub fn contains(&self, tag: Tag) -> bool {
                self.0[tag as usize] == 1
            }

            pub fn contains_all<const N: usize>(&self, tags: [Tag;N]) -> bool {
                let mut res = true;
                for tag in tags {
                    res &= self.0[tag as usize] == 1;
                }
                res
            }

            pub fn contains_any<const N: usize>(&self, tags: [Tag;N]) -> bool {
                let mut res = false;
                for tag in tags {
                    res |= self.0[tag as usize] == 1;
                }
                res
            }

            pub fn lacks(&self, tag: Tag) -> bool {
                !self.contains(tag)
            }

            pub fn lacks_all<const N: usize>(&self, tags: [Tag;N]) -> bool {
                !self.contains_any(tags)
            }

            pub fn lacks_any<const N: usize>(&self, tags: [Tag;N]) -> bool {
                !self.contains_all(tags)
            }
        }

        trait Tagged {
            fn tags(&self) -> &TagSet;
        }

        pub trait TagComparison {
            fn contains(&self, tag: Tag) -> bool;
            fn contains_all<const N: usize>(&self, tags: [Tag;N]) -> bool;
            fn contains_any<const N: usize>(&self, tags: [Tag;N]) -> bool;
            fn lacks(&self, tag: Tag) -> bool;
            fn lacks_all<const N: usize>(&self, tags: [Tag;N]) -> bool;
            fn lacks_any<const N: usize>(&self, tags: [Tag;N]) -> bool;
        }

        impl<T: Tagged> TagComparison for T {

            fn contains(&self, tag: Tag) -> bool {
                self.tags().contains(tag)
            }
            fn contains_all<const N: usize>(&self, tags: [Tag;N]) -> bool {
                self.tags().contains_all(tags)
            }
            fn contains_any<const N: usize>(&self, tags: [Tag;N]) -> bool {
                self.tags().contains_any(tags)
            }
            fn lacks(&self, tag: Tag) -> bool {
                self.tags().lacks(tag)
            }
            fn lacks_all<const N: usize>(&self, tags: [Tag;N]) -> bool {
                self.tags().lacks_all(tags)
            }
            fn lacks_any<const N: usize>(&self, tags: [Tag;N]) -> bool {
                self.tags().lacks_any(tags)
            }
        }

    }
}

pub fn emit_code(code: Code) -> TokenStream {
    let boilerplate = emit_boilerplate();

    let tags = emit_tags_enum(code.tags);
    let tag_sets: Vec<TokenStream> = code
        .tag_sets
        .iter()
        .enumerate()
        .map(|(idx, set)| emit_tag_set_defn(idx, set))
        .collect();
    let regexes: Vec<TokenStream> = code
        .regexes
        .iter()
        .enumerate()
        .map(|(idx, r)| emit_regex_defn(idx, r))
        .collect();
    let structs: Vec<TokenStream> = code.structs.iter().map(|s| emit_struct(s)).collect();
    let functions: Vec<TokenStream> = code.action_rules.iter().map(|a| emit_function(a)).collect();

    quote! {
        #boilerplate
        #tags
        #(#tag_sets)*
        #(#regexes)*
        #(#structs)*
        #(#functions)*
    }
}

fn emit_tags_enum(tags: Vec<Tag>) -> TokenStream {
    let tags: Vec<_> = tags.iter().map(|Tag(t)| emit_id(t)).collect();

    quote! {
        build_tags! {
            #(#tags),*
        }
    }
}

fn emit_tags(tags: &Vec<Tag>) -> TokenStream {
    let tags: Vec<_> = tags.iter().map(|Tag(t)| emit_tag(t)).collect();

    quote! {
        #(#tags),*
    }
}

fn emit_tag_set_defn(idx: usize, tags: &Vec<Tag>) -> TokenStream {
    let name = emit_id(&format!("TAG_SET_{idx}"));
    let tags: Vec<_> = tags.iter().map(|Tag(t)| emit_id(t)).collect();
    quote! {
        build_static_set! {
            #name;
            #(#tags),*
        }
    }
}

fn emit_regex_defn(idx: usize, reg: &Regex) -> TokenStream {
    let name = emit_id(&format!("REGEX_{idx}"));
    let Regex(reg) = reg;
    quote! {
        build_static_regex! {
            #name;
            #reg
        }
    }
}

fn emit_type(typ: &Type) -> TokenStream {
    let typ = emit_id(match typ {
        Type::Bool => "bool",
        Type::Int => "i32",
        Type::String => "String",
        Type::Struct(TitleId(s)) => s,
    });

    quote! { #typ }
}

fn emit_fields(fields: &Vec<Field>) -> TokenStream {
    let fields: Vec<_> = fields
        .iter()
        .map(|Field(Id(name), typ)| {
            let name = emit_id(name);
            let typ = emit_type(typ);
            quote! { #name: #typ }
        })
        .collect();

    quote! {
        #(#fields),*
    }
}

fn emit_struct(s: &Struct) -> TokenStream {
    let Struct(StructName(name), TagList(tags), FieldList(fields)) = s;
    let name = emit_id(name);
    let tags = emit_tags(tags);
    let field_names: Vec<_> = fields
        .iter()
        .map(|Field(Id(name), _)| emit_id(name))
        .collect();
    let fields = emit_fields(fields);

    quote! {
        #[derive(Debug)]
        pub struct #name {
            #fields,
            tags: TagSet,
        }
        impl #name {
            pub fn new(#fields, tags: Option<TagSet>) -> Self {
                let mut preset = TagSet::new_with(vec![#tags]);
                if let Some(tags) = tags {
                    preset.union(&tags)
                };
                Self {
                    #(#field_names),*,
                    tags: preset
                }
            }

            pub fn tag(&mut self, tags: impl Into<TagSet>) {
                self.tags.union(&tags.into());
            }
        }
        impl Tagged for #name {
            fn tags(&self) -> &TagSet {
                &self.tags
            }
        }
    }
}

fn emit_function(action: &ActionRules) -> TokenStream {
    let ActionRules(
        ActionName(name),
        FieldList(fields),
        _ret,
        fallback,
        allow_conds,
        deny_conds,
        Applications(applications),
    ) = action;

    let can_name = emit_id(&format!("can_{name}"));
    let after_name = emit_id(&format!("after_{name}"));
    let fields = emit_fields(fields);
    let fallback = emit_fallback(fallback);
    let deny_checks: Vec<_> = deny_conds
        .iter()
        .map(|lc| {
            let cond = emit_condition(&lc.condition);
            let label = &lc.label;
            quote! {
                if #cond {
                    return PolicyDecision::Deny(#label);
                }
            }
        })
        .collect();
    let allow_checks: Vec<_> = allow_conds
        .iter()
        .map(|lc| {
            let cond = emit_condition(&lc.condition);
            let label = &lc.label;
            quote! {
                if #cond {
                    return PolicyDecision::Allow(#label);
                }
            }
        })
        .collect();
    let applications: Vec<_> = applications
        .iter()
        .map(|(TagList(tags), cond)| {
            let tags = emit_tags(tags);
            let cond = emit_condition(cond);
            quote! {
                if #cond {
                    to_add.append(&mut vec![#tags]);
                }
            }
        })
        .collect();

    quote! {
        pub fn #can_name(#fields) -> PolicyDecision {
            #(#deny_checks)*
            #(#allow_checks)*
            #fallback
        }
        pub fn #after_name() -> Vec<Tag> {
            let mut to_add: Vec<Tag> = Vec::new();
            #(#applications)*
            to_add
        }
    }
}

pub fn emit_condition(cond: &Condition) -> TokenStream {
    match cond {
        Condition::Always => quote! { true },
        Condition::Never => quote! { false },
        Condition::When(e) => emit_bool_expr(e),
    }
}

fn emit_bool_expr(expr: &BoolExpr) -> TokenStream {
    match expr {
        BoolExpr::Or(a, b) => {
            let a = emit_bool_expr(a);
            let b = emit_bool_expr(b);
            quote! { (#a || #b) }
        }
        BoolExpr::And(a, b) => {
            let a = emit_bool_expr(a);
            let b = emit_bool_expr(b);
            quote! { (#a && #b) }
        }
        BoolExpr::Not(a) => {
            let a = emit_bool_expr(a);
            quote! { (!#a) }
        }
        BoolExpr::Rule(ir::Id(id), ExprList(exprs)) => {
            let func = emit_id(&format!("can_{id}"));
            let exprs: Vec<_> = exprs.iter().map(|e| emit_expr(e)).collect();
            quote! { matches!(#func(#(#exprs),*), PolicyDecision::Allow) }
        }
        BoolExpr::Gt(a, b) => {
            let a = emit_math_expr(a);
            let b = emit_math_expr(b);

            quote! { (#a > #b) }
        }
        BoolExpr::Lt(a, b) => {
            let a = emit_math_expr(a);
            let b = emit_math_expr(b);

            quote! { (#a < #b) }
        }
        BoolExpr::Gte(a, b) => {
            let a = emit_math_expr(a);
            let b = emit_math_expr(b);

            quote! { (#a >= #b) }
        }
        BoolExpr::Lte(a, b) => {
            let a = emit_math_expr(a);
            let b = emit_math_expr(b);

            quote! { (#a <= #b) }
        }
        BoolExpr::Eq(a, b) => {
            let a = emit_expr(a);
            let b = emit_expr(b);

            quote! { (#a == #b) }
        }
        BoolExpr::Neq(a, b) => {
            let a = emit_expr(a);
            let b = emit_expr(b);

            quote! { (#a != #b) }
        }
        BoolExpr::Match(s, r) => {
            let s = emit_string_expr(s);
            let r = emit_regex(r);

            quote! { #r.is_match(#s) }
        }
        BoolExpr::Contains(a, Tag(b)) => {
            let b = emit_tag(b);
            emit_tag_bool(a, TagBoolOp::Contains, b)
        }
        BoolExpr::ContainsAll(a, b) => {
            let b = emit_tag_set(b);
            emit_tag_bool(a, TagBoolOp::ContainsAll, b)
        }
        BoolExpr::ContainsAny(a, b) => {
            let b = emit_tag_set(b);
            emit_tag_bool(a, TagBoolOp::ContainsAny, b)
        }
        BoolExpr::Lacks(a, Tag(b)) => {
            let b = emit_tag(b);
            emit_tag_bool(a, TagBoolOp::Lacks, b)
        }
        BoolExpr::LacksAll(a, b) => {
            let b = emit_tag_set(b);
            emit_tag_bool(a, TagBoolOp::LacksAll, b)
        }
        BoolExpr::LacksAny(a, b) => {
            let b = emit_tag_set(b);
            emit_tag_bool(a, TagBoolOp::LacksAny, b)
        }
        BoolExpr::True => quote! { true },
        BoolExpr::False => quote! { false },
    }
}

fn emit_tag_bool(expr: &TagExpr, op: TagBoolOp, arg: TokenStream) -> TokenStream {
    let func = match op {
        TagBoolOp::Contains => emit_id("contains"),
        TagBoolOp::ContainsAll => emit_id("contains_all"),
        TagBoolOp::ContainsAny => emit_id("contains_any"),
        TagBoolOp::Lacks => emit_id("lacks"),
        TagBoolOp::LacksAll => emit_id("lacks_all"),
        TagBoolOp::LacksAny => emit_id("lacks_any"),
    };

    match expr {
        TagExpr::Field(FieldValue(fields)) => {
            let f = emit_field_value(fields);
            quote! { #f.#func(#arg) }
        }
        TagExpr::Any(v) => {
            let v: Vec<_> = v
                .iter()
                .map(|ir::Id(i)| {
                    let i = emit_id(i);
                    quote! { #i.#func(#arg) }
                })
                .collect();

            quote! { (#(#v)||*) }
        }
        TagExpr::Every(v) => {
            let v: Vec<_> = v
                .iter()
                .map(|ir::Id(i)| {
                    let i = emit_id(i);
                    quote! { #i.#func(#arg) }
                })
                .collect();

            quote! { (#(#v)&&*) }
        }
    }
}

fn emit_field_value(fields: &Vec<Id>) -> TokenStream {
    let fields: Vec<_> = fields.iter().map(|Id(f)| emit_id(f)).collect();
    quote! { #(#fields).* }
}

fn emit_expr(expr: &Expr) -> TokenStream {
    match expr {
        Expr::Maths(expr) => emit_math_expr(expr),
        Expr::String(expr) => emit_string_expr(expr),
        Expr::Field(FieldValue(fields)) => emit_field_value(fields),
    }
}

fn emit_math_expr(expr: &MathExpr) -> TokenStream {
    match expr {
        MathExpr::Add(a, b) => {
            let a = emit_math_expr(a);
            let b = emit_math_expr(b);
            quote! { (#a + #b) }
        }
        MathExpr::Sub(a, b) => {
            let a = emit_math_expr(a);
            let b = emit_math_expr(b);
            quote! { (#a - #b) }
        }
        MathExpr::Mul(a, b) => {
            let a = emit_math_expr(a);
            let b = emit_math_expr(b);
            quote! { (#a * #b) }
        }
        MathExpr::Div(a, b) => {
            let a = emit_math_expr(a);
            let b = emit_math_expr(b);
            quote! { (#a / #b) }
        }
        MathExpr::Neg(a) => {
            let a = emit_math_expr(a);
            quote! { (-#a) }
        }
        MathExpr::Num(n) => quote! { #n },
        MathExpr::Field(FieldValue(fields)) => emit_field_value(fields),
    }
}

fn emit_string_expr(expr: &StringExpr) -> TokenStream {
    match expr {
        StringExpr::String(s) => quote! { #s },
        StringExpr::Field(FieldValue(fields)) => emit_field_value(fields),
    }
}
