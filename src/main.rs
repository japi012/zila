use std::process::ExitCode;

mod analyzer;
mod lexer;

fn main() -> ExitCode {
    use lexer::Lexer;

    let source = r#"1 1 +"#;
    let words = Lexer::new(source).collect::<Vec<_>>();
    match analyzer::analyze(&words) {
        Ok(_) => (),
        Err(e) => {
            eprintln!("{e:?}");
            return ExitCode::FAILURE;
        }
    }

    ExitCode::SUCCESS
}
