use std::{
    fmt,
    io::{self, Write},
};

use crate::{
    compiler::{Instruction, Label, Proc},
    lexer::Word,
};

impl fmt::Display for Label<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.name() {
            Some(name) => write!(f, "proc_{}_{name}", self.id()),
            None => write!(f, "proc_{}", self.id()),
        }
    }
}

pub struct Generator<'src> {
    procs: &'src [Proc<'src>],
}

impl<'src> Generator<'src> {
    pub fn new(procs: &'src [Proc<'src>]) -> Self {
        Self { procs }
    }

    pub fn generate(procs: &'src [Proc<'src>], out: &mut impl Write) -> io::Result<()> {
        let generator = Self::new(procs);

        generator.gen_header(out)?;
        for proc in procs {
            generator.gen_proc(proc.label(), out)?;
        }

        Ok(())
    }

    fn get_proc(&self, label: Label) -> &Proc<'src> {
        &self.procs[label.id()]
    }

    fn gen_header(&self, out: &mut impl Write) -> io::Result<()> {
        writeln!(out, "section .bss")?;
        writeln!(out, "align 8")?;
        writeln!(out, "data_stack: resq 1024")?;

        writeln!(out, "section .text")?;
        writeln!(out, "global _start")?;

        writeln!(out, "_start:")?;
        writeln!(out, "    lea rcx, [rel data_stack]")?;
        writeln!(out, "    call proc_0")?;
        writeln!(out, "    mov rax, 60")?;
        writeln!(out, "    xor rdi, rdi")?;
        writeln!(out, "    syscall")?;

        Ok(())
    }

    fn gen_proc(&self, label: Label, out: &mut impl Write) -> io::Result<()> {
        writeln!(out, "{label}:")?;

        let proc = self.get_proc(label);

        for &(word, instruction) in proc.code() {
            self.gen_instruction(word, instruction, out)?;
        }

        writeln!(out, "    ; RETURN")?;
        writeln!(out, "    ret")?;

        Ok(())
    }

    fn gen_instruction(
        &self,
        word: Word<'src>,
        instruction: Instruction,
        out: &mut impl Write,
    ) -> io::Result<()> {
        match instruction {
            Instruction::PushInt(i) => {
                writeln!(out, "    ; {:?} -- PUSHINT", word.span())?;
                writeln!(out, "    mov qword [rcx], {i}")?;
                writeln!(out, "    add rcx, 8")?;
            }
            Instruction::PushBool(b) => {
                writeln!(out, "    ; {:?} -- PUSHBOOL", word.span())?;
                writeln!(
                    out,
                    "    mov qword [rcx], {}",
                    if b { -1isize } else { 0isize }
                )?;
                writeln!(out, "    add rcx, 8")?;
            }
            Instruction::PushQuote(q) => {
                writeln!(out, "    ; {:?} -- PUSHQUOTE", word.span())?;
                writeln!(out, "    mov qword [rcx], {q}")?;
                writeln!(out, "    add rcx, 8")?;
            }

            Instruction::Apply => {
                writeln!(out, "    ; {:?} -- APPLY", word.span())?;
                writeln!(out, "    sub rcx, 8")?;
                writeln!(out, "    call [rcx]")?;
            }
            Instruction::Branch => {
                writeln!(out, "    ; {:?} -- BRANCH", word.span())?;
                writeln!(out, "    mov rax, [rcx - 24]")?;
                writeln!(out, "    not rax")?;
                writeln!(out, "    mov rdx, [rcx - 16]")?;
                writeln!(out, "    mov rbx, [rcx - 8]")?;
                writeln!(out, "    and rdx, [rcx - 24]")?;
                writeln!(out, "    and rbx, rax")?;
                writeln!(out, "    or rdx, rbx")?;
                writeln!(out, "    mov [rcx - 24], rdx")?;
                writeln!(out, "    sub rcx, 16")?;
                writeln!(out, "    mov rax, [rcx - 24]")?;
            }
            Instruction::Exit => {
                writeln!(out, "    ; {:?} -- EXIT", word.span())?;
                writeln!(out, "    mov rax, 60")?;
                writeln!(out, "    mov rdi, [rcx - 8]")?;
                writeln!(out, "    syscall")?;
            }

            Instruction::Add => {
                writeln!(out, "    ; {:?} -- ADD", word.span())?;
                writeln!(out, "    mov rax, [rcx - 8]")?;
                writeln!(out, "    add [rcx - 16], rax")?;
                writeln!(out, "    sub rcx, 8")?;
            }
            Instruction::Sub => {
                writeln!(out, "    ; {:?} -- SUB", word.span())?;
                writeln!(out, "    mov rax, [rcx - 8]")?;
                writeln!(out, "    sub [rcx - 16], rax")?;
                writeln!(out, "    sub rcx, 8")?;
            }
            Instruction::Mul => {
                writeln!(out, "    ; {:?} -- MUL", word.span())?;
                writeln!(out, "    mov rax, [rcx - 8]")?;
                writeln!(out, "    imul [rcx - 16], rax")?;
                writeln!(out, "    sub rcx, 8")?;
            }
            Instruction::Div => todo!(),

            Instruction::Dup => {
                writeln!(out, "    ; {:?} -- DUP", word.span())?;
                writeln!(out, "    mov rax, [rcx - 8]")?;
                writeln!(out, "    mov [rcx], rax")?;
                writeln!(out, "    add rcx, 8")?;
            }
            Instruction::Over => {
                writeln!(out, "    ; {:?} -- OVER", word.span())?;
                writeln!(out, "    mov rax, [rcx - 16]")?;
                writeln!(out, "    mov [rcx], rax")?;
                writeln!(out, "    add rcx, 8")?;
            }
            Instruction::Drop => {
                writeln!(out, "    ; {:?} -- DROP", word.span())?;
                writeln!(out, "    sub rcx, 8")?;
            }
            Instruction::Swap => {
                writeln!(out, "    ; {:?} -- SWAP", word.span())?;
                writeln!(out, "    mov rax, [rcx - 8]")?;
                writeln!(out, "    mov rdx, [rcx - 16]")?;
                writeln!(out, "    mov [rcx - 8], rdx")?;
                writeln!(out, "    mov [rcx - 16], rax")?;
            }
        }

        Ok(())
    }
}
