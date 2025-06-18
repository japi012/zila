use std::{
    env::Args,
    path::{Path, PathBuf},
};

enum CommandError {
    FlagExpectsArgument { flag: Box<str>, arguments: Box<str> },
    NoFileGiven,
    UnknownFlag,
}

#[derive(Debug)]
pub struct CommandResult {
    pub file: PathBuf,
    pub output_file: PathBuf,
    pub command_line_args: Vec<String>,
    pub program_name: PathBuf,
}

pub fn usage(program: &Path) {
    eprintln!(
        "usage: {} [OPTIONS] <file.zila>
  OPTIONS:
    -o <file>       Sets the name of the output assembly, object file, and executable",
        program.display()
    );
}

pub struct CommandParser {
    args: Args,
    file: Option<PathBuf>,
    output_file: Option<PathBuf>,
    program_name: PathBuf,
}

impl CommandParser {
    pub fn new() -> Self {
        let mut args = std::env::args();
        let program_name = args.next().expect("program name").into();

        Self {
            args,
            file: None,
            output_file: None,
            program_name,
        }
    }

    fn make_default(self, file: PathBuf) -> CommandResult {
        CommandResult {
            file,
            output_file: self.output_file.unwrap_or("output".into()),
            command_line_args: self.args.collect(),
            program_name: self.program_name,
        }
    }

    pub fn parse_commands(mut self) -> Result<CommandResult, ()> {
        while let Some(key) = self.args.next() {
            if let Some(flag) = key.strip_prefix('-') {
                match flag {
                    "o" => {
                        let Some(output_file) = self.args.next() else {
                            eprintln!("ERROR: `-o` flag expects argument <file>");
                            usage(&self.program_name);
                            return Err(());
                        };

                        if self.output_file.is_some() {
                            eprintln!("ERROR: multiple outputs specified");
                            usage(&self.program_name);
                            return Err(());
                        }

                        self.output_file = Some(output_file.into());
                    }
                    "-" => break,
                    _ => {
                        eprintln!("ERROR: unknown flag `{key}`");
                        usage(&self.program_name);
                        return Err(());
                    }
                }
            } else {
                if self.file.is_some() {
                    eprintln!("ERROR: multiple input files specified");
                    usage(&self.program_name);
                    return Err(());
                }

                self.file = Some(key.into());
            }
        }

        if let Some(ref file) = self.file {
            let file = file.clone();
            Ok(self.make_default(file))
        } else {
            eprintln!("ERROR: no file given");
            usage(&self.program_name);
            return Err(());
        }
    }
}
