//! Elysium 2.0 compiler library.

pub mod error;

pub mod frontend;
pub mod middle;
pub mod backend;
pub mod driver;
pub mod epm;
pub mod ui;
pub mod tools;

// Stable `crate::ast` paths for the compiler pipeline and EPM helpers.
pub use frontend::{ast, lexer, parser};
pub use middle::{hir, ownership, type_checker};
pub use backend::{codegen, codegen_tools, mir};
pub use epm::{extension, init, install, manifest, migrate, module, port, publish, update};
pub use tools::{debug, highlighter, linter, test_runner};
pub use driver::cli;
pub use ui::elyx;
