use std::{
    fmt,
    io::{self, Write},
};

use crate::{
    compiler::{Instruction, Label, Proc},
    lexer::Span,
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
    string_literals: &'src [Box<str>],
}

impl<'src> Generator<'src> {
    pub fn new(procs: &'src [Proc<'src>], string_literals: &'src [Box<str>]) -> Self {
        Self {
            procs,
            string_literals,
        }
    }

    pub fn generate(
        procs: &'src [Proc<'src>],
        string_literals: &'src [Box<str>],
        out: &mut impl Write,
    ) -> io::Result<()> {
        let generator = Self::new(procs, string_literals);

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
        writeln!(out, "struct_stack: resq 1024")?;

        writeln!(out, "section .rodata")?;

        for (i, string_literal) in self.string_literals.iter().enumerate() {
            write!(out, "str_{i}: db ")?;
            let bytes = string_literal.as_bytes();
            for (l, byte) in bytes.iter().enumerate() {
                write!(out, "{byte}")?;
                if l < bytes.len() - 1 {
                    write!(out, ",")?;
                }
            }
            writeln!(out)?;
        }

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

        for &(span, instruction) in proc.code() {
            self.gen_instruction(span, instruction, out)?;
        }

        writeln!(out, "    ; RETURN")?;
        writeln!(out, "    ret")?;

        Ok(())
    }

    fn emit_copy_up(&self, out: &mut impl Write, offset: isize, size: usize) -> io::Result<()> {
        for i in 0..size {
            let byte_offset = offset - (8 * (size - i) as isize);
            writeln!(out, "    mov rax, [rcx + {byte_offset}]")?;
            writeln!(out, "    mov [rcx + {}], rax", i * 8)?;
        }
        writeln!(out, "    add rcx, {}", size * 8)?;
        Ok(())
    }

    fn emit_drop(&self, out: &mut impl Write, size: usize) -> io::Result<()> {
        writeln!(out, "    sub rcx, {}", size * 8)?;
        Ok(())
    }

    fn gen_instruction(
        &self,
        span: Span,
        instruction: Instruction,
        out: &mut impl Write,
    ) -> io::Result<()> {
        match instruction {
            Instruction::PushInt(i) => {
                writeln!(out, "    ; {:?} -- PUSHINT", span)?;
                writeln!(out, "    mov qword [rcx], {i}")?;
                writeln!(out, "    add rcx, 8")?;
            }
            Instruction::PushBool(b) => {
                writeln!(out, "    ; {:?} -- PUSHBOOL", span)?;
                writeln!(
                    out,
                    "    mov qword [rcx], {}",
                    if b { -1isize } else { 0isize }
                )?;
                writeln!(out, "    add rcx, 8")?;
            }
            Instruction::PushString(i) => {
                writeln!(out, "    ; {:?} -- PUSHSTRING", span)?;
                writeln!(out, "    lea rax, [rel str_{i}]")?;
                writeln!(out, "    mov [rcx], rax")?;
                writeln!(out, "    mov rax, {}", self.string_literals[i].len())?;
                writeln!(out, "    mov [rcx + 8], rax")?;
                writeln!(out, "    add rcx, 16")?;
            }
            Instruction::PushQuote(q) => {
                writeln!(out, "    ; {:?} -- PUSHQUOTE", span)?;
                writeln!(out, "    mov qword [rcx], {q}")?;
                writeln!(out, "    add rcx, 8")?;
            }

            Instruction::Apply => {
                writeln!(out, "    ; {:?} -- APPLY", span)?;
                writeln!(out, "    sub rcx, 8")?;
                writeln!(out, "    call [rcx]")?;
            }
            Instruction::Branch { size } => {
                writeln!(out, "    ; {:?} -- BRANCH", span)?;

                let cond_off = -8 * (2 * size as isize + 1);
                let true_off_start = -8 * (size as isize + 1);
                let false_off_start = -8 * 1;
                let result_off_start = cond_off;

                writeln!(out, "    mov rax, [rcx - {}]", -cond_off)?;
                writeln!(out, "    mov rbx, rax")?;
                writeln!(out, "    not rbx")?;

                for i in 0..size {
                    let true_i = true_off_start - 8 * i as isize;
                    let false_i = false_off_start - 8 * i as isize;
                    let res_i = result_off_start - 8 * i as isize;

                    writeln!(out, "    mov rdx, [rcx - {}]", -true_i)?;
                    writeln!(out, "    and rdx, rax")?;

                    writeln!(out, "    mov rsi, [rcx - {}]", -false_i)?;
                    writeln!(out, "    and rsi, rbx")?;

                    writeln!(out, "    or rdx, rsi")?;
                    writeln!(out, "    mov [rcx - {}], rdx", -res_i)?;
                }

                writeln!(out, "    sub rcx, {}", 16 * size)?;
            }

            Instruction::Exit => {
                writeln!(out, "    ; {:?} -- EXIT", span)?;
                writeln!(out, "    mov rax, 60")?;
                writeln!(out, "    mov rdi, [rcx - 8]")?;
                writeln!(out, "    syscall")?;
            }

            Instruction::Puts => {
                writeln!(out, "    ; {:?} -- PUTS", span)?;
                writeln!(out, "    mov rdi, 1")?;
                writeln!(out, "    mov rsi, [rcx - 16]")?;
                writeln!(out, "    mov rdx, [rcx - 8]")?;
                writeln!(out, "    mov rax, 1")?;
                writeln!(out, "    syscall")?;
                writeln!(out, "    sub rcx, 16")?;
            }

            Instruction::Add => {
                writeln!(out, "    ; {:?} -- ADD", span)?;
                writeln!(out, "    mov rax, [rcx - 8]")?;
                writeln!(out, "    add [rcx - 16], rax")?;
                writeln!(out, "    sub rcx, 8")?;
            }
            Instruction::Sub => {
                writeln!(out, "    ; {:?} -- SUB", span)?;
                writeln!(out, "    mov rax, [rcx - 8]")?;
                writeln!(out, "    sub [rcx - 16], rax")?;
                writeln!(out, "    sub rcx, 8")?;
            }
            Instruction::Mul => {
                writeln!(out, "    ; {:?} -- MUL", span)?;
                writeln!(out, "    mov rax, [rcx - 8]")?;
                writeln!(out, "    imul [rcx - 16], rax")?;
                writeln!(out, "    sub rcx, 8")?;
            }
            Instruction::Div => todo!(),

            Instruction::Dup { size } => {
                writeln!(out, "    ; {:?} -- DUP", span)?;
                self.emit_copy_up(out, -(size as isize * 8), size)?;
            }

            Instruction::Over { size_a, size_b } => {
                writeln!(out, "    ; {:?} -- OVER", span)?;
                let offset = -((size_a + size_b) as isize * 8);
                self.emit_copy_up(out, offset, size_a)?;
            }

            Instruction::Drop { size } => {
                writeln!(out, "    ; {:?} -- DROP", span)?;
                self.emit_drop(out, size)?;
            }
            Instruction::Swap { size_a, size_b } => {
                writeln!(out, "    ; {:?} -- SWAP", span)?;

                let sa = size_a * 8;
                let sb = size_b * 8;

                for i in 0..size_a {
                    writeln!(out, "    mov rax, [rcx - {}]", 8 * (i + 1))?;
                    writeln!(out, "    mov [rsp - {}], rax", 8 * (i + 1))?;
                }
                for i in 0..size_b {
                    writeln!(out, "    mov rax, [rcx - {}]", sa + 8 * (i + 1))?;
                    writeln!(out, "    mov [rcx - {}], rax", 8 * (i + 1))?;
                }
                for i in 0..size_a {
                    writeln!(out, "    mov rax, [rsp - {}]", 8 * (i + 1))?;
                    writeln!(out, "    mov [rcx - {}], rax", sb + 8 * (i + 1))?;
                }
            }
        }

        Ok(())
    }
}
