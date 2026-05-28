// Syntax highlighter for Elysium 2.0
// Uses the existing lexer tokens and separately extracts comments/whitespace.

use crate::lexer::{Token, TokenStream};
use termcolor::{Color, ColorSpec, StandardStream, WriteColor};
use std::io::Write;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpanKind {
    Keyword,
    Type,
    String,
    Number,
    Comment,
    DocComment,
    Operator,
    Punctuation,
    Identifier,
    Builtin,
    Normal,
}

/// A highlighted span.
#[derive(Debug, Clone)]
pub struct HighlightedSpan {
    pub start: usize,
    pub end: usize,
    pub kind: SpanKind,
    pub text: String,
}

fn is_type_name(name: &str) -> bool {
    matches!(name, "Int" | "Float" | "Bool" | "String" | "Char"
        | "Option" | "Result" | "Array" | "Nil" | "Self")
}

fn is_builtin_func(name: &str) -> bool {
    matches!(name, "print" | "println" | "sum" | "min" | "max"
        | "abs" | "len" | "count" | "isEmpty" | "map" | "filter" | "reduce")
}

fn token_kind(tok: &Token) -> Option<SpanKind> {
    match tok {
        // Keywords
        Token::Let | Token::Var | Token::Func
        | Token::If | Token::Else | Token::Then
        | Token::For | Token::In | Token::While
        | Token::Return | Token::Match | Token::Case
        | Token::Try | Token::Catch | Token::Finally
        | Token::Async | Token::Await | Token::Class
        | Token::Init | Token::Enum | Token::Component
        | Token::State | Token::Bc | Token::Because
        | Token::Only | Token::Unsafe | Token::Weak
        | Token::Unowned | Token::Typealias
        | Token::Import | Token::As | Token::Do | Token::Render
        | Token::Spec | Token::Describe | Token::Feat | Token::It | Token::Expect
        | Token::Todo | Token::KwQuestion
        | Token::Bench | Token::Bm
        | Token::True | Token::False | Token::Nil => Some(SpanKind::Keyword),

        // String literals
        Token::StringLiteral(_) => Some(SpanKind::String),
        Token::CharLiteral(_) => Some(SpanKind::String),

        // Numbers
        Token::IntLiteral(_) | Token::FloatLiteral(_) => Some(SpanKind::Number),

        // Identifiers
        Token::Identifier(name) => {
            if is_type_name(name) {
                Some(SpanKind::Type)
            } else if is_builtin_func(name) {
                Some(SpanKind::Builtin)
            } else {
                Some(SpanKind::Identifier)
            }
        }

        // Operators
        Token::Assign | Token::Plus | Token::Minus
        | Token::Star | Token::Slash | Token::Percent
        | Token::EqEq | Token::NotEq | Token::Lt
        | Token::Gt | Token::LtEq | Token::GtEq
        | Token::AndAnd | Token::OrOr | Token::Bang
        | Token::Arrow | Token::LeftArrow
        | Token::Ellipsis | Token::Ellipsis2
        | Token::DotDot => Some(SpanKind::Operator),

        // Punctuation
        Token::Dot | Token::Comma | Token::Colon
        | Token::Semicolon | Token::LParen | Token::RParen
        | Token::LBracket | Token::RBracket | Token::LBrace
        | Token::RBrace | Token::Pipe | Token::Question => Some(SpanKind::Punctuation),

        // Comments are skipped by the lexer — handled separately
        Token::DocComment | Token::LineComment | Token::BlockComment => unreachable!(),
        Token::Whitespace => Some(SpanKind::Normal),
        Token::Error => None,
    }
}

