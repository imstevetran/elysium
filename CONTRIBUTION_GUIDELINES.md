# Contribution Guidelines

## Coding Conventions

### Naming
- **Functions & variables**: `camelCase` — e.g. `func fetchData()`, `let userCount`
- **Types, classes, enums, components**: `PascalCase` — e.g. `class UserProfile`, `enum Color`, `component TodoList`
- **Enum variants**: `PascalCase` — e.g. `Circle(radius)`, `Rectangle(w, h)`
- **File names**: `snake_case.ely` for library files, `kebab-case.ely` for examples
- **Constants**: `SCREAMING_SNAKE_CASE` for compile-time constant-like values
- **Internal/private items**: prefix with `__` for compiler-generated names (e.g. `__console_print`, `__langchain_llm`)

### Documentation
- Every public function, class, enum, component, and typealias **must** have a `/// Summary:` doc comment
- Format: `/// Summary: <one-line description>`
- Optionally: `/// Parameters: <name> - <desc>` for each parameter
- Optionally: `/// Returns: <description>` for return values
- Internal/private items should have doc comments where the purpose is not obvious
- Use `bc "reason"` annotations on functions with bodies longer than 3 statements to explain purpose

### Formatting
- **Indentation**: 4 spaces (no tabs)
- **Braces**: opening brace on the same line for blocks (`func name() {`)
- **Max line length**: aim for under 100 characters
- **Spacing**: one space after commas, around binary operators, and after control-flow keywords
- **Blank lines**: separate top-level items with a blank line; group related items

### Imports
- Use relative paths: `import "./foo.ely"` not bare names
- Use aliased imports for namespacing: `import "./math.ely" as math`
- Group imports at the top of the file, before any other items

### Error Handling
- Use `Result<T, E>` return types for operations that can fail
- Use `?` operator for early propagation of errors
- Use `try { } catch { } finally { }` for error handling with cleanup
- Use `bc condition, "message"` for runtime assertions

### Specs / Testing
- Use `spec "Name" { feat "description" { expect expr } }` for unit tests
- Use `describe "Name" { it "desc" { expect expr } }` as an alternative style
- Use `todo` / `todo "message"` to mark unimplemented tests
- Use `bench { ... }` / `bm { ... }` for performance benchmarking

### Memory & Ownership
- Favor immutable bindings (`let`) over mutable (`var`)
- Use `only let` for exclusive ownership when a variable should not be aliased
- Use `weak`/`unowned` references to break reference cycles
- Wrap unsafe operations in `unsafe { }` blocks

## Adding a Builtin Package

Builtin packages (like `ble`, `langchain`, `regex`, `zigbee`) follow a **7-layer pattern**:

### Layer 1: JS Runtime (`npm-package/runtime/<name>.js`)
Implement all functions in JavaScript. Each function receives raw string/Int arguments and returns strings (JSON-encoded where needed for Elysium's type system). Export all functions via `module.exports`.

Use environment variables for configuration (e.g. `OPENAI_API_KEY`, `OPENAI_BASE_URL`). Fall back to mock output when required config is missing.

### Layer 2: Export (`npm-package/runtime/index.js`)
Add `const <name>Mod = require('./<name>');` and include `<name>: <name>Mod` in `module.exports`.

### Layer 3: Desugaring (`core/src/driver/desugar.rs`)
In `desugar_builtin_in_expr`, add the package name to the match block:
```rust
"<name>" => "__<name>_",
```

This converts `<name>.method(args)` to `__<name>_method(args)`.

### Layer 4: Type Checker (`core/src/middle/type_checker.rs`)
In `register_builtins()`, register all `__<name>_*` builtins with their parameter types and return types. Follow the existing pattern of grouping by return type (String, Bool, Int, Nil).

### Layer 5: MIR Enum (`core/src/backend/mir.rs`)
Add a new variant to `MirStmt`:
```rust
<Name>Call {
    result: Option<String>,
    method: String,
    args: Vec<MirValue>,
    dbg_line: u32,
},
```

### Layer 6: MIR Lowering (`core/src/backend/mir.rs`)
In the `MirLowerer::lower_stmt`, add handling for `__<name>_` prefixed calls in both:
- `HirStmt::Let` — when the result is assigned to a variable (use `result: Some(name)`)
- `HirStmt::Expr` — when called as a standalone expression (use `result: None`)

### Layer 7: Codegen (`core/src/backend/codegen.rs`)
- Add dispatch in `emit_stmt` for the new `MirStmt::<Name>Call` variant
- Add dispatch in `emit_stmt_in_wrapper` for parallel block support
- Implement `emit_<name>_call()` that emits `printf` stubs (C backend) — real implementation lives in the JS runtime

### Verification
```bash
cargo check           # Compiles clean
cargo test            # All tests pass
cargo run -- check examples/<name>.ely  # Type-check passes
node -e "const e = require('./npm-package/runtime/index'); console.log(typeof e.<name>)"  # JS module loads
```

## Adding a Package to EPM

1. Create an `elysium.json` manifest:
   ```json
   {
     "name": "my-package",
     "version": "0.1.0",
     "entry": "main.ely",
     "dependencies": {}
   }
   ```
2. Write library code in `.ely` files
3. Publish: `epm publish`
4. Consume: `epm install my-package`

## Release Process

1. Run `./scripts/release.sh <version>` (e.g. `./scripts/release.sh 0.2.0`)
2. Push both the commit and tag:
   ```
   git push origin main
   git push origin v<version>
   ```
3. The CI release workflow automatically:
   - Builds native binaries for Linux x64, macOS ARM64, Windows x64
   - Creates a GitHub Release with binaries attached
   - Publishes to npm via Trusted Publishing (OIDC)
   - Docs mirror to `elysiumlang.github.io` is handled by `docs-mirror.yml` on `main` (not on release tags)
