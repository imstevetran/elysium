use codespan_reporting::diagnostic::{Diagnostic, Label, Severity};
use codespan_reporting::files::SimpleFile;
use codespan_reporting::term;
use termcolor::StandardStream;

pub type Spanned<T> = (T, SourceSpan);

#[derive(Debug, Clone, PartialEq)]
pub struct SourceSpan {
    pub offset: usize,
    pub length: usize,
}

impl SourceSpan {
    pub fn new(offset: usize, length: usize) -> Self {
        Self { offset, length }
    }
}

#[derive(Debug)]
pub struct CompileError {
    pub message: String,
    pub span: Option<SourceSpan>,
    pub help: Option<String>,
}

impl CompileError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            span: None,
            help: None,
        }
    }

    pub fn with_span(message: impl Into<String>, span: SourceSpan) -> Self {
        Self {
            message: message.into(),
            span: Some(span),
            help: None,
        }
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    pub fn emit(&self, source: &str, file_path: &str) {
        let stderr = StandardStream::stderr(termcolor::ColorChoice::Auto);
        let file = SimpleFile::new(file_path, source);

        let mut labels = Vec::new();
        if let Some(span) = &self.span {
            labels.push(
                Label::primary((), span.offset..span.offset + span.length)
                    .with_message(&self.message),
            );
        }

        let mut diagnostic = Diagnostic::new(Severity::Error)
            .with_message(&self.message)
            .with_labels(labels);

        if let Some(help) = &self.help {
            diagnostic = diagnostic.with_notes(vec![format!("help: {}", help)]);
        }

        term::emit(&mut stderr.lock(), &term::Config::default(), &file, &diagnostic)
            .unwrap();
    }
}

pub type Result<T> = std::result::Result<T, CompileError>;
