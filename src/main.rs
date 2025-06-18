use std::{
    fs::{self, File},
    path::Path,
    process::{Command, ExitCode, ExitStatus},
};

mod analyzer;
mod command_parser;
mod compiler;
mod lexer;
mod x86_64gen;

fn main() -> ExitCode {
    use command_parser::CommandParser;
    let command_parser = CommandParser::new();

    let Ok(res) = command_parser.parse_commands() else {
        return ExitCode::FAILURE;
    };

    let source = match fs::read_to_string(&res.file) {
        Ok(source) => source,
        Err(e) => {
            eprintln!("ERROR: cannot read file `{}`: {e}", res.file.display());
            command_parser::usage(&res.program_name);
            return ExitCode::FAILURE;
        }
    };

    eprintln!("INFO: Compiling `{}`...", res.file.display(),);
    if compile(&res.file, &source, &res.output_file).is_err() {
        return ExitCode::FAILURE;
    }

    eprintln!(
        "INFO: Running `nasm {}.asm -felf64 -o {}.o`",
        res.output_file.display(),
        res.output_file.display()
    );
    if bomb(
        Command::new("nasm")
            .arg(format!("{}.asm", res.output_file.display()))
            .arg("-felf64")
            .arg("-o")
            .arg(format!("{}.o", res.output_file.display()))
            .status(),
    ) {
        return ExitCode::FAILURE;
    }

    eprintln!(
        "INFO: Running `ld -o {} {}.o`",
        res.output_file.display(),
        res.output_file.display()
    );
    if bomb(
        Command::new("ld")
            .arg("-o")
            .arg(&res.output_file)
            .arg(format!("{}.o", res.output_file.display()))
            .status(),
    ) {
        return ExitCode::FAILURE;
    }

    eprintln!("INFO: Generated `./{}`", res.output_file.display());

    ExitCode::SUCCESS
}

fn bomb<E>(r: Result<ExitStatus, E>) -> bool {
    match r {
        Err(_) => true,
        Ok(status) => !status.success(),
    }
}

fn compile(path: &Path, source: &str, output_path: &Path) -> Result<(), ()> {
    use analyzer::Analyzer;
    use compiler::Compiler;
    use lexer::Lexer;

    let words = Lexer::new(source).collect::<Vec<_>>();
    let defs = match Analyzer::analyze(words.iter().copied()) {
        Ok(res) => Ok(res),
        Err(err) => Err(
            analyzer::report_error(err, path, source, &mut std::io::stderr())
                .map_err(|e| eprintln!("{e}"))?,
        ),
    }?;

    let (main_proc, procs, string_literals) = Compiler::compile(defs);

    let mut file =
        File::create(format!("{}.asm", output_path.display())).map_err(|e| eprintln!("{e}"))?;
    x86_64gen::Generator::generate(
        main_proc.expect("no `main`"),
        &procs,
        &string_literals,
        &mut file,
    )
    .map_err(|e| eprintln!("{e}"))?;

    Ok(())
}
