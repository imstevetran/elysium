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
    #[token("extension")]
    Extension,

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
    #[token("stub")]
    Stub,
    #[token("switch")]
    Switch,
    #[token("private")]
    Private,
    #[token("lazy")]
    Lazy,
    #[token("parallel")]
    Parallel,
    #[token("schedule")]
    Schedule,
    #[token("wait")]
    Wait,
    #[token("worker")]
    Worker,
    #[token("is")]
    Is,

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
    /// Backtick strings: multi-line, can contain " but not backticks.
    /// E.g. `{"key": "value", "nested": [1, 2]}`
    #[regex(r#"`[^`]*`"#, |lex| {
        let s = lex.slice();
        Some(s[1..s.len()-1].to_string())
    })]
    BacktickString(String),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn tokenize(input: &str) -> Vec<Token> {
        TokenStream::new(input).map(|(_, t, _)| t).collect()
    }

    macro_rules! assert_token {
        ($left:expr, $right:pat) => {
            match &$left {
                $right => {},
                other => panic!("expected pattern, got {:?}", other),
            }
        };
    }

    #[test]
    fn test_lex_spec_keyword() {
        let toks = tokenize("spec");
        assert_eq!(toks.len(), 1);
        assert_eq!(toks[0], Token::Spec);
    }

    #[test]
    fn test_lex_describe_keyword() {
        let toks = tokenize("describe");
        assert_eq!(toks.len(), 1);
        assert_eq!(toks[0], Token::Describe);
    }

    #[test]
    fn test_lex_feat_keyword() {
        let toks = tokenize("feat");
        assert_eq!(toks.len(), 1);
        assert_eq!(toks[0], Token::Feat);
    }

    #[test]
    fn test_lex_it_keyword() {
        let toks = tokenize("it");
        assert_eq!(toks.len(), 1);
        assert_eq!(toks[0], Token::It);
    }

    #[test]
    fn test_lex_expect_keyword() {
        let toks = tokenize("expect");
        assert_eq!(toks.len(), 1);
        assert_eq!(toks[0], Token::Expect);
    }

    #[test]
    fn test_lex_todo_keyword() {
        let toks = tokenize("todo");
        assert_eq!(toks.len(), 1);
        assert_eq!(toks[0], Token::Todo);
    }

    #[test]
    fn test_lex_question_keyword() {
        let toks = tokenize("question");
        assert_eq!(toks.len(), 1);
        assert_eq!(toks[0], Token::KwQuestion);
    }

    #[test]
    fn test_lex_bench_keyword() {
        let toks = tokenize("bench");
        assert_eq!(toks.len(), 1);
        assert_eq!(toks[0], Token::Bench);
    }

    #[test]
    fn test_lex_bm_keyword() {
        let toks = tokenize("bm");
        assert_eq!(toks.len(), 1);
        assert_eq!(toks[0], Token::Bm);
    }

    #[test]
    fn test_lex_import_as_keywords() {
        let toks = tokenize("import as");
        assert_eq!(toks.len(), 2);
        assert_eq!(toks[0], Token::Import);
        assert_eq!(toks[1], Token::As);
    }

    #[test]
    fn test_lex_spec_suite() {
        let src = r#"spec "my tests" { feat "add" { expect 1 + 1 } }"#;
        let toks = tokenize(src);
        let keywords: Vec<&Token> = toks.iter().filter(|t| matches!(t,
            Token::Spec | Token::Describe | Token::Feat | Token::It
            | Token::Expect | Token::Todo | Token::KwQuestion
            | Token::Bench | Token::Bm | Token::Import | Token::As
        )).collect();
        assert_eq!(keywords.len(), 3);
        assert_eq!(*keywords[0], Token::Spec);
        assert_eq!(*keywords[1], Token::Feat);
        assert_eq!(*keywords[2], Token::Expect);
    }

    #[test]
    fn test_lex_bench_bm() {
        let toks = tokenize("bench { bm { } }");
        let keywords: Vec<&Token> = toks.iter().filter(|t| matches!(t, Token::Bench | Token::Bm)).collect();
        assert_eq!(keywords.len(), 2);
        assert_eq!(*keywords[0], Token::Bench);
        assert_eq!(*keywords[1], Token::Bm);
    }

    #[test]
    fn test_lex_todo_no_message() {
        let toks = tokenize("todo");
        assert_eq!(toks.len(), 1);
        assert_eq!(toks[0], Token::Todo);
    }

    #[test]
    fn test_lex_todo_with_message() {
        let toks = tokenize(r#"todo "fix this later""#);
        assert_eq!(toks.len(), 2); // todo keyword + string literal
        assert_eq!(toks[0], Token::Todo);
        assert_token!(toks[1], Token::StringLiteral(_));
    }

    #[test]
    fn test_lex_question_with_message() {
        let toks = tokenize(r#"question "why is this here?""#);
        assert_eq!(toks[0], Token::KwQuestion);
        assert_token!(toks[1], Token::StringLiteral(_));
    }

    #[test]
    fn test_lex_parallel_keyword() {
        let toks = tokenize("parallel");
        assert_eq!(toks.len(), 1);
        assert_eq!(toks[0], Token::Parallel);
    }

    #[test]
    fn test_lex_parallel_block() {
        let toks = tokenize("parallel {\n    print(1)\n}");
        let keywords: Vec<&Token> = toks.iter().filter(|t| matches!(t, Token::Parallel)).collect();
        assert_eq!(keywords.len(), 1);
        assert_eq!(*keywords[0], Token::Parallel);
    }
}
