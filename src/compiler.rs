use crate::{
    analyzer::{Item, ItemKind, Type},
    lexer::Span,
};

impl Type {
    fn size(&self) -> Option<usize> {
        match self {
            Type::Var(_) | Type::MultiVar(_) => None,
            Type::Bool => Some(1),
            Type::Int => Some(1),
            Type::Quotation(_) => Some(1),
            Type::String => Some(2),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Instruction<'src> {
    PushInt(isize),
    PushBool(bool),
    PushString(usize),
    PushQuote(Label<'src>),

    Add,
    Sub,
    Mul,
    Div,

    Exit,

    Puts,

    Dup { size: usize },
    Swap { size_a: usize, size_b: usize },
    Drop { size: usize },
    Over { size_a: usize, size_b: usize },
    Apply,
    Branch { size: usize },
}

#[derive(Debug, Clone, Copy)]
pub struct Label<'src> {
    id: usize,
    name: Option<&'src str>,
}

impl<'src> Label<'src> {
    fn new(id: usize, name: Option<&'src str>) -> Self {
        Self { id, name }
    }

    pub fn id(&self) -> usize {
        self.id
    }

    pub fn name(&self) -> Option<&'src str> {
        self.name
    }
}

#[derive(Debug, Clone)]
pub struct Proc<'src> {
    label: Label<'src>,
    code: Vec<(Span, Instruction<'src>)>,
}

impl<'src> Proc<'src> {
    pub fn new(label: Label<'src>) -> Self {
        Self {
            label,
            code: Vec::new(),
        }
    }

    pub fn label(&self) -> Label {
        self.label
    }

    pub fn code(&self) -> &[(Span, Instruction<'src>)] {
        &self.code
    }
}

fn escape(s: &str) -> Box<str> {
    let mut escaped = String::new();

    let mut is_escaped = false;
    for c in s.chars() {
        if is_escaped {
            match c {
                'n' => escaped.push('\n'),
                '\\' => escaped.push('\\'),
                '"' => escaped.push('\"'),
                _ => (),
            }
        } else if c == '\\' {
            is_escaped = true;
        } else {
            escaped.push(c);
        }
    }

    escaped.into_boxed_str()
}

pub struct Compiler<'src> {
    procs: Vec<Proc<'src>>,
    string_literals: Vec<Box<str>>,
}

impl<'src> Compiler<'src> {
    pub fn new() -> Self {
        Self {
            procs: Vec::new(),
            string_literals: Vec::new(),
        }
    }

    pub fn compile<I: IntoIterator<Item = Item<'src>>>(
        items: I,
    ) -> (Vec<Proc<'src>>, Vec<Box<str>>) {
        let mut items = items.into_iter().peekable();
        let mut compiler = Self::new();
        let main_proc = compiler.new_proc(None);

        while let Some(item) = items.next() {
            compiler.compile_item_to_block(item, main_proc);
        }

        (compiler.procs, compiler.string_literals)
    }

    fn add_instruction(&mut self, label: Label<'src>, instruction: Instruction<'src>, span: Span) {
        self.procs[label.id].code.push((span, instruction))
    }

    fn new_proc(&mut self, name: Option<&'src str>) -> Label<'src> {
        let id = self.procs.len();
        let label = Label::new(id, name);

        let proc = Proc::new(label);
        self.procs.push(proc);

        label
    }

    fn compile_item_to_block(&mut self, item: Item<'src>, label: Label<'src>) {
        let (kind, span) = item.parts();
        match kind {
            ItemKind::Quotation(_, items) => {
                let quotation_proc = self.new_proc(None);

                for quotation_word in items {
                    self.compile_item_to_block(quotation_word, quotation_proc);
                }

                self.add_instruction(label, Instruction::PushQuote(quotation_proc), span);
            }

            ItemKind::Integer(i) => self.add_instruction(label, Instruction::PushInt(i), span),
            ItemKind::String(s) => {
                let string_id = self.string_literals.len();
                self.string_literals.push(escape(&s[1..s.len() - 2]));
                self.add_instruction(label, Instruction::PushString(string_id), span)
            }

            ItemKind::Word(_, "true") => {
                self.add_instruction(label, Instruction::PushBool(true), span)
            }
            ItemKind::Word(_, "false") => {
                self.add_instruction(label, Instruction::PushBool(false), span)
            }

            ItemKind::Word(_, "+") => self.add_instruction(label, Instruction::Add, span),
            ItemKind::Word(_, "-") => self.add_instruction(label, Instruction::Sub, span),
            ItemKind::Word(_, "*") => self.add_instruction(label, Instruction::Mul, span),
            ItemKind::Word(_, "/") => self.add_instruction(label, Instruction::Div, span),

            ItemKind::Word(_, "exit") => self.add_instruction(label, Instruction::Exit, span),

            ItemKind::Word(_, "puts") => self.add_instruction(label, Instruction::Puts, span),

            ItemKind::Word(sig, "dup") => {
                let (inputs, _) = sig.parts();
                self.add_instruction(
                    label,
                    Instruction::Dup {
                        size: inputs[0].size().unwrap(),
                    },
                    span,
                )
            }
            ItemKind::Word(sig, "drop") => {
                let (inputs, _) = sig.parts();
                self.add_instruction(
                    label,
                    Instruction::Drop {
                        size: inputs[0].size().unwrap(),
                    },
                    span,
                )
            }
            ItemKind::Word(sig, "swap") => {
                let (inputs, _) = sig.parts();
                self.add_instruction(
                    label,
                    Instruction::Swap {
                        size_a: inputs[0].size().unwrap(),
                        size_b: inputs[1].size().unwrap(),
                    },
                    span,
                )
            }
            ItemKind::Word(sig, "over") => {
                let (inputs, _) = sig.parts();
                self.add_instruction(
                    label,
                    Instruction::Over {
                        size_a: inputs[0].size().unwrap(),
                        size_b: inputs[1].size().unwrap(),
                    },
                    span,
                )
            }
            ItemKind::Word(_, "apply") => self.add_instruction(label, Instruction::Apply, span),
            ItemKind::Word(sig, "?") => {
                let (inputs, _) = sig.parts();
                self.add_instruction(
                    label,
                    Instruction::Branch {
                        size: inputs[0].size().unwrap(),
                    },
                    span,
                )
            }

            ItemKind::Word(_, s) => todo!("user defined words: {s}"),
        }
    }
}
