use lalrpop_util::lalrpop_mod;
use proc_macro2::TokenStream;
use regex;
use structopt::StructOpt;
use std::{fs,io,path::{Path,PathBuf}};

use crate::ast::Code;

lalrpop_mod!(
    #[allow(clippy::ptr_arg)]
    #[rustfmt::skip]
    grammar
);

mod ast;
mod check;
mod collect;
mod emit;
mod gen_ir;
mod ir;

#[derive(Debug, StructOpt)]
struct Opt {    
    #[structopt(short, long, parse(from_os_str))]
    file: PathBuf,
    
    #[structopt(short, long, parse(from_os_str), default_value = ".")]
    parent: PathBuf,

    #[structopt(long, default_value = "policies")]
    name: String,
}


#[derive(Debug)]
pub enum CompilerError {
    #[allow(dead_code)]
    AlreadyDefined(String),
    #[allow(dead_code)]
    Undefined(String),
    #[allow(dead_code)]
    Cycle(String),
    #[allow(dead_code)]
    TypeError(String),
    #[allow(dead_code)]
    InvalidRegex(String),
    #[allow(dead_code)]
    ValueError(String),
    #[allow(dead_code)]
    Io(io::Error),
    #[allow(dead_code)]
    Formatting(syn::Error),
}

fn pre_process(dirty_input: String) -> String {
    // Remove Comments
    let comments_re = regex::Regex::new(r"//[^\n\r]*[\n\r]*").expect("This is a valid regex.");
    let clean_input = comments_re.replace_all(&dirty_input, " ").into_owned();
    clean_input
}

fn compile(file_input: String) -> TokenStream {
    let cleaned_input = pre_process(file_input);

    let code_parser = grammar::CodeParser::new();
    let Code(lines) = code_parser
        .parse(&cleaned_input)
        .inspect_err(|e| {
            panic!("Failed to parse: {e:?}");
        })
        .unwrap();

    let (collected_code, identifiers) = collect::collect_code(&lines)
        .inspect_err(|e| {
            panic!("Failed to collect: {e:?}");
        })
        .unwrap();

    let generation_context = check::validate_code(&collected_code, identifiers)
        .inspect_err(|e| {
            panic!("Validation failed: {e:?}");
        })
        .unwrap();

    let ir_ast = gen_ir::compile_ir(&collected_code, generation_context);

    emit::emit_code(ir_ast)
}

fn save_as_crate(code: TokenStream, parent: PathBuf, crate_name: &str) -> Result<(), CompilerError> {
    let val = prettyplease::unparse(
        &syn::parse_file(&code.to_string()).map_err(|e| CompilerError::Formatting(e))?,
    );

    let crate_dir = parent.join(Path::new(crate_name));
    let src_dir = crate_dir.join(Path::new("src"));

    fs::create_dir_all(&src_dir).map_err(|e| CompilerError::Io(e))?;
    fs::write(
        crate_dir.join(Path::new("Cargo.toml")),
        cargo_toml(crate_name),
    )
    .map_err(|e| CompilerError::Io(e))?;
    fs::write(src_dir.join(Path::new("lib.rs")), val).map_err(|e| CompilerError::Io(e))?;

    Ok(())
}

fn cargo_toml(name: &str) -> String {
    format!(
        r#"[package]
name = "{name}"
version = "0.0.1"
edition = "2024"

[dependencies]
regex = "1.12.2"
"#
    )
}

fn main() {
    let options = Opt::from_args();

    // Read the file contents
    let contents = fs::read_to_string(options.file.clone())
        .inspect_err(|e| {
            panic!("Error reading '{:?}': {e:?}", options.file);
        })
        .unwrap();

    let tokens = compile(contents);

    save_as_crate(tokens, options.parent, options.name.as_str()).inspect_err(|e| {
        panic!("Error saving '{}': {e:?}", options.name);
    })
    .unwrap();
}

#[cfg(test)]
mod test {
    use crate::pre_process;

    #[test]
    fn test_pre_process() {
        let input =
            String::from("this is code\n     as is this//but not this\n//nor is this\ncode again");
        assert_eq!(
            pre_process(input),
            String::from("this is code\n     as is this  code again"),
            "Preprocessing failed."
        );

        let input = String::from(
            "this is code\n\r     as is this//but not this\n\r//nor is this\n\rcode again",
        );
        assert_eq!(
            pre_process(input),
            String::from("this is code\n\r     as is this  code again"),
            "Preprocessing failed with carriage returns."
        );
    }
}
