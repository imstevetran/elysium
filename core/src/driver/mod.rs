//! Compiler driver: parse, desugar, compile, and CLI command implementations.

pub mod cli;

mod commands;
mod compile;
mod desugar;
mod elyx_cmd;
mod imports;
mod source;
mod stubs;

pub use commands::{
    cmd_test, dep_graph_file, doc_file, gen_test_file, highlight_file, lint_file, port_file,
};
pub use compile::{check_file, compile_and_run, compile_file};
pub use elyx_cmd::{build_elyx, check_elyx};
pub use source::is_elyx_file;
pub use stubs::resolve_env_alias;
