# Agent Guidelines — Elysium 2.0

## Design Decisions

### Spec-Driven Development Keywords
- `spec` / `describe` — test suite (both work, synonyms)
- `feat` / `it` — individual test case (both work, synonyms)
- `expect <expr>` — assertion statement
- `todo ["message"]` — todo marker in specs, compiles to nil
- `question ["message"]` — open question/concern marker, compiles to nil

The `question` keyword was chosen over `oq` or `concern` because it's the most intuitive plain-English word. Since `?` already uses `Token::Question` in the lexer, the keyword token is `Token::KwQuestion`.

### Private, Lazy, and Class Keywords
- `private` — access modifier for functions, class fields, and class methods
- `lazy` — modifier for functions and let variables (deferred/lazy evaluation)
- `init` — constructor method within a class body
- `class` — struct-like class definition with fields and methods
- Syntax:
  - `private func foo()` — private top-level function
  - `lazy func foo()` — lazy function (evaluated on first call)
  - `private lazy func foo()` — both modifiers
  - `lazy let x = expr` — lazy variable (evaluated on first access)
  - `class Foo { let x: Int; private let y: String; func bar() {}; private func baz() {} }`
  - `init(x: Int) {}` — constructor method inside a class
- Pipeline: class methods are lowered as standalone functions in HIR
- Lazy let uses `is_lazy: bool` on `HirStmt::Let` and `MirStmt::Alloca`; store is deferred to first access
- Private is tracked on `Function.is_private`, `ClassField.is_private`, and `Let.is_private`; type checker uses it for access control

### Import System
- `import "./path.ely"` — basic import (items are inlined)
- `import "./path.ely" as alias` — aliased import (desugars `alias.fn()` → `alias_fn()`)

### Benchmarking
- `bench { ... }` / `bm { ... }` — benchmark block that measures wall-clock time
- Both are synonyms; `bench` is the full keyword, `bm` is the shorthand
- Implementation emits LLVM IR with `clock_gettime(CLOCK_MONOTONIC, ...)` calls around the body block
- Output: `bench: %.6f s\n` via `printf` to stdout
- The body's statements are compiled inline between start/end timing calls
- Timespec struct is allocated as an `[2 x i64]` on the stack

### EPM (Elysium Package Manager)
- `epm init [name]` — scaffold a new package (creates `elysium.json` + `main.ely`)
- `epm install [package] [--save] [--shake] [--legacy]` — install all deps or a specific package
  - Default mode: flat dependency resolution (one version per package name, best candidate selected)
  - `--legacy`: allow multiple versions of the same dependency
- `epm lock` — generate `elysium.lock` from current flat resolution (also auto-generated during `epm install`)
- `epm publish` — tarball the package and push to registry
- `epm search <query>` — search registry by name or description
- `epm info <package>` — show package details and versions
- `epm tree` — show the flat dependency tree (reads `elysium.lock` first if available)
- `epm why <package>` — trace dependency paths from root to a package, showing how it's pulled in
- `epm shake [--dry-run]` — tree-shake installed packages (remove unreachable .ely files)
- `epm install --shake` — install and tree-shake in one step
- `epm login <token>` — store GitHub PAT for publish auth
- `epm list` — list installed packages in `elysium_modules/`
- `epm --env-file <path> <command>` — use a custom .env file (defaults to `.env` in current directory)
- Manifest file: `elysium.json` (name, version, description, entry, license, author, repository, dependencies)
- Lockfile: `elysium.lock` (auto-generated, stores resolved version for each dep, checked in to git)
- Registry lives at `https://github.com/imstevetran/epm-registry.git`
- Registry structure: `registry.json` (JSON index of all packages) + `packages/` (tarballs)
- EPM caches the registry clone in `~/.epm/.epm-registry/` and fetched manifests in `~/.epm/manifests/`
- Token stored in `~/.epm/token` with `chmod 600`
- Published tarballs exclude: `elysium_modules/`, `.git/`, `.epm/`, `target/`, `Cargo.lock`, `elysium.lock`, `.env`
- Tree-shaking: walks dependency tree, follows `import` statements from each package's entry point,
  removes any `.ely` file not reachable via the import graph
