use std::{collections::HashMap, iter::Peekable};

use crate::lexer::{Span, Token, Word};

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
    String,
    Var(usize),
    MultiVar(usize),
    Quotation(Signature),
}

#[derive(Debug, Clone)]
pub enum ItemKind<'src> {
    Integer(isize),
    String(&'src str),
    Word(Signature, &'src str),
    Quotation(Signature, Box<[Item<'src>]>),
}

#[derive(Debug, Clone)]
pub struct Item<'src> {
    kind: ItemKind<'src>,
    span: Span,
}

impl<'src> Item<'src> {
    fn new(kind: ItemKind<'src>, span: Span) -> Self {
        Self { kind, span }
    }

    pub fn parts(self) -> (ItemKind<'src>, Span) {
        (self.kind, self.span)
    }

    pub fn kind(&self) -> &ItemKind<'src> {
        &self.kind
    }

    pub fn span(&self) -> Span {
        self.span
    }
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

    pub fn parts(self) -> (Vec<Type>, Vec<Type>) {
        (self.inputs, self.outputs)
    }
}

struct Context {
    var_context: HashMap<usize, Type>,
    multivar_context: HashMap<usize, Box<[Type]>>,
    var_gen: usize,
    multivar_gen: usize,
}

impl Context {
    fn new() -> Self {
        Self {
            var_context: HashMap::new(),
            multivar_context: HashMap::new(),
            var_gen: 0,
            multivar_gen: 0,
        }
    }

    fn get_var(&self, var: usize) -> Option<&Type> {
        self.var_context.get(&var)
    }

    fn get_multivar(&self, var: usize) -> Option<&[Type]> {
        self.multivar_context.get(&var).map(|ty| &**ty)
    }

    fn set_var(&mut self, var: usize, ty: Type) {
        self.var_context.insert(var, ty);
    }

    fn set_multivar(&mut self, var: usize, ty: Box<[Type]>) {
        self.multivar_context.insert(var, ty);
    }

    fn gen_var(&mut self) -> usize {
        let v = self.var_gen;
        self.var_gen += 1;
        v
    }

    fn gen_multivar(&mut self) -> usize {
        let v = self.multivar_gen;
        self.multivar_gen += 1;
        v
    }
}

struct State<'src> {
    signature: Signature,
    items: Vec<Item<'src>>,
}

impl<'src> State<'src> {
    pub fn new() -> Self {
        Self {
            signature: Signature::new(vec![], vec![]),
            items: Vec::new(),
        }
    }

    fn push_output(&mut self, ty: Type) {
        self.signature.outputs.push(ty)
    }

    fn push_input(&mut self, ty: Type) {
        self.signature.inputs.push(ty)
    }

    fn clone_outputs(&self) -> Vec<Type> {
        self.signature.outputs.clone()
    }

    fn resolve_type(&self, t: Type, stack: &mut Vec<Type>, context: &Context) {
        match t {
            Type::Int => stack.push(Type::Int),
            Type::Bool => stack.push(Type::Bool),
            Type::String => stack.push(Type::String),
            Type::Var(v) => {
                if let Some(var) = context.get_var(v).cloned() {
                    let mut resolved = Vec::new();
                    self.resolve_type(var, &mut resolved, context);
                    stack.push(resolved.into_iter().next().unwrap());
                } else {
                    stack.push(t);
                }
            }
            Type::MultiVar(v) => {
                if let Some(var) = context.get_multivar(v) {
                    let mut resolved = Vec::new();
                    for v in var {
                        self.resolve_type(v.clone(), &mut resolved, context);
                    }
                    stack.extend(resolved);
                } else {
                    stack.push(t);
                }
            }
            Type::Quotation(signature) => {
                stack.push(Type::Quotation(self.resolve_signature(signature, context)))
            }
        }
    }

    fn resolve_signature(&self, signature: Signature, context: &Context) -> Signature {
        let Signature { inputs, outputs } = signature;

        let mut new_inputs = Vec::new();
        inputs
            .into_iter()
            .for_each(|t_inner| self.resolve_type(t_inner, &mut new_inputs, context));

        let mut new_outputs = Vec::new();
        outputs
            .into_iter()
            .for_each(|t_inner| self.resolve_type(t_inner, &mut new_outputs, context));

        Signature::new(new_inputs, new_outputs)
    }

    fn resolve(&self, context: &Context) -> Signature {
        self.resolve_signature(self.signature.clone(), context)
    }

