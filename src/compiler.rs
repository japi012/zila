use crate::lexer::{Token, Word};

#[derive(Debug, Clone, Copy)]
pub enum Instruction<'src> {
    PushInt(isize),
    PushBool(bool),
    PushQuote(Label<'src>),

    Add,
    Sub,
    Mul,
    Div,

    Exit,

    Dup,
    Swap,
    Drop,
    Over,
    Apply,
    Branch,
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
    code: Vec<(Word<'src>, Instruction<'src>)>,
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

    pub fn code(&self) -> &[(Word<'src>, Instruction<'src>)] {
        &self.code
    }
}

pub struct Compiler<'src> {
    words: &'src [Word<'src>],
    pos: usize,
    procs: Vec<Proc<'src>>,
}

impl<'src> Compiler<'src> {
    pub fn new(words: &'src [Word<'src>]) -> Self {
        Self {
            words,
            pos: 0,
            procs: Vec::new(),
        }
    }

    pub fn compile(words: &'src [Word<'src>]) -> Vec<Proc<'src>> {
        let mut compiler = Self::new(words);
        let main_proc = compiler.new_proc(None);

        while compiler.pos < words.len() {
            compiler.compile_word_to_block(main_proc);
        }

        compiler.procs
    }
    
    fn add_instruction(
        &mut self,
        label: Label<'src>,
        instruction: Instruction<'src>,
        word: Word<'src>,
    ) {
        self.procs[label.id].code.push((word, instruction))
    }

    fn current_word(&self) -> Word<'src> {
        self.words[self.pos]
    }

    fn new_proc(&mut self, name: Option<&'src str>) -> Label<'src> {
        let id = self.procs.len();
        let label = Label::new(id, name);

        let proc = Proc::new(label);
        self.procs.push(proc);

        label
    }

    fn compile_word_to_block(&mut self, label: Label<'src>) {
        let word = self.current_word();
        self.pos += 1;

        match word.token() {
            Token::Symbol("[") => {
                let quote_proc = self.new_proc(None);

                while !matches!(self.current_word().token(), Token::Symbol("]")) {
                    self.compile_word_to_block(quote_proc);
                }
                self.pos += 1;

                self.add_instruction(label, Instruction::PushQuote(quote_proc), word);
            }

            Token::Integer(i) => self.add_instruction(label, Instruction::PushInt(i), word),

            Token::Symbol("true") => self.add_instruction(label, Instruction::PushBool(true), word),
            Token::Symbol("false") => {
                self.add_instruction(label, Instruction::PushBool(false), word)
            }

            Token::Symbol("+") => self.add_instruction(label, Instruction::Add, word),
            Token::Symbol("-") => self.add_instruction(label, Instruction::Sub, word),
            Token::Symbol("*") => self.add_instruction(label, Instruction::Mul, word),
            Token::Symbol("/") => self.add_instruction(label, Instruction::Div, word),

            Token::Symbol("exit") => self.add_instruction(label, Instruction::Exit, word),

            Token::Symbol("dup") => self.add_instruction(label, Instruction::Dup, word),
            Token::Symbol("drop") => self.add_instruction(label, Instruction::Drop, word),
            Token::Symbol("swap") => self.add_instruction(label, Instruction::Swap, word),
            Token::Symbol("over") => self.add_instruction(label, Instruction::Over, word),

            Token::Symbol("apply") => self.add_instruction(label, Instruction::Apply, word),
            Token::Symbol("?") => self.add_instruction(label, Instruction::Branch, word),

            Token::Symbol(s) => todo!("user defined words: {s}"),
        }
    }
}
