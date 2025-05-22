use std::collections::HashMap;

use crate::lexer::{Token, Word};

#[derive(Debug, Clone)]
pub enum CompileError<'src> {
    UndefinedWord {
        word: Word<'src>,
    },
    CannotExecSignature {
        word: Word<'src>,
        stack: Vec<Type>,
        sig: Signature,
    },
}

#[derive(Debug, Clone)]
pub enum Type {
    Int,
    Bool,
    Var(usize),
    MultiVar(usize),
    Quotation(Signature),
}

#[derive(Debug, Clone)]
pub struct Signature {
    inputs: Vec<Type>,
    outputs: Vec<Type>,
}

impl Signature {
    fn new(inputs: Vec<Type>, outputs: Vec<Type>) -> Self {
        Self { inputs, outputs }
    }
}

fn register_builtins<'src>(bindings: &mut HashMap<&'src str, Signature>) {
    use Signature as S;
    use Type::*;

    let mut add = |n, s| bindings.insert(n, s);

    add("+", S::new(vec![Int, Int], vec![Int]));
    add("-", S::new(vec![Int, Int], vec![Int]));
    add("*", S::new(vec![Int, Int], vec![Int]));
    add("/", S::new(vec![Int, Int], vec![Int]));

    add("puti", S::new(vec![Int], vec![]));

    add("true", S::new(vec![], vec![Bool]));
    add("false", S::new(vec![], vec![Bool]));

    add("dup", S::new(vec![Var(0)], vec![Var(0), Var(0)]));
    add("swap", S::new(vec![Var(1), Var(0)], vec![Var(1), Var(0)]));
    add("drop", S::new(vec![Var(0)], vec![]));
    add(
        "over",
        S::new(vec![Var(1), Var(0)], vec![Var(0), Var(1), Var(0)]),
    );

    add(
        "apply",
        S::new(
            vec![
                Quotation(Signature::new(vec![MultiVar(0)], vec![MultiVar(1)])),
                MultiVar(0),
            ],
            vec![MultiVar(1)],
        ),
    );
    add("?", S::new(vec![Bool, Var(0), Var(0)], vec![Var(0)]));
}

fn next<'src>(words: &[Word<'src>], pos: &mut usize) -> Option<Word<'src>> {
    if *pos < words.len() {
        let r = Some(words[*pos]);
        *pos += 1;
        r
    } else {
        None
    }
}

pub fn analyze<'src>(words: &[Word<'src>]) -> Result<(), CompileError<'src>> {
    let mut var_gen = 0;
    let mut multivar_gen = 0;

    let mut inputs = Vec::new();
    let mut outputs = Vec::new();
    let mut var_context = HashMap::new();
    let mut multivar_context = HashMap::new();

    let mut bindings = HashMap::new();
    register_builtins(&mut bindings);

    let mut pos = 0;

    while pos < words.len() {
        check_word(
            words,
            &mut pos,
            &mut bindings,
            &mut var_gen,
            &mut multivar_gen,
            &mut inputs,
            &mut outputs,
            &mut var_context,
            &mut multivar_context,
        )?;
    }

    let mut new_inputs = Vec::new();
    inputs
        .into_iter()
        .for_each(|t_inner| resolve(t_inner, &mut new_inputs, &var_context, &multivar_context));

    let mut new_outputs = Vec::new();
    outputs
        .into_iter()
        .for_each(|t_inner| resolve(t_inner, &mut new_outputs, &var_context, &multivar_context));

    Ok(())
}

fn check_word<'src>(
    words: &[Word<'src>],
    pos: &mut usize,
    bindings: &HashMap<&'src str, Signature>,
    var_gen: &mut usize,
    multivar_gen: &mut usize,
    inputs: &mut Vec<Type>,
    outputs: &mut Vec<Type>,
    var_context: &mut HashMap<usize, Type>,
    multivar_context: &mut HashMap<usize, Box<[Type]>>,
) -> Result<(), CompileError<'src>> {
    let Some(word) = next(words, pos) else {
        return Ok(());
    };

    match word.token() {
        Token::Integer(_) => {
            outputs.push(Type::Int);
            Ok(())
        }
        Token::Symbol("[") => {
            let mut q_var_gen = 0;
            let mut q_multivar_gen = 0;

            let mut q_inputs = Vec::new();
            let mut q_outputs = Vec::new();
            let mut q_var_context = HashMap::new();
            let mut q_multivar_context = HashMap::new();

            while words
                .get(*pos)
                .is_some_and(|word| !matches!(word.token(), Token::Symbol("]")))
            {
                check_word(
                    words,
                    pos,
                    bindings,
                    &mut q_var_gen,
                    &mut q_multivar_gen,
                    &mut q_inputs,
                    &mut q_outputs,
                    &mut q_var_context,
                    &mut q_multivar_context,
                )?;
            }

            *pos += 1;

            outputs.push(Type::Quotation(Signature::new(q_inputs, q_outputs)));
            Ok(())
        }
        Token::Symbol(sym) => {
            let Some(sig) = bindings.get(sym) else {
                return Err(CompileError::UndefinedWord { word });
            };

            try_signature(
                word,
                sig.clone(),
                var_gen,
                multivar_gen,
                inputs,
                outputs,
                var_context,
                multivar_context,
            )
        }
    }
}