    fn resolve_item(&self, item: &Item<'src>, context: &Context) -> Item<'src> {
        Item::new(
            match &item.kind {
                ItemKind::Quotation(signature, items) => {
                    let mut new_items = Vec::new();
                    for item in items {
                        new_items.push(self.resolve_item(&item, context))
                    }
                    ItemKind::Quotation(
                        self.resolve_signature(signature.clone(), context),
                        new_items.into_boxed_slice(),
                    )
                }
                ItemKind::Word(signature, word) => {
                    let sig = self.resolve_signature(signature.clone(), context);
                    ItemKind::Word(sig, word)
                }
                _ => item.kind.clone(),
            },
            item.span,
        )
    }

    fn resolve_all(self, context: &Context) -> (Signature, Vec<Item<'src>>) {
        let signature = self.resolve(context);
        let mut new_items = Vec::new();

        for item in self.items.iter() {
            new_items.push(self.resolve_item(item, context))
        }

        (signature, new_items)
    }

    fn instantiate(
        &mut self,
        stack: &mut [Type],
        local_vars: &mut HashMap<usize, usize>,
        local_multivars: &mut HashMap<usize, usize>,
        context: &mut Context,
    ) {
        for t in stack.iter_mut() {
            match t {
                Type::Int | Type::Bool | Type::String => (),
                Type::Var(n) => {
                    if let Some(var) = local_vars.get(n) {
                        *t = Type::Var(*var);
                    } else {
                        let var = context.gen_var();
                        local_vars.insert(*n, var);
                        *t = Type::Var(var);
                    }
                }
                Type::MultiVar(n) => {
                    if let Some(var) = local_vars.get(n) {
                        *t = Type::MultiVar(*var);
                    } else {
                        let var = context.gen_multivar();
                        local_vars.insert(*n, var);
                        *t = Type::MultiVar(var);
                    }
                }
                Type::Quotation(q_sig) => {
                    self.instantiate(&mut q_sig.inputs, local_vars, local_multivars, context);
                    self.instantiate(&mut q_sig.outputs, local_vars, local_multivars, context);
                }
            }
        }
    }

    fn unify(
        &mut self,
        word: Word<'src>,
        sig: &Signature,
        stack_shot: &[Type],
        a: &Type,
        b: &Type,
        context: &mut Context,
    ) -> Result<(), CompileError<'src>> {
        match (a, b) {
            (Type::Bool, Type::Bool) => Ok(()),
            (Type::Int, Type::Int) => Ok(()),
            (Type::String, Type::String) => Ok(()),
            (Type::Var(v), t) => {
                if let Some(v_t) = context.get_var(*v) {
                    self.unify(word, sig, stack_shot, &v_t.clone(), t, context)?;
                } else {
                    if let Type::Var(t_var) = t {
                        if t_var == v {
                            return Ok(());
                        }
                    }
                    context.set_var(*v, t.clone());
                }
                Ok(())
            }
            (t, Type::Var(v)) => {
                if let Some(v_t) = context.get_var(*v) {
                    self.unify(word, sig, stack_shot, &v_t.clone(), t, context)?;
                } else {
                    context.set_var(*v, t.clone());
                }
                Ok(())
            }
            (Type::Quotation(a_sig), Type::Quotation(b_sig)) => {
                self.unify_signature(word, sig, a_sig, b_sig, stack_shot, context)
            }
            _ => Err(CompileError::CannotExecSignature {
                word,
                stack: stack_shot.to_vec(),
                sig: sig.clone(),
            }),
        }
    }

    fn unify_signature(
        &mut self,
        word: Word<'src>,
        sig: &Signature,
        a: &Signature,
        b: &Signature,
        stack_shot: &[Type],
        context: &mut Context,
    ) -> Result<(), CompileError<'src>> {
        self.unify_stack(word, sig, &a.inputs, &b.inputs, stack_shot, context)?;
        self.unify_stack(word, sig, &a.outputs, &b.outputs, stack_shot, context)
    }

    fn unify_stack(
        &mut self,
        word: Word<'src>,
        sig: &Signature,
        a: &[Type],
        b: &[Type],
        stack_shot: &[Type],
        context: &mut Context,
    ) -> Result<(), CompileError<'src>> {
        match (a.split_last(), b.split_last()) {
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
                    self.unify(word, sig, stack_shot, a_t, b_t, context)?;
                }

                let tail = &a[len..];
                context.set_multivar(*b_mv, tail.to_vec().into_boxed_slice());

                Ok(())
            }
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
                    self.unify(word, sig, stack_shot, a_t, b_t, context)?;
                }

                let tail = &b[len..];
                context.set_multivar(*a_mv, tail.to_vec().into_boxed_slice());

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
                    self.unify(word, sig, stack_shot, a_t, b_t, context)?;
                }

                Ok(())
            }
        }
    }
}

pub struct Analyzer<'src, W: Iterator<Item = Word<'src>>> {
    word_bindings: HashMap<&'src str, Signature>,
    words: Peekable<W>,
}

