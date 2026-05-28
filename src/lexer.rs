use logos::Logos;

#[derive(Logos, Debug, Clone, PartialEq)]
pub enum Token {
    // Keywords
    #[token("let")]
    Let,
    #[token("var")]
    Var,
    #[token("func")]
    Func,
    #[token("if")]
    If,
    #[token("else")]
    Else,
    #[token("then")]
    Then,
    #[token("for")]
    For,
    #[token("in")]
    In,
    #[token("while")]
    While,
    #[token("return")]
    Return,
    #[token("match")]
    Match,
    #[token("case")]
    Case,
    #[token("try")]
    Try,
    #[token("catch")]
    Catch,
    #[token("finally")]
    Finally,
    #[token("async")]
    Async,
    #[token("await")]
    Await,
    #[token("class")]
    Class,
    #[token("init")]
    Init,
    #[token("enum")]
    Enum,
    #[token("component")]
    Component,
    #[token("state")]
    State,
    #[token("bc")]
    Bc,
    #[token("because")]
    Because,
    #[token("only")]
    Only,
    #[token("unsafe")]
    Unsafe,
    #[token("weak")]
    Weak,
    #[token("unowned")]
    Unowned,
    #[token("typealias")]
    Typealias,
    #[token("import")]
    Import,
    #[token("as")]
    As,
    #[token("true")]
    True,
    #[token("false")]
    False,
    #[token("nil")]
    Nil,
    #[token("do")]
    Do,
    #[token("render")]
    Render,

    // Testing keywords
    #[token("spec")]
    Spec,
    #[token("describe")]
    Describe,
    #[token("feat")]
    Feat,
    #[token("it")]
    It,
    #[token("expect")]
    Expect,
    #[token("todo")]
    Todo,
    #[token("question")]
    KwQuestion,
    #[token("bench")]
    Bench,
    #[token("bm")]
    Bm,

    // Literals
    #[regex("[0-9]+", |lex| lex.slice().parse().ok())]
    IntLiteral(i64),
    #[regex("[0-9]+\\.[0-9]+", |lex| lex.slice().parse().ok())]
    FloatLiteral(f64),
    #[regex(r#""[^"]*""#, |lex| {
        let s = lex.slice();
        Some(s[1..s.len()-1].to_string())
    })]
    StringLiteral(String),
    #[regex(r#"'[^']'"#, |lex| {
        let s = lex.slice().as_bytes();
        Some(s[1] as char)
    })]
    CharLiteral(char),

    // Identifiers
    #[regex("[a-zA-Z_][a-zA-Z0-9_]*", |lex| Some(lex.slice().to_string()))]
    Identifier(String),

    // Operators and punctuation
    #[token("=")]
    Assign,
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token("%")]
    Percent,
    #[token("==")]
    EqEq,
    #[token("!=")]
    NotEq,
    #[token("<")]
    Lt,
    #[token(">")]
    Gt,
    #[token("<=")]
    LtEq,
    #[token(">=")]
    GtEq,
    #[token("&&")]
    AndAnd,
    #[token("||")]
    OrOr,
    #[token("!")]
    Bang,
    #[token("->")]
    Arrow,
    #[token(".")]
    Dot,
    #[token(",")]
    Comma,
    #[token(":")]
    Colon,
    #[token(";")]
    Semicolon,
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token("|")]
    Pipe,
    #[token("?")]
    Question,
    #[token("...")]
    Ellipsis,
    #[token("…")]
    Ellipsis2,
    #[token("..")]
    DotDot,
    #[token("<-")]
    LeftArrow,

    // Comments (skipped)
    #[regex(r"///[^\n]*", logos::skip)]
    DocComment,
    #[regex(r"//[^\n]*", logos::skip)]
    LineComment,
    #[regex(r"/\*[^*]*\*+([^/*][^*]*\*+)*/", logos::skip)]
    BlockComment,

    // Whitespace (skipped)
    #[regex(r"[ \t\r\n]+", logos::skip)]
    Whitespace,

    // Catch-all for errors
    Error,
}

/// A spanned token for the parser — yields (usize, Token, usize) tuples.
pub struct TokenStream<'a> {
    lexer: logos::Lexer<'a, Token>,
}

impl<'a> TokenStream<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            lexer: Token::lexer(input),
        }
    }
}

impl<'a> Iterator for TokenStream<'a> {
    type Item = (usize, Token, usize);

    fn next(&mut self) -> Option<Self::Item> {
        let tok = self.lexer.next()?;
        let span = self.lexer.span();
        let token = tok.unwrap_or(Token::Error);
        Some((span.start, token, span.end))
    }
}