- `.env` support: `epm` loads `.env` from the current directory on every command. Use `--env-file <path>` to specify a different file. The token can also be set via `EPM_GIT_TOKEN` env var instead of `epm login`.

### npm Package (`elysium-lang`)
- Located in `npm-package/` directory
- Published as `elysium-lang` on npm
- Ships:
  - **CLI binary** (`elysium` or `ely` command) — the full Rust compiler, downloaded/built during `npm install`
  - **JavaScript runtime** (`require('elysium-lang')`) — mirrors `elysium-rt` Rust crate as JS
- **Runtime modules**:
  - `arc.js` — `Ref`, `Weak`, `Unowned` (reference counting)
  - `task.js` — `Task`, `Scheduler` (async task scheduling via `setImmediate`)
  - `channel.js` — `Channel` (async message passing with `EventEmitter`)
  - `ui.js` — `View`, `Style`, `ComponentState`, `diff`, `Patch`, `Axis` (virtual DOM diffing)
- **Install flow** (`postinstall`):
  1. Try to download prebuilt `.gz` binary from GitHub Releases (`imstevetran/elysium`)
  2. Fall back to `cargo build --release` if no binary available
- **Build for release**: `node scripts/build-binaries.js [--all]` — cross-compiles and gzips per-platform
- Platform targets: `x86_64` and `aarch64` for macOS, Linux, Windows

### Flat Dependency Resolution (epm resolver)
- Default mode: single version per package name
- Resolution algorithm:
  1. Walk all transitive dependencies to gather version constraints
  2. For each package name, find the highest semver version satisfying ALL constraints
  3. Generate `elysium.lock` pinning each package to its resolved version
- `--legacy` mode: resolves each version constraint independently
  - First-come-first-served: the first requirement encountered wins when a name is already resolved
- Uses `semver` crate for version matching:
  - Bare `"1.0.0"` → `^1.0.0` (compatible with same major)
  - Explicit constraints like `">=1.0.0 <2.0.0"` are parsed directly
  - `"*"` or empty → any version

### Stub Functions & Environment Filtering
- `func foo() -> Int stub` — declares a stub function (no body) available in all environments
- `func foo() -> Int stub: [local, dev]` — declares a stub restricted to specific environments
- Supported built-in envs: `local` (default), `dev`, `test`, `prod`
- Custom aliases can be defined in `elysium.json` under the `environments` field (e.g. `"staging": "dev"`)
- CLI flags: `--env` on `build`, `run`, `check` (default: `local`)
- The compiler pipeline:
  1. Parsed `stub` keyword produces `stub_envs: Option<Vec<String>>` on `Function` AST node
  2. Bare `stub` (no env list) → `Some(vec![])` (matches all environments)
  3. `stub: [local, test]` → `Some(vec!["local", "test"])`
  4. Before type-checking/codegen, `filter_stubs()` removes functions whose env list doesn't match `--env`
  5. Type checker, ownership checker, and linter skip body-walking for stub functions
  6. Stub functions have empty `body: Block { statements: [] }` (no code generated)
  7. HIR/MIR lowering and codegen handle empty bodies naturally — just emit a function with a default return

### Switch/Case/Then/Else (added May 2026)
- `switch` is a user-friendly alias for `match` — desugars directly into the existing `Match`/`MatchExpression` AST nodes, so the entire pipeline (HIR, MIR, codegen) works unchanged.
- Syntax: `switch expr { case pattern then { body } else { body } }`
  - Uses `then` keyword (already exists in language) instead of `->` arrow
  - `else` block becomes a `Wildcard` pattern arm
  - Supports both statement and expression forms
- `parse_pattern` supports: `_` (wildcard as `Pattern::Wildcard`), integers, floats, bools, nil, strings, identifiers (bindings), enum variants, `only Type`

