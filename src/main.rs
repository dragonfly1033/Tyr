use lalrpop_util::lalrpop_mod;
use proc_macro2::TokenStream;
use regex;
use std::{
    fs, io,
    path::{Path, PathBuf},
};
use structopt::StructOpt;

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
mod emit_python;
mod emit_pyi;
mod gen_ir;
mod ir;
mod coverage;

#[derive(Debug, StructOpt)]
enum Opt {
    /// Compile a .tyr policy file into a Rust crate
    Compile {
        // tyr source file.
        #[structopt(short, long, parse(from_os_str))]
        file: PathBuf,

        // parent directory in which to output generated crate.
        #[structopt(short, long, parse(from_os_str), default_value = ".")]
        parent: PathBuf,

        // name of generated crate.
        #[structopt(long, default_value = "policies")]
        name: String,

        // generate pyo3 Python bindings.
        #[structopt(long)]
        python: bool,
    },
    /// Analyse coverage of a policy set, optionally comparing two
    Coverage {
        // file A. Generates coverage report when used alone.
        #[structopt(short, long, parse(from_os_str))]
        file: PathBuf,

        // file B. Generates a comparison report with file A. 
        #[structopt(long, parse(from_os_str))]
        compare: Option<PathBuf>,

        /// Solver timeout in milliseconds
        #[structopt(long, default_value = "5000")]
        timeout: u32,
    },
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

fn compile_to_ir(file_input: String) -> ir::Code {
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

    gen_ir::compile_ir(&collected_code, generation_context)
}

fn compile(ir_ast: &ir::Code, python: bool, name: &str) -> TokenStream {
    if python {
        emit_python::emit_code(ir_ast, name)
    } else {
        emit::emit_code(ir_ast)
    }
}

fn save_as_crate(
    code: TokenStream,
    pyi: Option<String>,
    parent: PathBuf,
    crate_name: &str,
    python: bool,
) -> Result<(), CompilerError> {
    let val = prettyplease::unparse(
        &syn::parse_file(&code.to_string()).map_err(|e| CompilerError::Formatting(e))?,
    );

    let crate_dir = parent.join(Path::new(crate_name));
    let src_dir = crate_dir.join(Path::new("src"));

    fs::create_dir_all(&src_dir).map_err(|e| CompilerError::Io(e))?;
    fs::write(
        crate_dir.join(Path::new("Cargo.toml")),
        cargo_toml(crate_name, python),
    )
    .map_err(|e| CompilerError::Io(e))?;
    fs::write(src_dir.join(Path::new("lib.rs")), val).map_err(|e| CompilerError::Io(e))?;

    if let Some(pyi_content) = pyi {
        fs::write(
            crate_dir.join(Path::new(&format!("{crate_name}.pyi"))),
            pyi_content,
        )
        .map_err(|e| CompilerError::Io(e))?;

        fs::write(
            crate_dir.join(Path::new(&format!("pyproject.toml"))),
            format!(r#"
[build-system]
requires = ["maturin>=1.0"]
build-backend = "maturin"

[project]
name = "{crate_name}"
version = "0.1.0"
requires-python = ">=3.8"

[tool.maturin]
include = ["{crate_name}.pyi"]
            "#),
        )
        .map_err(|e| CompilerError::Io(e))?;
    }

    Ok(())
}

fn cargo_toml(name: &str, python: bool) -> String {
    let pyo3_dep = if python {
        r#"pyo3 = { version = "0.28.2" }
"#
    } else {
        ""
    };
    let crate_type = if python {
        format!(r#"
[lib]
name = "{name}"
crate-type = ["cdylib"]
"#)
    } else {
        String::new()
    };
    format!(
        r#"[package]
name = "{name}"
version = "0.0.1"
edition = "2024"
{crate_type}
[dependencies]
regex = "1.12.2"
{pyo3_dep}"#
    )
}


fn read_file(path: &PathBuf) -> String {
    fs::read_to_string(path)
        .inspect_err(|e| {
            panic!("Error reading '{:?}': {e:?}", path);
        })
        .unwrap()
}

fn main() {
    match Opt::from_args() {
        Opt::Compile { file, parent, name, python } => {
            let contents = read_file(&file);
            let ir = compile_to_ir(contents);
            let pyi = if python { Some(emit_pyi::emit_pyi(&ir)) } else { None };
            let tokens = compile(&ir, python, name.as_str());

            save_as_crate(tokens, pyi, parent, name.as_str(), python)
                .inspect_err(|e| {
                    panic!("Error saving '{}': {e:?}", name);
                })
                .unwrap();
        }
        Opt::Coverage { file, compare: compare_path, timeout } => {
            let contents = read_file(&file);
            let ir = compile_to_ir(contents);

            if let Some(compare_path) = compare_path {
                let other_contents = read_file(&compare_path);
                let other_ir = compile_to_ir(other_contents);
                let result = coverage::compare(&ir, &other_ir, timeout);
                println!("{result}");
            } else {
                let reports = coverage::describe_coverage(&ir, timeout);
                for report in reports {
                    println!("{report}");
                    println!();
                }
            }
        }
    }
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
