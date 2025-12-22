use std::env;
use std::fs;
use std::process;
use lalrpop_util::lalrpop_mod;
use regex;

use crate::ast::Code;

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
    AlreadyDefined(String),
    #[allow(dead_code)]
    Undefined(String),
    #[allow(dead_code)]
    Cycle(String),
    #[allow(dead_code)]
    TypeError(String),
}

fn pre_process(dirty_input: String) -> String {
    // Remove Comments
    let comments_re = regex::Regex::new(r"//[^\n\r]*[\n\r]*").expect("This is a valid regex.");
    let clean_input = comments_re.replace_all(&dirty_input, " ").into_owned();
    clean_input
}

fn compile(file_input: String) {
    let cleaned_input = pre_process(file_input);

    let code_parser = grammar::CodeParser::new();
    let Code(lines) = code_parser.parse(&cleaned_input).inspect_err(|e| {
        panic!("Failed to parse: {e:?}");
    }).unwrap();

    println!("{lines:?}");

    let (collected_code, identifiers) = check::collect_code(&lines).inspect_err(|e| {
        panic!("Failed to collect: {e:?}");
    }).unwrap();

    let generation_context = check::validate_code(&collected_code, identifiers).inspect_err(|e| {
        panic!("Validation failed: {e:?}");
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

#[cfg(test)]
mod test {
    use crate::pre_process;

    #[test]
    fn test_pre_process() {
        let input = String::from("this is code\n     as is this//but not this\n//nor is this\ncode again");
        assert_eq!(
            pre_process(input), 
            String::from("this is code\n     as is this  code again"), 
            "Preprocessing failed."
        );

        let input = String::from("this is code\n\r     as is this//but not this\n\r//nor is this\n\rcode again");
        assert_eq!(
            pre_process(input), 
            String::from("this is code\n\r     as is this  code again"), 
            "Preprocessing failed with carriage returns."
        );
    }
}