impl<'src, W: Iterator<Item = Word<'src>>> Analyzer<'src, W> {
    pub fn new(words: W) -> Self {
        Self {
            word_bindings: HashMap::new(),
            words: words.peekable(),
        }
    }

    fn register_builtins(&mut self) {
        use Signature as S;
        use Type::*;

        self.word_bindings
            .insert("+", S::new(vec![Int, Int], vec![Int]));
        self.word_bindings
            .insert("-", S::new(vec![Int, Int], vec![Int]));
        self.word_bindings
            .insert("*", S::new(vec![Int, Int], vec![Int]));
        self.word_bindings
            .insert("/", S::new(vec![Int, Int], vec![Int]));

        self.word_bindings.insert("exit", S::new(vec![Int], vec![]));

        self.word_bindings
            .insert("puts", S::new(vec![String], vec![]));

        self.word_bindings
            .insert("true", S::new(vec![], vec![Bool]));
        self.word_bindings
            .insert("false", S::new(vec![], vec![Bool]));

        self.word_bindings
            .insert("dup", S::new(vec![Var(0)], vec![Var(0), Var(0)]));
        self.word_bindings
            .insert("swap", S::new(vec![Var(1), Var(0)], vec![Var(1), Var(0)]));
        self.word_bindings
            .insert("drop", S::new(vec![Var(0)], vec![]));
        self.word_bindings.insert(
            "over",
            S::new(vec![Var(1), Var(0)], vec![Var(0), Var(1), Var(0)]),
        );

        self.word_bindings.insert(
            "apply",
            S::new(
                vec![
                    Quotation(Signature::new(vec![MultiVar(0)], vec![MultiVar(1)])),
                    MultiVar(0),
                ],
                vec![MultiVar(1)],
            ),
        );
        self.word_bindings
            .insert("?", S::new(vec![Var(0), Var(0), Bool], vec![Var(0)]));
    }

    pub fn analyze(words: W) -> Result<(Signature, Box<[Item<'src>]>), CompileError<'src>> {
        let mut analyzer = Self::new(words);
        let mut state = State::new();
        let mut context = Context::new();
        analyzer.register_builtins();

        while analyzer.words.peek().is_some() {
            analyzer.check_word(&mut state, &mut context)?;
        }

        let (signature, word_types) = state.resolve_all(&context);

        Ok((signature, word_types.into_boxed_slice()))
    }

    fn check_word(
        &mut self,
        state: &mut State<'src>,
        context: &mut Context,
    ) -> Result<(), CompileError<'src>> {
        let Some(word) = self.words.next() else {
            return Ok(());
        };

        let item = Item::new(
            match word.token() {
                Token::Integer(i) => {
                    state.push_output(Type::Int);
                    ItemKind::Integer(i)
                }
                Token::String(s) => {
                    state.push_output(Type::String);
                    ItemKind::String(s)
                }
                Token::Symbol("[") => {
                    let mut quotation_state = State::new();

                    while self
                        .words
                        .peek()
                        .is_some_and(|word| !matches!(word.token(), Token::Symbol("]")))
                    {
                        self.check_word(&mut quotation_state, context)?;
                    }

                    self.words.next();

                    let (sig, items) = quotation_state.resolve_all(context);
                    state.push_output(Type::Quotation(sig.clone()));
                    ItemKind::Quotation(sig, items.into_boxed_slice())
                }
                Token::Symbol(sym) => {
                    let Some(signature) = self.word_bindings.get(sym) else {
                        return Err(CompileError::UndefinedWord { word });
                    };

                    let mut signature = signature.clone();
                    self.try_signature(word, state, &mut signature, context, true)?;

                    ItemKind::Word(signature, sym)
                }
            },
            word.span(),
        );

        state.items.push(item);

        Ok(())
    }

    fn try_signature(
        &mut self,
        word: Word<'src>,
        state: &mut State<'src>,
        sig: &mut Signature,
        context: &mut Context,
        instantiate: bool,
    ) -> Result<(), CompileError<'src>> {
        let stack = state.clone_outputs();

        if instantiate {
            let mut local_vars = HashMap::new();
            let mut local_multivars = HashMap::new();

            state.instantiate(
                &mut sig.inputs,
                &mut local_vars,
                &mut local_multivars,
                context,
            );
            state.instantiate(
                &mut sig.outputs,
                &mut local_vars,
                &mut local_multivars,
                context,
            );
        }

        for input in &sig.inputs {
            if let Type::MultiVar(mv) = input {
                let Some(tys) = context.get_multivar(*mv) else {
                    todo!("undefined multivar")
                };
                self.try_signature(
                    word,
                    state,
                    &mut Signature::new(tys.to_vec(), Vec::new()),
                    context,
                    false,
                )?;
            } else if let Some(ty) = state.signature.outputs.pop() {
                state.unify(word, &sig, &stack, input, &ty, context)?;
            } else {
                state.push_input(input.clone());
            }
        }

        let mut new_outputs = Vec::new();
        sig.outputs
            .iter()
            .for_each(|t_inner| state.resolve_type(t_inner.clone(), &mut new_outputs, context));

        state.signature.outputs.extend(new_outputs);
        Ok(())
    }
}
