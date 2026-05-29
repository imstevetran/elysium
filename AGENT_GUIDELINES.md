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
- `epm install [package] [--save] [--shake] [--legacy]` — install all deps or a specific package
  - Default mode: flat dependency resolution (one version per package name, best candidate selected)
  - `--legacy`: allow multiple versions of the same dependency
- `epm lock` — generate `elysium.lock` from current flat resolution (also auto-generated during `epm install`)
- `epm publish` — tarball the package and push to registry
- `epm search <query>` — search registry by name or description
- `epm info <package>` — show package details and versions
- `epm tree` — show the flat dependency tree (reads `elysium.lock` first if available)
- `epm shake [--dry-run]` — tree-shake installed packages (remove unreachable .ely files)
- `epm install --shake` — install and tree-shake in one step
- `epm login <token>` — store GitHub PAT for publish auth
- `epm list` — list installed packages in `elysium_modules/`
- Manifest file: `elysium.json` (name, version, description, entry, license, author, repository, dependencies)
- Lockfile: `elysium.lock` (auto-generated, stores resolved version for each dep, checked in to git)
- Registry lives at `https://github.com/imstevetran/epm-registry.git`
- Registry structure: `registry.json` (JSON index of all packages) + `packages/` (tarballs)
- EPM caches the registry clone in `~/.epm/.epm-registry/` and fetched manifests in `~/.epm/manifests/`
- Token stored in `~/.epm/token` with `chmod 600`
- Published tarballs exclude: `elysium_modules/`, `.git/`, `.epm/`, `target/`, `Cargo.lock`, `elysium.lock`
- Tree-shaking: walks dependency tree, follows `import` statements from each package's entry point,
  removes any `.ely` file not reachable via the import graph

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
