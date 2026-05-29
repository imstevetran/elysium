# Agent Guidelines — Elysium 2.0

## Design Decisions

### Spec-Driven Development Keywords
- `spec` / `describe` — test suite (both work, synonyms)
- `feat` / `it` — individual test case (both work, synonyms)
- `expect <expr>` — assertion statement
- `todo ["message"]` — todo marker in specs, compiles to nil
- `question ["message"]` — open question/concern marker, compiles to nil

The `question` keyword was chosen over `oq` or `concern` because it's the most intuitive plain-English word. Since `?` already uses `Token::Question` in the lexer, the keyword token is `Token::KwQuestion`.

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
- `epm install [package]` — install all deps or a specific package
- `epm publish` — tarball the package and push to registry
- `epm search <query>` — search registry by name or description
- `epm info <package>` — show package details and versions
- `epm tree` — show the full dependency tree of installed packages
- `epm shake [--dry-run]` — tree-shake installed packages (remove unreachable .ely files)
- `epm install --shake` — install and tree-shake in one step
- `epm login <token>` — store GitHub PAT for publish auth
- `epm list` — list installed packages in `elysium_modules/`
- Manifest file: `elysium.json` (name, version, description, entry, license, author, repository, dependencies)
- Registry lives at `https://github.com/imstevetran/epm-registry.git`
- Registry structure: `registry.json` (JSON index of all packages) + `packages/` (tarballs)
- EPM caches the registry clone in `~/.epm/.epm-registry/`
- Token stored in `~/.epm/token` with `chmod 600`
- Published tarballs exclude: `elysium_modules/`, `.git/`, `.epm/`, `target/`, `Cargo.lock`
- Tree-shaking: walks dependency tree, follows `import` statements from each package's entry point,
  removes any `.ely` file not reachable via the import graph