/// Extract all comments from source text.
fn extract_comments(source: &str) -> Vec<HighlightedSpan> {
    let mut spans = Vec::new();

    let lines: Vec<&str> = source.lines().collect();
    let mut in_block_comment = false;
    let mut block_start = 0;

    for (line_idx, line) in lines.iter().enumerate() {
        if in_block_comment {
            if let Some(end_pos) = line.find("*/") {
                // End of block comment
                let start_of_line = source.lines().take(line_idx).map(|l| l.len() + 1).sum::<usize>();
                let content_end = start_of_line + end_pos + 2;
                spans.push(HighlightedSpan {
                    start: block_start,
                    end: content_end,
                    kind: SpanKind::Comment,
                    text: source[block_start..content_end].to_string(),
                });
                in_block_comment = false;
            }
            continue;
        }

        // Check for line comments and doc comments
        if let Some(pos) = line.find("//") {
            let start_of_line = source.lines().take(line_idx).map(|l| l.len() + 1).sum::<usize>();
            let comment_start = start_of_line + pos;

            // Check for block comment opening on same line
            if let Some(bc_pos) = line[pos..].find("/*") {
                let abs_bc_pos = start_of_line + pos + bc_pos;
                if abs_bc_pos < comment_start {
                    // Block comment before line comment — handled below
                    // For now, fall through to check block comment
                }
            }

            let kind = if pos + 3 < line.len() && &line[pos..pos+3] == "///" {
                SpanKind::DocComment
            } else {
                SpanKind::Comment
            };

            spans.push(HighlightedSpan {
                start: comment_start,
                end: comment_start + line[pos..].len(),
                kind,
                text: line[pos..].to_string(),
            });
        }

        // Check for block comments
        if let Some(bc_start) = line.find("/*") {
            let start_of_line = source.lines().take(line_idx).map(|l| l.len() + 1).sum::<usize>();
            let abs_start = start_of_line + bc_start;

            if let Some(bc_end) = line[bc_start..].find("*/") {
                let abs_end = abs_start + bc_end + 2;
                spans.push(HighlightedSpan {
                    start: abs_start,
                    end: abs_end,
                    kind: SpanKind::Comment,
                    text: source[abs_start..abs_end].to_string(),
                });
            } else {
                in_block_comment = true;
                block_start = abs_start;
            }
        }
    }

    // If we're still in a block comment at end, add it
    if in_block_comment {
        spans.push(HighlightedSpan {
            start: block_start,
            end: source.len(),
            kind: SpanKind::Comment,
            text: source[block_start..].to_string(),
        });
    }

    spans
}

/// Merge token spans and comment spans, sorted by position.
fn merge_spans(tokens: Vec<(usize, Token, usize)>, comments: Vec<HighlightedSpan>, source: &str) -> Vec<HighlightedSpan> {
    let mut result: Vec<HighlightedSpan> = Vec::new();

    // Add token spans
    for (start, tok, end) in tokens {
        if let Some(kind) = token_kind(&tok) {
            result.push(HighlightedSpan {
                start,
                end,
                kind,
                text: source[start..end].to_string(),
            });
        }
    }

    // Add comment spans
    result.extend(comments);

    // Sort by position
    result.sort_by_key(|s| s.start);

    // Merge overlapping spans (comments take priority over tokens)
    let mut merged: Vec<HighlightedSpan> = Vec::new();
    for span in result {
        if let Some(last) = merged.last_mut() {
            if span.start < last.end {
                // Overlap — comment takes priority
                if span.kind == SpanKind::Comment || span.kind == SpanKind::DocComment {
                    // Comment overlaps a token — truncate the token
                    last.end = span.start;
                } else {
                    // Token overlaps a comment — skip the token
                    continue;
                }
            }
        }
        merged.push(span);
    }

    // Fill gaps with Normal spans
    let mut filled = Vec::new();
    let mut pos = 0;
    for span in &merged {
        if span.start > pos {
            filled.push(HighlightedSpan {
                start: pos,
                end: span.start,
                kind: SpanKind::Normal,
                text: source[pos..span.start].to_string(),
            });
        }
        filled.push(span.clone());
        pos = span.end;
    }
    if pos < source.len() {
        filled.push(HighlightedSpan {
            start: pos,
            end: source.len(),
            kind: SpanKind::Normal,
            text: source[pos..].to_string(),
        });
    }

    filled
}