### Unified Logging — Console (added May 2026)
- `console.debug("msg")`, `console.info("msg")`, `console.warn("msg")`, `console.error("msg")`, `console.log("msg")` are supported syntax
- Also `print(x)` is supported
- **Backend (compiled binary)**: `console.*` desugars to `__console_*` builtins → MIR `ConsoleCall` → LLVM `printf` with `[DEBUG]`, `[INFO]`, `[WARN]`, `[ERROR]` prefixes
- **Client-side / Node**: `npm-package/runtime/console.js` maps to native `console.*` API with environment detection (browser vs Node vs fallback)
- Desugaring happens in `main.rs` via `desugar_console_calls()` → `desugar_console_in_expr()` which converts `console.method(...)` → `__console_method(...)` and `print` → `__console_print`
- Type checker has `__console_*` registered as `(Infer) → Nil` builtins
- Codegen `emit_console_call()` builds a printf format string with the prefix, formats each arg as `%s`, and appends `\n` for debug/info/warn/error/log (not for bare print)
- JS runtime `console.js` exports `{ debug, info, warn, error, log }` — access via `require('elysium-lang/runtime/console')` or `require('elysium-lang').console`

### Filesystem Package (added May 2026)
- `fs.readFile("path")`, `fs.writeFile("path", "content")`, `fs.exists("path")`, `fs.removeFile("path")`,
  `fs.createDir("path")`, `fs.removeDir("path")`, `fs.copyFile("src", "dst")`, `fs.rename("old", "new")`,
  `fs.appendFile("path", "content")` are supported via method-call syntax on `fs.`
- Desugaring in `main.rs` (`desugar_builtin_calls`) converts `fs.method(...)` → `__fs_method(...)`
- Type checker registers `__fs_*` builtins with type-specific signatures:
  - `__fs_readFile`, `__fs_readFileSync`: `(String) → String`
  - `__fs_writeFile`, `__fs_appendFile`: `(String, String) → Nil`
  - `__fs_exists`, `__fs_isFile`, `__fs_isDir`: `(String) → Bool`
  - `__fs_removeFile`, `__fs_createDir`, `__fs_removeDir`: `(String) → Nil`
  - `__fs_copyFile`, `__fs_rename`: `(String, String) → Nil`
- MIR: Uses `MirStmt::FsCall { result: Option<String>, method, args, dbg_line }` for both void and return-value contexts
- Codegen (`emit_fs_call`): calls C stdlib functions directly (`fopen`, `fgets`/`fputs`, `fclose`, `remove`, `access`, `mkdir`, `rmdir`, `rename`)
- JS runtime `npm-package/runtime/fs.js` maps to Node's native `fs` module (readFileSync, writeFileSync, etc.)
- Exported via `require('elysium-lang').fs`

### Transport Package (added May 2026)
- Networking utilities via `transport.` method-call syntax: HTTP, WebSocket, MQTT
- Desugaring in `main.rs` (`desugar_builtin_calls`) converts `transport.method(...)` → `__transport_method(...)`
- Type checker registers `__transport_*` builtins with typed signatures:
  - HTTP: `get(url: String) → String`, `post/put(url: String, body: String) → String`, `delete(url: String) → String`
  - WebSocket: `wsConnect(url: String) → String`, `wsSend(conn: String, data: String) → Nil`, `wsClose(conn: String) → Nil`
  - MQTT: `mqttConnect(broker: String, clientId: String) → String`, `mqttPublish(client: String, topic: String, msg: String) → Nil`, `mqttSubscribe(client: String, topic: String) → Nil`, `mqttDisconnect(client: String) → Nil`
- MIR: Uses `MirStmt::TransportCall { result, method, args, dbg_line }` for both void and return-value contexts
- Codegen (`emit_transport_call`): prints a stub message `[transport] method: use JS runtime\n` — transport is JS-runtime-only
- JS runtime `npm-package/runtime/transport.js` provides:
  - **HTTP** via `get`, `post`, `put`, `delete` using `fetch` (Node 18+ / browser) or `node-fetch` polyfill
  - **WebSocket** via `wsConnect`, `wsSend`, `wsClose` using `WebSocket` (browser) or `ws` package (Node)
  - **MQTT** via `mqttConnect`, `mqttPublish`, `mqttSubscribe`, `mqttDisconnect` using `mqtt` package (Node-only)
  - **Status codes** via `transport.status.OK`, `status.NOT_FOUND`, etc.
- Exported via `require('elysium-lang').transport`
