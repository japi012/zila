use std::{fs::{self, File}, process::ExitCode};

mod analyzer;
mod compiler;
mod lexer;
mod x86_64gen;

fn main() -> ExitCode {
    let mut args = std::env::args();
    let _program_name = args.next().unwrap();

    let Some(path) = args.next() else {
        eprintln!("please provide a file name");
        return ExitCode::FAILURE;
    };

    let source = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };

    match compile(&path, &source) {
        Ok(_) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}

fn compile(_path: &str, source: &str) -> Result<(), ()> {
    use compiler::Compiler;
    use lexer::Lexer;

    let words = Lexer::new(source).collect::<Vec<_>>();
    analyzer::analyze(&words).map_err(|e| eprintln!("{e:?}"))?;

    let procs = Compiler::compile(&words);

    let mut file = File::create("output.asm").map_err(|e| eprintln!("{e}"))?;
    x86_64gen::Generator::generate(&procs, &mut file).map_err(|e| eprintln!("{e}"))?;

    Ok(())
}