/// Tokenize source into highlighted spans.
pub fn tokenize(source: &str) -> Vec<HighlightedSpan> {
    let tokens: Vec<_> = TokenStream::new(source).collect();
    let comments = extract_comments(source);
    merge_spans(tokens, comments, source)
}

/// Print highlighted source to ANSI terminal.
pub fn print_ansi(source: &str) -> std::io::Result<()> {
    let spans = tokenize(source);
    let mut stdout = StandardStream::stdout(termcolor::ColorChoice::Auto);

    for span in &spans {
        let color = match span.kind {
            SpanKind::Keyword => Color::Rgb(0x89, 0x9C, 0xDF),
            SpanKind::Type => Color::Rgb(0x5E, 0xAD, 0x87),
            SpanKind::String => Color::Rgb(0xCE, 0x91, 0x78),
            SpanKind::Number => Color::Rgb(0xD3, 0xBF, 0x78),
            SpanKind::Comment => Color::Rgb(0x9B, 0x9B, 0x9B),
            SpanKind::DocComment => Color::Rgb(0x6C, 0xAE, 0x6C),
            SpanKind::Operator => Color::Rgb(0xAC, 0xAC, 0xDE),
            SpanKind::Punctuation => Color::Rgb(0x82, 0xAA, 0xFF),
            SpanKind::Builtin => Color::Rgb(0x7C, 0xBF, 0xD4),
            SpanKind::Identifier => Color::Rgb(0xD4, 0xD4, 0xD4),
            SpanKind::Normal => Color::Rgb(0xAA, 0xAA, 0xAA),
        };

        let bold = matches!(span.kind, SpanKind::Keyword | SpanKind::Type | SpanKind::Builtin);
        let mut spec = ColorSpec::new();
        spec.set_fg(Some(color));
        if bold {
            spec.set_bold(true);
        }

        stdout.set_color(&spec)?;
        write!(stdout, "{}", span.text)?;
        stdout.reset()?;
    }

    stdout.reset()?;
    writeln!(stdout)?;
    Ok(())
}

/// Convert highlighted source to HTML.
pub fn to_html(source: &str) -> String {
    let spans = tokenize(source);
    let mut html = String::new();

    html.push_str("<pre class=\"elysium-code\"><code>");

    for span in &spans {
        let cls = match span.kind {
            SpanKind::Keyword => "kw",
            SpanKind::Type => "ty",
            SpanKind::String => "str",
            SpanKind::Number => "num",
            SpanKind::Comment => "cm",
            SpanKind::DocComment => "doc",
            SpanKind::Operator => "op",
            SpanKind::Punctuation => "punct",
            SpanKind::Builtin => "builtin",
            SpanKind::Identifier => "id",
            SpanKind::Normal => "",
        };

        let escaped = span
            .text
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");

        if cls.is_empty() {
            html.push_str(&escaped);
        } else {
            html.push_str(&format!("<span class=\"elys-{}\">{}</span>", cls, escaped));
        }
    }

    html.push_str("</code></pre>");
    html
}

/// Generate CSS for HTML output.
pub fn css() -> &'static str {
    r#".elysium-code {
    background: #1e1e2e;
    color: #d4d4d4;
    padding: 16px;
    border-radius: 8px;
    font-family: 'JetBrains Mono', 'Fira Code', monospace;
    font-size: 14px;
    line-height: 1.6;
    overflow-x: auto;
}
.elys-kw { color: #899cdf; font-weight: bold; }
.elys-ty { color: #5ead87; font-weight: bold; }
.elys-str { color: #ce9178; }
.elys-num { color: #d3bf78; }
.elys-cm { color: #9b9b9b; font-style: italic; }
.elys-doc { color: #6cae6c; font-style: italic; }
.elys-op { color: #acacde; }
.elys-punct { color: #82aaff; }
.elys-builtin { color: #7cbfd4; font-weight: bold; }
.elys-id { color: #d4d4d4; }
"#
}