fn try_signature<'src>(
    word: Word<'src>,
    mut sig: Signature,
    var_gen: &mut usize,
    multivar_gen: &mut usize,
    inputs: &mut Vec<Type>,
    outputs: &mut Vec<Type>,
    var_context: &mut HashMap<usize, Type>,
    multivar_context: &mut HashMap<usize, Box<[Type]>>,
) -> Result<(), CompileError<'src>> {
    let stack = outputs.clone();

    let mut local_vars = HashMap::new();
    let mut local_multivars = HashMap::new();
    instantiate(
        &mut sig.inputs,
        var_gen,
        multivar_gen,
        &mut local_vars,
        &mut local_multivars,
    );
    instantiate(
        &mut sig.outputs,
        var_gen,
        multivar_gen,
        &mut local_vars,
        &mut local_multivars,
    );

    for input in &sig.inputs {
        if let Type::MultiVar(mv) = input {
            let Some(tys) = multivar_context.get(mv) else {
                todo!("undefined multivar")
            };
            try_signature(
                word,
                Signature::new(tys.to_vec(), Vec::new()),
                var_gen,
                multivar_gen,
                inputs,
                outputs,
                var_context,
                multivar_context,
            )?;
        } else {
            if let Some(ty) = outputs.pop() {
                unify(
                    word,
                    &sig,
                    &stack,
                    input,
                    &ty,
                    var_context,
                    multivar_context,
                )?;
            } else {
                inputs.push(input.clone());
            }
        }
    }

    outputs.extend(sig.outputs);

    Ok(())
}

fn instantiate(
    stack: &mut Vec<Type>,
    var_gen: &mut usize,
    multivar_gen: &mut usize,
    local_vars: &mut HashMap<usize, usize>,
    local_multivars: &mut HashMap<usize, usize>,
) {
    for t in stack.iter_mut() {
        match t {
            Type::Int | Type::Bool => (),
            Type::Var(n) => {
                if let Some(var) = local_vars.get(n) {
                    *t = Type::Var(*var);
                } else {
                    let var = *var_gen;
                    *var_gen += 1;
                    local_vars.insert(*n, var);
                    *t = Type::Var(var);
                }
            }
            Type::MultiVar(n) => {
                if let Some(var) = local_vars.get(n) {
                    *t = Type::MultiVar(*var);
                } else {
                    let var = *var_gen;
                    *var_gen += 1;
                    local_vars.insert(*n, var);
                    *t = Type::MultiVar(var);
                }
            }
            Type::Quotation(q_sig) => {
                instantiate(
                    &mut q_sig.inputs,
                    var_gen,
                    multivar_gen,
                    local_vars,
                    local_multivars,
                );
                instantiate(
                    &mut q_sig.outputs,
                    var_gen,
                    multivar_gen,
                    local_vars,
                    local_multivars,
                );
            }
        }
    }
}

fn resolve(
    t: Type,
    stack: &mut Vec<Type>,
    var_context: &HashMap<usize, Type>,
    multivar_context: &HashMap<usize, Box<[Type]>>,
) {
    match t {
        Type::Int => stack.push(Type::Int),
        Type::Bool => stack.push(Type::Bool),
        Type::Var(v) => {
            if let Some(var) = var_context.get(&v).cloned() {
                let mut resolved = Vec::new();
                resolve(var, &mut resolved, var_context, multivar_context);
                stack.push(resolved.into_iter().next().unwrap());
            } else {
                stack.push(t);
            }
        }
        Type::MultiVar(v) => {
            if let Some(var) = multivar_context.get(&v).cloned() {
                let mut resolved = Vec::new();
                for v in var {
                    resolve(v, &mut resolved, var_context, multivar_context);
                }
                stack.extend(resolved.into_iter());
            } else {
                stack.push(t);
            }
        }
        Type::Quotation(sig) => {
            let mut inputs = Vec::new();
            sig.inputs
                .into_iter()
                .for_each(|t_inner| resolve(t_inner, &mut inputs, var_context, multivar_context));

            let mut outputs = Vec::new();
            sig.outputs
                .into_iter()
                .for_each(|t_inner| resolve(t_inner, &mut outputs, var_context, multivar_context));

            stack.push(Type::Quotation(Signature::new(inputs, outputs)))
        }
    }
}

