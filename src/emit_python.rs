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
        Fallback::Allow => quote! { PolicyDecision::Allow },
        Fallback::Deny => quote! { PolicyDecision::Deny("fallback") },
        Fallback::Warn => quote! { PolicyDecision::Warn },
    }
}

fn emit_boilerplate() -> TokenStream {
    quote! {
        #![allow(unused_parens)]

        use std::sync::LazyLock;
        use regex::Regex;
        use pyo3::prelude::*;
        use pyo3::types::{PyTuple, PyDict, PyCFunction};

        const STATIC_REGEX_COMPILE_ERROR: &'static str = "Valid regex from transpilation";

        #[derive(Debug)]
        enum PolicyDecision {
            Allow,
            Deny(&'static str),
            Warn,
        }

        macro_rules! count {
            () => { 0 };
            ($head:ident $(, $tail:ident)*) => { 1 + count! { $($tail),* } };
        }

        macro_rules! build_tags {
            () => {
                #[pyclass(from_py_object, eq, eq_int)]
                #[allow(non_camel_case_types)]
                #[derive(Debug, Clone, Copy, PartialEq)]
                #[repr(usize)]
                pub enum Tag {}

                const NUM_TAGS: usize = 0;

                impl TryFrom<&str> for Tag {
                    type Error = String;
                    fn try_from(value: &str) -> Result<Self, Self::Error> {
                        match value {
                            _ => Err(format!("unknown tag: {}", value)),
                        }
                    }
                }
            };
            ($($name:ident),* $(,)?) => {
                #[pyclass(from_py_object, eq, eq_int)]
                #[allow(non_camel_case_types)]
                #[derive(Debug, Clone, Copy, PartialEq)]
                #[repr(usize)]
                pub enum Tag { $($name),* }

                const NUM_TAGS: usize = count! { $($name),* };

                impl TryFrom<&str> for Tag {
                    type Error = String;
                    fn try_from(value: &str) -> Result<Self, Self::Error> {
                        match value {
                            $( stringify!($name) => Ok(Tag::$name), )*
                            _ => Err(format!("unknown tag: {}", value)),
                        }
                    }
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

        #[derive(Debug, Clone)]
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

            pub fn contains_all<const N: usize>(&self, tags: [Tag; N]) -> bool {
                let mut res = true;
                for tag in tags {
                    res &= self.0[tag as usize] == 1;
                }
                res
            }

            pub fn contains_any<const N: usize>(&self, tags: [Tag; N]) -> bool {
                let mut res = false;
                for tag in tags {
                    res |= self.0[tag as usize] == 1;
                }
                res
            }

            pub fn lacks(&self, tag: Tag) -> bool {
                !self.contains(tag)
            }

            pub fn lacks_all<const N: usize>(&self, tags: [Tag; N]) -> bool {
                !self.contains_any(tags)
            }

            pub fn lacks_any<const N: usize>(&self, tags: [Tag; N]) -> bool {
                !self.contains_all(tags)
            }
        }

        trait Tagged {
            fn tags(&self) -> &TagSet;
            fn tag(&mut self, tags: impl Into<TagSet>);
        }

        pub trait TagComparison {
            fn contains(&self, tag: Tag) -> bool;
            fn contains_all<const N: usize>(&self, tags: [Tag; N]) -> bool;
            fn contains_any<const N: usize>(&self, tags: [Tag; N]) -> bool;
            fn lacks(&self, tag: Tag) -> bool;
            fn lacks_all<const N: usize>(&self, tags: [Tag; N]) -> bool;
            fn lacks_any<const N: usize>(&self, tags: [Tag; N]) -> bool;
        }

        impl<T: Tagged> TagComparison for T {
            fn contains(&self, tag: Tag) -> bool {
                self.tags().contains(tag)
            }
            fn contains_all<const N: usize>(&self, tags: [Tag; N]) -> bool {
                self.tags().contains_all(tags)
            }
            fn contains_any<const N: usize>(&self, tags: [Tag; N]) -> bool {
                self.tags().contains_any(tags)
            }
            fn lacks(&self, tag: Tag) -> bool {
                self.tags().lacks(tag)
            }
            fn lacks_all<const N: usize>(&self, tags: [Tag; N]) -> bool {
                self.tags().lacks_all(tags)
            }
            fn lacks_any<const N: usize>(&self, tags: [Tag; N]) -> bool {
                self.tags().lacks_any(tags)
            }
        }
    }
}

pub fn emit_code(code: &Code, module_name: &str) -> TokenStream {
    let boilerplate = emit_boilerplate();
    let mod_name = emit_id(module_name);
    let mod_name_str = module_name;

    let tags = emit_tags_enum(&code.tags);
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
    let structs: Vec<TokenStream> = code.structs.iter().map(emit_struct).collect();
    let functions: Vec<TokenStream> = code.action_rules.iter().map(emit_function).collect();

    let struct_names: Vec<TokenStream> = code
        .structs
        .iter()
        .map(|Struct(StructName(name), _, _)| emit_id(name))
        .collect();
    let fn_names: Vec<TokenStream> = code
        .action_rules
        .iter()
        .map(|ActionRules(ActionName(name), _, _, _, _, _, _)| emit_id(&format!("try_{name}")))
        .collect();

    quote! {
        #boilerplate
        #tags
        #(#tag_sets)*
        #(#regexes)*

        pyo3::create_exception!(#mod_name_str, PolicyDenied, pyo3::exceptions::PyException);
        pyo3::create_exception!(#mod_name_str, PolicyWarned, pyo3::exceptions::PyException);

        #(#structs)*
        #(#functions)*

        #[pymodule]
        mod #mod_name {
            #[pymodule_export]
            use super::Tag;
            #(
                #[pymodule_export]
                use super::#struct_names;
            )*
            #(
                #[pymodule_export]
                use super::#fn_names;
            )*
            #[pymodule_export]
            use super::PolicyDenied;
            #[pymodule_export]
            use super::PolicyWarned;
        }
    }
}

fn emit_tags_enum(tags: &Vec<Tag>) -> TokenStream {
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

// Emit the Rust type used in pyclass struct fields and #[new] constructor params.
// pyo3 requires owned types for #[pyclass] fields: String not &str, i32 not &i32.
fn emit_py_type(typ: &Type) -> TokenStream {
    match typ {
        Type::Bool => quote! { bool },
        Type::Int => quote! { i32 },
        Type::String => quote! { String },
        Type::Struct(TitleId(s)) => emit_id(s),
    }
}

fn emit_struct(s: &Struct) -> TokenStream {
    let Struct(StructName(name), TagList(preset_tags), FieldList(fields)) = s;
    let name = emit_id(name);

    // Struct field declarations with pyo3 getters/setters (owned types)
    let field_decls: Vec<TokenStream> = fields
        .iter()
        .map(|Field(Id(n), typ)| {
            let n = emit_id(n);
            let t = emit_py_type(typ);
            quote! {
                #[pyo3(get, set)]
                pub #n: #t,
            }
        })
        .collect();

    // Constructor params (same owned types, no TagSet)
    let ctor_params: Vec<TokenStream> = fields
        .iter()
        .map(|Field(Id(n), typ)| {
            let n = emit_id(n);
            let t = emit_py_type(typ);
            quote! { #n: #t }
        })
        .collect();

    // Field names for Self { .. } initializer
    let field_names: Vec<TokenStream> = fields
        .iter()
        .map(|Field(Id(n), _)| emit_id(n))
        .collect();

    // Preset tags baked into this struct type from the DSL
    let preset_tag_vals: Vec<TokenStream> = preset_tags
        .iter()
        .map(|Tag(t)| emit_tag(t))
        .collect();

    // pyo3 signature string: (field1, field2, ..., tags=None)
    // Build as a literal for #[pyo3(signature = (...))]
    let sig_fields: Vec<TokenStream> = fields
        .iter()
        .map(|Field(Id(n), _)| emit_id(n))
        .collect();

    quote! {
        #[pyclass(from_py_object)]
        #[derive(Debug, Clone)]
        pub struct #name {
            #(#field_decls)*
            pub _tags: TagSet,
        }

        #[pymethods]
        impl #name {
            #[new]
            #[pyo3(signature = (#(#sig_fields),*, tags = None))]
            pub fn new(#(#ctor_params),*, tags: Option<Vec<Tag>>) -> Self {
                let mut tag_set = TagSet::new_with(vec![#(#preset_tag_vals),*]);
                if let Some(user_tags) = tags {
                    for t in user_tags {
                        tag_set.add(t);
                    }
                }
                Self {
                    #(#field_names),*,
                    _tags: tag_set,
                }
            }
        }

        impl Tagged for #name {
            fn tags(&self) -> &TagSet {
                &self._tags
            }
            fn tag(&mut self, tags: impl Into<TagSet>) {
                self._tags.union(&tags.into());
            }
        }
    }
}

fn emit_function(action: &ActionRules) -> TokenStream {
    let ActionRules(
        ActionName(name),
        FieldList(fields),
        ret,
        fallback,
        allow_conds,
        deny_conds,
        Applications(applications),
    ) = action;

    let try_name = emit_id(&format!("try_{name}"));
    let ret_type = emit_py_type(ret);
    let fallback_ts = emit_fallback(fallback);

    // Positional extraction from the PyTuple args: let foo: FooType = args.get_item(N)?.extract()?;
    let arg_extractions: Vec<TokenStream> = fields
        .iter()
        .enumerate()
        .map(|(i, Field(Id(n), typ))| {
            let n = emit_id(n);
            let t = emit_py_type(typ);
            quote! { let #n: #t = args.get_item(#i)?.extract()?; }
        })
        .collect();

    // Shadow owned names with refs inside the condition block only,
    // leaving owned bindings available outside for the body call.
    let ref_shadows: Vec<TokenStream> = fields
        .iter()
        .map(|Field(Id(n), _)| {
            let n = emit_id(n);
            quote! { let #n = &#n; }
        })
        .collect();

    let deny_checks: Vec<TokenStream> = deny_conds
        .iter()
        .map(|lc| {
            let cond = emit_condition(&lc.condition);
            let label = &lc.label;
            quote! {
                if #cond {
                    break 'can PolicyDecision::Deny(#label);
                }
            }
        })
        .collect();

    let allow_checks: Vec<TokenStream> = allow_conds
        .iter()
        .map(|c| {
            let cond = emit_condition(c);
            quote! {
                if #cond {
                    break 'can PolicyDecision::Allow;
                }
            }
        })
        .collect();

    let applications_ts: Vec<TokenStream> = applications
        .iter()
        .map(|(TagList(tags), cond)| {
            let tags_ts = emit_tags(tags);
            let cond_ts = emit_condition(cond);
            quote! {
                if #cond_ts {
                    to_add.append(&mut vec![#tags_ts]);
                }
            }
        })
        .collect();

    // Args passed into the Python body callable: owned values (moved out after checks)
    let call_args: Vec<TokenStream> = fields
        .iter()
        .map(|Field(Id(n), _)| emit_id(n))
        .collect();

    quote! {
        #[pyfunction]
        pub fn #try_name(py: Python<'_>, func: Py<PyAny>) -> Py<PyAny> {
            let wrapper = move |args: &Bound<'_, PyTuple>, _kwargs: Option<&Bound<'_, PyDict>>| -> PyResult<Py<PyAny>> {
                // Extract owned values from the Python args tuple
                #(#arg_extractions)*

                // Evaluate deny/allow conditions using references (owned values stay alive)
                let pd = {
                    #(#ref_shadows)*
                    'can: {
                        #(#deny_checks)*
                        #(#allow_checks)*
                        break 'can #fallback_ts;
                    }
                };

                let py = args.py();
                match pd {
                    PolicyDecision::Deny(s) => Err(PyErr::new::<PolicyDenied, _>(s)),
                    PolicyDecision::Warn => Err(PyErr::new::<PolicyWarned, _>("")),
                    PolicyDecision::Allow => {
                        // Pass owned values into the Python body callable
                        let mut res: #ret_type = func.bind(py).call1((#(#call_args),*,))?.extract()?;
                        let mut to_add: Vec<Tag> = Vec::new();
                        #(#applications_ts)*
                        res.tag(to_add);
                        Ok(res.into_pyobject(py)?.into_any().unbind())
                    }
                }
            };
            PyCFunction::new_closure(py, None, None, wrapper)
                .unwrap()
                .into_pyobject(py)
                .unwrap()
                .into_any()
                .unbind()
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
            let exprs: Vec<_> = exprs.iter().map(emit_expr).collect();
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
