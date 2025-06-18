use std::ops::Range;

#[derive(Debug, Clone, Copy)]
pub struct Span {
    start: usize,
    end: usize,
}

impl Span {
    fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn parts(&self) -> (usize, usize) {
        (self.start, self.end)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Token<'src> {
    Integer(isize),
    Symbol(&'src str),
    String(&'src str),
}

#[derive(Debug, Clone, Copy)]
pub struct Word<'src> {
    token: Token<'src>,
    span: Span,
}

impl<'src> Word<'src> {
    fn new(token: Token<'src>, span: Span) -> Self {
        Self { token, span }
    }

    pub fn token(&self) -> Token<'src> {
        self.token
    }

    pub fn span(&self) -> Span {
        self.span
    }

    pub fn word(&self) -> &'src str {
        let Token::Symbol(word) = self.token() else {
            unreachable!();
        };
        word
    }
}

pub struct Lexer<'src> {
    source: &'src str,
    chars: std::str::CharIndices<'src>,
}

impl<'src> Lexer<'src> {
    pub fn new(source: &'src str) -> Self {
        Lexer {
            source,
            chars: source.char_indices(),
        }
    }

    fn word(&mut self) -> Option<Word<'src>> {
        let (start, start_ch) = self.chars.find(|&(_, c)| !c.is_whitespace())?;

        let (end, token) = match start_ch {
            '"' => {
                let mut escaped = false;
                let mut end = start;

                for (_, c) in self.chars.by_ref() {
                    if escaped {
                        escaped = false;
                    } else if c == '\\' {
                        escaped = true;
                    } else if c == '"' {
                        break;
                    }
                    end += 1;
                }

                let end = (end + 2).min(self.source.len());
                (end, Token::String(&self.source[start..end]))
            }
            _ => {
                let end = self
                    .chars
                    .find(|&(_, c)| c.is_whitespace())
                    .map(|(i, _)| i)
                    .unwrap_or(self.source.len());

                let word = &self.source[start..end];

                let token = if !word.contains(|c: char| !c.is_ascii_digit()) {
                    Token::Integer(word.parse().unwrap())
                } else {
                    Token::Symbol(word)
                };

                (end, token)
            }
        };

        Some(Word::new(token, Span::new(start, end)))
    }
}

impl<'src> Iterator for Lexer<'src> {
    type Item = Word<'src>;

    fn next(&mut self) -> Option<Self::Item> {
        self.word()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl<'src> PartialEq for Word<'src> {
        fn eq(&self, other: &Self) -> bool {
            self.span.start == other.span.start
                && self.span.end == other.span.end
                && match (self.token, other.token) {
                    (Token::Integer(a), Token::Integer(b)) => a == b,
                    (Token::Symbol(a), Token::Symbol(b)) => a == b,
                    _ => false,
                }
        }
    }

    #[test]
    fn tokenize_numbers() {
        let source = "1 2 34   90 3475 690173  9876543210  000001";
        let mut lexer = Lexer::new(source);

        assert_eq!(
            lexer.next(),
            Some(Word::new(Token::Integer(1), Span::new(0, 1)))
        );
        assert_eq!(
            lexer.next(),
            Some(Word::new(Token::Integer(2), Span::new(2, 3)))
        );
        assert_eq!(
            lexer.next(),
            Some(Word::new(Token::Integer(34), Span::new(4, 6)))
        );
        assert_eq!(
            lexer.next(),
            Some(Word::new(Token::Integer(90), Span::new(9, 11)))
        );
        assert_eq!(
            lexer.next(),
            Some(Word::new(Token::Integer(3475), Span::new(12, 16)))
        );
        assert_eq!(
            lexer.next(),
            Some(Word::new(Token::Integer(690173), Span::new(17, 23)))
        );
        assert_eq!(
            lexer.next(),
            Some(Word::new(Token::Integer(9876543210), Span::new(25, 35)))
        );
        assert_eq!(
            lexer.next(),
            Some(Word::new(Token::Integer(1), Span::new(37, 43)))
        );
    }
}
