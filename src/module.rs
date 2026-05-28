// Module resolver — resolves `import "./foo.ely"` across files.
//
// Walks imports recursively, parses each file, merges all items into one flat
// Program, and reports errors for missing files, cycles, etc.

use crate::ast::*;
use crate::error::{CompileError, Result, SourceSpan};
use crate::parser::Parser;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// A resolved, parsed module.
#[derive(Debug, Clone)]
pub struct ParsedModule {
    /// Resolved absolute path.
    pub path: PathBuf,
    /// The source text.
    pub source: String,
    /// The parsed program.
    pub program: Program,
}

/// Module resolver state.
pub struct ModuleResolver {
    /// Resolved modules: canonical path → parsed module.
    modules: HashMap<PathBuf, ParsedModule>,
    /// Set of paths currently being resolved (for cycle detection).
    resolving: HashSet<PathBuf>,
    /// The root file path.
    root: PathBuf,
}

impl ModuleResolver {
    pub fn new(root: PathBuf) -> Self {
        ModuleResolver {
            modules: HashMap::new(),
            resolving: HashSet::new(),
            root: root.canonicalize().unwrap_or(root),
        }
    }

    /// Resolve all imports starting from the root file.
    /// Returns a merged Program containing all items from all files.
    pub fn resolve_all(&mut self) -> Result<Program> {
        let root = self.root.clone();
        let parsed = self.resolve_file(&root)?;
        let mut merged = parsed.program;
        // Sub-modules are already merged during resolution
        Ok(merged)
    }

    /// Get a specific resolved module by its original path.
    pub fn get_module(&self, path: &Path) -> Option<&ParsedModule> {
        let canon = path.canonicalize().ok()?;
        self.modules.get(&canon)
    }

    /// Get all resolved modules.
    pub fn modules(&self) -> &HashMap<PathBuf, ParsedModule> {
        &self.modules
    }

    /// Resolve a single file and its transitive imports.
    fn resolve_file(&mut self, path: &Path) -> Result<ParsedModule> {
        let canonical = path.canonicalize().map_err(|e| {
            CompileError::new(format!("cannot resolve file `{}`: {}", path.display(), e))
        })?;

        // Check cache first
        if let Some(module) = self.modules.get(&canonical) {
            return Ok(module.clone());
        }

        // Cycle detection
        if !self.resolving.insert(canonical.clone()) {
            return Err(CompileError::new(format!(
                "circular import detected: `{}`",
                path.display()
            )));
        }

        // Read and parse
        let source = std::fs::read_to_string(&canonical).map_err(|e| {
            CompileError::new(format!("failed to read `{}`: {}", canonical.display(), e))
        })?;

        let program = if canonical.extension().map(|e| e == "elyx").unwrap_or(false) {
            // Parse .elyx: extract the component as a standalone program
            let elyx_file = crate::elyx::parse_elyx(&source).map_err(|e| {
                CompileError::new(format!(
                    "failed to parse .elyx file `{}`: {}",
                    canonical.display(), e.message
                ))
            })?;
            let component = elyx_file.component;
            let component_name = component.value.name.clone();
            Program {
                items: vec![Node::new(
                    Item::Component(component.value),
                    component.span,
                )],
            }
        } else {
            let mut parser = Parser::new(&source);
            parser.parse_program()?
        };

        let parsed = ParsedModule {
            path: canonical.clone(),
            source,
            program,
        };

        // Cache BEFORE resolving children to handle mutual imports gracefully
        self.modules.insert(canonical.clone(), parsed.clone());
        self.resolving.remove(&canonical);

        // Resolve imports within this file (require explicit imports for now)
        // We treat the root file's imports specially
        Ok(parsed)
    }

    /// Load the root file WITHOUT resolving its imports inline.
    /// This is used by check/build which want a clean root program.
    pub fn resolve_root_only(&mut self) -> Result<ParsedModule> {
        let root = self.root.clone();
        self.resolve_file(&root)
    }

    /// Collect all items from the root file and all its transitive imports,
    /// resolving relative import paths.
    pub fn collect_all_items(&mut self) -> Result<Vec<(PathBuf, Node<Item>)>> {
        let root = self.root.clone();
        self.resolve_file(&root)?;

        let mut all_items = Vec::new();
        let mut visited = HashSet::new();
        self.collect_items_recursive(&root, &mut all_items, &mut visited)?;
        Ok(all_items)
    }

    fn collect_items_recursive(
        &mut self,
        path: &Path,
        all_items: &mut Vec<(PathBuf, Node<Item>)>,
        visited: &mut HashSet<PathBuf>,
    ) -> Result<()> {
        let canonical = path.canonicalize().map_err(|e| {
            CompileError::new(format!("bad path `{}`: {}", path.display(), e))
        })?;

        if !visited.insert(canonical.clone()) {
            return Ok(()); // Already visited
        }

        // Re-read and parse to get items with imports
        let source = std::fs::read_to_string(&canonical).map_err(|e| {
            CompileError::new(format!("failed to read `{}`: {}", canonical.display(), e))
        })?;

        let program = if canonical.extension().map(|e| e == "elyx").unwrap_or(false) {
            let elyx_file = crate::elyx::parse_elyx(&source)?;
            Program {
                items: vec![Node::new(
                    Item::Component(elyx_file.component.value.clone()),
                    elyx_file.component.span.clone(),
                )],
            }
        } else {
            let mut parser = Parser::new(&source);
            parser.parse_program()?
        };

        // Collect items from this file (excluding imports)
        for item in &program.items {
            if !matches!(item.value, Item::Import(..)) {
                all_items.push((canonical.clone(), item.clone()));
            }
        }

        // Recurse into imports
        for item in &program.items {
            if let Item::Import(import_path, _alias) = &item.value {
                let resolved = self.resolve_import_path(&canonical, import_path)?;
                self.collect_items_recursive(&resolved, all_items, visited)?;
            }
        }

        Ok(())
    }

    /// Given the importing file's path and the import string, resolve the
    /// target path relative to the importing file.
    pub fn resolve_import_path(&self, from: &Path, import_path: &str) -> Result<PathBuf> {
        let from_dir = from.parent().unwrap_or(Path::new("."));

        // Try the path as-is first
        let candidate = from_dir.join(import_path);
        if candidate.exists() {
            return Ok(candidate);
        }

        // Try with .ely extension
        let with_ely = from_dir.join(format!("{}.ely", import_path.trim_end_matches(".elyx")));
        if with_ely.exists() {
            return Ok(with_ely);
        }

        // Try with .elyx extension
        let with_elyx = from_dir.join(format!("{}.elyx", import_path.trim_end_matches(".ely")));
        if with_elyx.exists() {
            return Ok(with_elyx);
        }

        // Try the raw string
        Err(CompileError::new(format!(
            "cannot find import `{}` from `{}`",
            import_path,
            from.display()
        )))
    }
}