fn unify<'src>(
    word: Word<'src>,
    sig: &Signature,
    stack_shot: &[Type],
    a: &Type,
    b: &Type,
    var_context: &mut HashMap<usize, Type>,
    multivar_context: &mut HashMap<usize, Box<[Type]>>,
) -> Result<(), CompileError<'src>> {
    match (a, b) {
        (Type::Bool, Type::Bool) => Ok(()),
        (Type::Int, Type::Int) => Ok(()),
        (Type::Var(v), t) => {
            if let Some(v_t) = var_context.get(v) {
                unify(
                    word,
                    sig,
                    stack_shot,
                    &v_t.clone(),
                    t,
                    var_context,
                    multivar_context,
                )?;
            } else {
                if let Type::Var(t_var) = t {
                    if t_var == v {
                        return Ok(());
                    }
                }
                var_context.insert(*v, t.clone());
            }
            Ok(())
        }
        (t, Type::Var(v)) => {
            if let Some(v_t) = var_context.get(v) {
                unify(
                    word,
                    sig,
                    stack_shot,
                    &v_t.clone(),
                    t,
                    var_context,
                    multivar_context,
                )?;
            } else {
                var_context.insert(*v, t.clone());
            }
            Ok(())
        }
        (Type::Quotation(a_sig), Type::Quotation(b_sig)) => unify_signature(
            word,
            sig,
            a_sig,
            b_sig,
            stack_shot,
            var_context,
            multivar_context,
        ),
        _ => Err(CompileError::CannotExecSignature {
            word,
            stack: stack_shot.to_vec(),
            sig: sig.clone(),
        }),
    }
}

fn unify_signature<'src>(
    word: Word<'src>,
    sig: &Signature,
    a: &Signature,
    b: &Signature,
    stack_shot: &[Type],
    var_context: &mut HashMap<usize, Type>,
    multivar_context: &mut HashMap<usize, Box<[Type]>>,
) -> Result<(), CompileError<'src>> {
    unify_stack(
        word,
        sig,
        &a.inputs,
        &b.inputs,
        stack_shot,
        var_context,
        multivar_context,
    )?;

    unify_stack(
        word,
        sig,
        &a.outputs,
        &b.outputs,
        stack_shot,
        var_context,
        multivar_context,
    )?;

    Ok(())
}

fn unify_stack<'src>(
    word: Word<'src>,
    sig: &Signature,
    a: &[Type],
    b: &[Type],
    stack_shot: &[Type],
    var_context: &mut HashMap<usize, Type>,
    multivar_context: &mut HashMap<usize, Box<[Type]>>,
) -> Result<(), CompileError<'src>> {
    match (a.split_last(), b.split_last()) {
        (Some((Type::MultiVar(a_mv), a_rest)), _) => {
            let len = a_rest.len();
            if b.len() < len {
                return Err(CompileError::CannotExecSignature {
                    word,
                    stack: stack_shot.to_vec(),
                    sig: sig.clone(),
                });
            }

            for (a_t, b_t) in a_rest.iter().zip(&b[..len]) {
                unify(
                    word,
                    sig,
                    stack_shot,
                    a_t,
                    b_t,
                    var_context,
                    multivar_context,
                )?;
            }

            let tail = &b[len..];
            multivar_context.insert(*a_mv, tail.to_vec().into_boxed_slice());

            Ok(())
        }
        (_, Some((Type::MultiVar(b_mv), b_rest))) => {
            let len = b_rest.len();
            if a.len() < len {
                return Err(CompileError::CannotExecSignature {
                    word,
                    stack: stack_shot.to_vec(),
                    sig: sig.clone(),
                });
            }

            for (a_t, b_t) in a[..len].iter().zip(b_rest) {
                unify(
                    word,
                    sig,
                    stack_shot,
                    a_t,
                    b_t,
                    var_context,
                    multivar_context,
                )?;
            }

            let tail = &a[len..];
            multivar_context.insert(*b_mv, tail.to_vec().into_boxed_slice());

            Ok(())
        }
        _ => {
            if a.len() != b.len() {
                return Err(CompileError::CannotExecSignature {
                    word,
                    stack: stack_shot.to_vec(),
                    sig: sig.clone(),
                });
            }

            for (a_t, b_t) in a.iter().zip(b.iter()) {
                unify(
                    word,
                    sig,
                    stack_shot,
                    a_t,
                    b_t,
                    var_context,
                    multivar_context,
                )?;
            }

            Ok(())
        }
    }
}
