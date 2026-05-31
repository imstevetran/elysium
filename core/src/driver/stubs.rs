//! Environment-aware stub filtering.

use std::path::PathBuf;

use crate::ast;
use crate::manifest;

use super::desugar::desugar_module_calls;

/// Resolve environment aliases from the project's elysium.json manifest.
/// Supports built-in envs (local, dev, test, prod) and custom aliases
/// defined in the `environments` field of the manifest.
pub fn resolve_env_alias(file: &PathBuf, env: &str) -> String {
    manifest::resolve_env_alias(file, env)
}

/// Filter out stub functions that don't match the target environment.
/// When a function is `stub: [env1, env2]`, it is only kept if target env matches one of them.
/// When a function is `stub` (no env list), it's a generic stub — kept for all envs (body is empty).
pub fn filter_stubs(mut prog: ast::Program, aliases: &std::collections::HashSet<String>, env: &str) -> ast::Program {
    desugar_module_calls(&mut prog, aliases);
    filter_stubs_raw(prog, env)
}

pub fn filter_stubs_raw(mut program: ast::Program, env: &str) -> ast::Program {
    program.items.retain(|item| {
        match &item.value {
            ast::Item::Function(f) => {
                match &f.stub_envs {
                    Some(envs) => {
                        // Empty envs list = bare stub, keep for all envs
                        // Non-empty list = only keep if target env is in the list
                        envs.is_empty() || envs.iter().any(|e| e == env)
                    }
                    None => true, // Not a stub, always keep
                }
            }
            _ => true, // Non-functions always kept
        }
    });
    program
}

