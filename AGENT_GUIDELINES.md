

### GitHub Pages Documentation Site (added May 2026)
- Full documentation site in `core/docs/` (28+ static HTML pages)
- **Structure**:
  - `docs/index.html` — Home page with hero, feature grid, hello world, Todo app example
  - `docs/guide/index.html` — Complete language guide (variables, functions, types, control flow, error handling, memory, concurrency, human-centric constructs)
  - `docs/ui/index.html` — Declarative UI layer guide (components, state, conditional rendering, two-way binding, styling, built-in views)
  - `docs/std/index.html` — Standard library reference (console, fs, transport, string 30+ methods including crypto, regex, datetime)
  - `docs/spec/index.html` — Spec-driven development (spec/describe, feat/it, expect, todo, question, bench/bm)
  - `docs/tooling/index.html` — CLI compiler, npm package `elysium-lang`, EPM package manager, linter, syntax highlighter, doc generator, environment filtering
  - `docs/recipes/index.html` — Recipe hub with 16 cards linking to individual recipe pages
  - 16 individual recipe pages covering: hello-world, functions-imports, console-logging, switch-case, filesystem, http-requests, string-crypto, datetime, regex, async-parallel, spec-testing, benchmarking, classes, stub-env, ui-counter, discount
- **Branding**: Official gold logo at `docs/assets/brand/elysium-logo.png` (navbar + home hero); `favicon.png` / `logo-icon.png` derived for tab icon
- **Styling**: Dark theme inspired by Vercel/modern docs, responsive, with syntax highlighting classes, copy-to-clipboard on code blocks, mobile navigation, intersection-observer animations
- **Site location**: `core/docs/` (source of truth in this monorepo)
- **Public URL**: `https://elysiumlang.github.io/` via docs-only mirror repo `elysiumlang/elysiumlang.github.io`
- **Mirror workflow**: `.github/workflows/docs-mirror.yml` pushes `core/docs` to `elysiumlang/elysiumlang.github.io` on `main` when docs change; needs repo secret `DOCS_DEPLOY_TOKEN` (PAT with `contents: write` on the Pages repo)
- **Manual mirror**: `./scripts/mirror-docs.sh` (set `DOCS_REPO` if using SSH)
- **Pages repo setup**: (1) Create org/user `elysiumlang` + empty public repo `elysiumlang.github.io`, (2) Settings → Pages → Deploy from branch `main` / `/ (root)`, (3) Add `DOCS_DEPLOY_TOKEN` to `imstevetran/elysium` secrets, (4) push doc changes to `main` or run workflow manually
- **This repo does not host Pages** — only mirrors docs outward; language repo stays `imstevetran/elysium`
- **Package registry page** (`docs/packages/index.html`): loads live from `https://raw.githubusercontent.com/imstevetran/epm-registry/main/registry.json`; `docs/data/packages-meta.json` only supplies icons/categories/tags for known packages (presentation layer, not the source of truth)
- **EPM registry packages** (May 2026): official packages under `@elysium/*` (`auth`, `ble`, `zigbee`, `langchain`, `langgraph` @ 0.1.0); sources in `packages/@elysium/`; imports `#/@elysium/<name>`; compiler aliases legacy `#/langchain` → `@elysium/langchain`
- **EPM auth model** (May 2026): `epm login` → `gh` browser/device flow; `epm publish`/`org create`/`grant` require GitHub user; package `owner` is only account that can `epm grant`; org `owner` is only account that can create packages under `@org/*`
- **README updated**: Now points to documentation site, includes full project structure tree

### VS Code Extension (added May 2026)
- Extension created at `vscode-elysium/` — provides syntax highlighting, snippets, and language config for `.ely` and `.elyx` files
- **TextMate grammar** (`syntaxes/elysium.tmLanguage.json`): 15 pattern repositories covering all 57 keywords, type names, builtins, comments (///, //, /* */), strings with escapes, numbers, operators/arrows, XML tags (.elyx), import statements, function/class/component/enum/typealias definitions with entity-name scoping, and a variable catch-all
- **Keywords**: Scoped as `keyword.control`, `keyword.declaration`, `keyword.other`, `constant.language`
- **Operators**: Arrow `->`/`<-` before individual chars to prevent split matching
- **XML tags**: Handle attribute expressions via `meta.expression` inside `{...}` curly braces
- **Language config**: Bracket pairs, auto-closing, indentation rules, onEnter rules
- **21 snippets** for common constructs (func, let, var, for, if, if-else, while, match, try-catch, class, component, enum, typealias, spec, describe, import, async, parallel, print, bench)
- **Packaged**: Published to Cursor as `imstevetran.elysium-lang` from VSIX at `vscode-elysium/elysium-lang-0.1.0.vsix`
- **Verified**: Tokenization tested against all example files via vscode-textmate; `->` correctly scoped as arrow, `result` not mistaken for type, XML expression braces properly handled
- **To rebuild**: `cd vscode-elysium && npx @vscode/vsce package && code --install-extension elysium-lang-0.1.0.vsix --force`

### GitHub Actions CI/CD (added May 2026)
- Two workflows created in `.github/workflows/`:
  - **`ci.yml`**: Runs on push/PR to `main` across ubuntu/macos/windows. Installs LLVM 18 (required by inkwell), builds Rust workspace with `--release`, runs `cargo test`, and validates npm package structure via `npm pack --dry-run`.
  - **Ubuntu LLVM deps**: `llvm-18-dev` alone is not enough — `llvm-config --libnames --link-static` lists `libPolly.a` but Ubuntu splits it into `libpolly-18-dev`. Without it, `llvm-sys` fails with `could not find native static library Polly`.
  - **Windows LLVM**: `windows-latest` images preinstall LLVM 20 via Chocolatey; `choco install llvm --version=18.1.0` fails without uninstalling first (`choco uninstall llvm -y` then install 18.1.0).
  - **`release.yml`**: Triggered on `v*` tags. Four jobs:
    1. **build-binaries**: Matrix across ubuntu (linux x64), macos (arm64), windows (x64). Installs LLVM 18, runs `node scripts/build-binaries.js`, uploads gzipped binaries as artifacts.
    2. **create-release**: Downloads all binary artifacts and creates a GitHub Release with `softprops/action-gh-release`, attaching all `.gz` files and generating release notes.
    3. **publish-npm**: Uses npm Trusted Publishing (OIDC) with `id-token: write` permission. No `NPM_TOKEN` needed -- npm CLI >= 11.5.1 auto-detects OIDC. Publishes from `npm-package/` with automatic provenance attestation.
    4. **docs mirror** (separate workflow `docs-mirror.yml`): pushes `core/docs` to `elysiumlang/elysiumlang.github.io` when docs change on `main`
- **User setup needed**:
  1. Docs mirror: create `elysiumlang/elysiumlang.github.io`, enable Pages from `main`, add `DOCS_DEPLOY_TOKEN` secret on this repo
  2. npm Trusted Publisher: npmjs.com/package/elysium-lang/settings -> Trusted Publisher -> GitHub Actions: `imstevetran/elysium`, workflow `release.yml`, allow `npm publish`
  3. (Optional) Package settings -> Publishing access -> "Require two-factor authentication and disallow tokens"

### Release process (added May 2026)
- Use `./scripts/release.sh <version>` to cut a release. Example: `./scripts/release.sh 0.2.0`
- The script bumps the version in `Cargo.toml`, `npm-package/package.json`, and `vscode-elysium/package.json`, commits, and tags.
- After running the script, push both the commit and the tag:
  ```
  git push origin main
  git push origin v0.2.0
  ```
- The CI release workflow (`.github/workflows/release.yml`) then automatically:
  1. Builds native binaries for Linux x64, macOS ARM64, and Windows x64
  2. Creates a GitHub Release with those binaries attached + auto-generated release notes
  3. Publishes `elysium-lang@<version>` to npm via Trusted Publishing (OIDC) with provenance attestation
  4. (Docs deploy separately via `docs-mirror.yml` when `core/docs` changes on `main`)
- The release workflow triggers on tags matching `v*`. The tag must be an annotated/lightweight tag pushed to the remote.
- Version must follow semver (`<major>.<minor>.<patch>`). The script validates this.
- The script requires a clean working tree (no uncommitted changes).

### `core/src` layout (reorganized May 2026)
- **`lib.rs` + `main.rs`**: library root and thin CLI entry (`main.rs` ~300 lines)
- **`frontend/`**: `ast`, `lexer`, `parser`
- **`middle/`**: `hir`, `ownership`, `type_checker`
- **`backend/`**: `mir`, `codegen`, `codegen_tools`
- **`driver/`**: `cli`, `compile`, `imports`, `desugar`, `stubs`, `elyx_cmd`, `commands`, `source`
- **`epm/`**: package manager (`init`, `install`, `publish`, `update`, `manifest`, `module`, `port`, `migrate`, `extension`)
- **`ui/`**: `elyx` (.elyx parser)
- **`tools/`**: `highlighter`, `linter`, `debug`, `test_runner`
- **`error.rs`**: shared at crate root; `crate::ast` etc. re-exported from `lib.rs` for stable paths

### BLE (Bluetooth Low Energy) Package (added May 2026)
- BLE operations via `ble.` method-call syntax: scanning, connect/disconnect, read/write characteristics, RSSI
- Desugaring in `main.rs` (`desugar_builtin_calls`) converts `ble.method(...)` → `__ble_method(...)` (same pattern as fs/transport/regex/datetime)
- Type checker registers `__ble_*` builtins:
  - **Void** `→ Nil`: `scan`, `stopScan`, `disconnect(addr)`, `writeCharacteristic(addr, uuid, value)`
  - **String** `→ String`: `connect(addr)`, `readCharacteristic(addr, uuid)`, `readRssi(addr)`, `deviceName(addr)`
  - **Bool** `→ Bool`: `isConnected(addr)`, `isScanning`
- MIR: Uses `MirStmt::BleCall { result, method, args, dbg_line }` for both void and return-value contexts
- C backend codegen (`emit_ble_call`): emits `printf` stubs `[ble] method: ...` — real BLE requires platform-native frameworks (CoreBluetooth on macOS, BlueZ on Linux) that are impractical to embed in the compiled C binary
- JS runtime `npm-package/runtime/ble.js` provides full implementation:
  - **Browser**: Uses Web Bluetooth API (`navigator.bluetooth.requestDevice`, GATT service/characteristic access)
  - **Node.js**: Uses `@abandonware/noble` package for BLE scanning, connection, and characteristic operations
  - Falls back to stub logging if no BLE backend is available
  - Maintains internal device registry (`Map<address, { peripheral, name, rssi }>`)
- Exported via `require('elysium-lang').ble`
- Example: `examples/ble.ely` demonstrates scanning, connecting, reading/writing characteristics, RSSI, and disconnecting
- Type-checking verified: `cargo run -- check examples/ble.ely` passes

### LangChain Package (LLM, Chat, RAG, Agents, AI) — Elysium Source Package (Reworked May 2026)
- Spec-driven tests at `tests/test_langchain.ely` — uses `spec "LangChain" { feat "..." { expect <expr> } }` to verify all 14 langchain functions type-check correctly
- LangChain-style AI operations implemented as a pure Elysium source file at `packages/@elysium/langchain/langchain.ely`
- Uses Elysium's `import "#/@elysium/langchain" as langchain` syntax — no compiler-level builtin registration needed
- **Published to EPM registry** as `langchain@0.1.0` — installable via `epm install @elysium/langchain`
- Uses Elysium builtins (dict, json, math, http, env) instead of transport.post() URL hacks
- **Ollama/local model support**: No API key required. Set `OPENAI_BASE_URL` to your Ollama endpoint (default: `http://localhost:11434/v1`). Set `LLM_MODEL` for the default model (default: `qwen3.5:2b`).
- Environment variables: `OPENAI_BASE_URL`, `OPENAI_API_KEY` (optional for local), `LLM_MODEL`
- The `http.request()` response wrapper is parsed via `json.parse → json.get("body") → json.parse → json.get("choices.0.message.content")`
- API surface: `llm`, `chat`, `embed`, `similarity`, `template`, `rag`, `summarize`, `analyze`, `classify`, `translate`, `agent`, `agentStream`, `chain`, `extract`
- Example: `examples/langchain.ely` imports the package and exercises all operations
- Verified via `cargo run -- check examples/langchain.ely`, `cargo run -- check packages/@elysium/langchain/langchain.ely`

### Zigbee (Home Automation) Package (added May 2026)
- Zigbee home automation operations via `zigbee.` method-call syntax: network management, device control, attribute read/write, group management, binding
- Desugaring in `main.rs` (`desugar_builtin_calls`) converts `zigbee.method(...)` → `__zigbee_method(...)` (same pattern as fs/transport/regex/ble)
- Type checker registers `__zigbee_*` builtins:
  - **Void `→ Nil`**: `start`, `shutdown`, `permitJoin(seconds)`, `scan`, `on(dev, ep, cluster)`, `off(dev, ep, cluster)`, `toggle(dev, ep, cluster)`, `writeAttribute(dev, ep, cluster, attr, value)`, `addToGroup(dev, groupId)`, `removeFromGroup(dev, groupId)`, `bind(srcDev, srcEp, dstDev, dstEp)`
  - **String `→ String`**: `readAttribute(dev, ep, cluster, attr)`, `getDeviceName(dev)`, `getManufacturer(dev)`
  - **Int `→ Int`**: `getDeviceCount`, `getPanId`, `getChannel`
  - **Bool `→ Bool`**: `isJoined`, `isOnline(dev)`, `isPermittingJoin`
- MIR: Uses `MirStmt::ZigbeeCall { result, method, args, dbg_line }` for both void and return-value contexts
- C backend codegen (`emit_zigbee_call`): emits `printf` stubs `[zigbee] method: ...` — real Zigbee requires platform-specific serial/USB dongle integration (TI CC2531/CC2652, Conbee II, Elelabs)
- JS runtime `npm-package/runtime/zigbee.js` provides full implementation:
  - **Node.js**: Optionally uses `zigbee-herdsman` for real coordinator communication via USB dongle
  - Maintains in-memory device registry with state tracking
  - All device control functions (on/off/toggle) work via Zigbee On/Off cluster commands
  - Group management and binding operations supported
  - Falls back to stub logging if no Zigbee backend is available
- Exported via `require('elysium-lang').zigbee`
- Example: `examples/zigbee.ely` demonstrates start, permitJoin, device control, attribute reads, group management, and shutdown
- Type-checking verified via `cargo run -- check examples/zigbee.ely`

### LangGraph Package (Stateful Graph-Based Agent Orchestration) — Elysium Source Package (Reworked May 2026)
- Spec-driven tests at `tests/test_langgraph.ely` — uses `spec "LangGraph" { feat "..." { expect <expr> } }` to verify all 12 langgraph functions type-check correctly
- LangGraph-style stateful agent orchestration implemented as a pure Elysium source file at `packages/@elysium/langgraph/langgraph.ely`
- Uses Elysium's `import "#/@elysium/langgraph" as langgraph` syntax — no compiler-level builtin registration needed
- **Published to EPM registry** as `langgraph@0.1.0` — installable via `epm install @elysium/langgraph`
- All functions delegate to the JS runtime via `transport.post("__langgraph__/<method>", body)`:
  - The `__langgraph__/` URL prefix is intercepted by `npm-package/runtime/transport.js` and routed to `npm-package/runtime/langgraph.js`
  - Multi-argument functions encode parameters with `|||` delimiter
- API surface: `graph`, `addNode`, `addEdge`, `addConditionalEdges`, `compile`, `invoke`, `stream`, `getState`, `updateState`, `branch`, `interrupt`, `resume`
- JS runtime provides full implementation: state graphs, conditional routing, LLM nodes, streaming, interrupt/resume, parallel branching
- Falls back to mock responses if no API key is configured
- Example: `examples/langgraph.ely` imports the package and exercises all operations
- Verified via `cargo run -- check examples/langgraph.ely`

### `elysium test` Command (added May 2026)
- New CLI subcommand `elysium test [path]` — runs spec-driven tests
- Usage:
  - `elysium test` — scans `core/examples/` directory for `.ely` files with `spec` blocks
  - `elysium test core/examples/spec_simple.ely` — runs a single file
  - `elysium test core/examples/ --dry-run` — lists specs and feats without type-checking
  - `elysium test --env test` — use the "test" environment for stub filtering (default: "test")
- **Design**: Elysium's `spec`/`feat`/`expect` are compile-time constructs. `expect <expr>` validates that `<expr>` is well-typed. There is no runtime execution — if type-checking passes, all specs pass.
- **Implementation** (3 files): `src/cli.rs` — `Test` command with `path`, `--dry-run`, `--env` options; `src/test_runner.rs` — `run_tests_in_file()` and `list_tests()` functions; `src/main.rs` — `cmd_test()` dispatch with import resolution, stub filtering, and summary output
- Output format: per-file header, spec/tests count, checkmarks for each passing spec/feat, summary line
- When type-checking fails, the error is printed and the file is marked as failed
- Exit code is non-zero if any file has failures
- Currently has spec examples in `core/examples/spec_simple.ely`, `core/examples/spec_example.ely`

### Schedule (Cron) Keyword for Background Functions (added May 2026)
- New `schedule "expr" func name() { ... }` syntax for running functions on a timer in the background
- **Supports two formats**:
  - **Cron-style**: `*/5 * * * *`, `0 8 * * *`, `0 */2 * * *` (standard 5-field cron)
  - **Friendly formats**: `every 5 minutes`, `every hour`, `hourly`, `daily at 08:00`, `at 08:00 every day`, `every Monday at 09:00`, `every month on day 15 at 10:00`, `every 2 hours`, `every minute`, `minutely`, `daily`, `weekly`, `monthly`, `every Monday`
- **Implementation**:
  - `parse_schedule()` in `src/codegen.rs`: Detects cron vs friendly (5 fields without alphabetic chars = cron), dispatches accordingly
  - `parse_friendly()` handles: `every N <unit>`, `every <unit>`, `hourly`/`daily`/`weekly`/`monthly` aliases, `at HH:MM` extraction from anywhere in the string, day-of-week detection (e.g. `every Monday`)
  - Returns `ScheduleKind` enum: `Interval(u32)`, `DailyAt { hour, min }`, `WeeklyAt { dow, hour, min }`, `MonthlyAt { dom, hour, min }`
- **Runtime behavior**:
  - `Interval` schedules: compile-time constant `sleep(N)` in infinite loop (no `time()` needed)
  - Time-of-day schedules (`DailyAt`, `WeeklyAt`): runtime `time()` + arithmetic computes seconds-until-next-occurrence as the initial sleep, then `sleep(base_interval)` for subsequent iterations. Uses SSA-correct `select` instruction (no phi nodes needed)
- **Pipeline changes** (8 files):
  - `src/lexer.rs`: Added `Schedule` keyword token
  - `src/ast.rs`: Added `schedule_expr: Option<String>` to `Function`
  - `src/parser.rs`: Parse `schedule "cron" func ...` and `schedule "cron" async func ...`; added `parse_scheduled_func_def()` with `schedule_expr` param
  - `src/hir.rs`: Propagated `schedule_expr` through `HirFunction`
  - `src/mir.rs`: Propagated `schedule_expr` through `MirFunction`
  - `src/codegen.rs`: ~180 lines of schedule-parsing logic (cron + friendly), `ScheduleKind` enum, `parse_schedule()`, `parse_cron()`, `parse_friendly()`, `parse_every_n()`, `parse_every_unit()`, `schedule_base_interval()` free functions; thread wrapper emits interval-sleep or time-of-day wrapper; runtime `time()` call for initial offset with `select` for SSA; `call_scheduled_func()` helper; `_entry_block` unused var suppressed
  - `src/highlighter.rs`: Added `Schedule` to keyword rendering
  - `src/mir.rs`: Test `lower_hir_stmts` updated with `schedule_expr: None`
- Example: `examples/schedule.ely` with 5 scheduled functions using different formats
- Verified: `cargo test` — all 116 tests pass; `cargo run -- check examples/schedule.ely` passes; `cargo run -- build examples/schedule.ely` produces valid LLVM IR with `time()`, `select`, `sleep`, and `pthread_create`/`pthread_detach`

### Wait Keyword (added May 2026)
- New `wait <millis>` statement — pauses execution for N milliseconds
- Syntax: `wait 1000` (waits 1 second), `wait 500` (waits 500ms)
- **Implementation** (11 files):
  - `src/lexer.rs`: Added `Wait` token
  - `src/ast.rs`: Added `Wait` struct with `millis: u64` field and `Stmt::Wait(Box<Node<Wait>>)` variant
  - `src/parser.rs`: Added `parse_wait_stmt()` that consumes `wait <int>`; `Token::Wait` in `peek_is` and `token_eq`; dispatch in `parse_stmt`
  - `src/hir.rs`: Added `HirStmt::Wait(u64, u32)` variant and lowering from AST
  - `src/mir.rs`: Added `MirStmt::Wait(u64, u32)` variant; `stmt_line` and `lower_stmt` lowering from HIR
  - `src/codegen.rs`: Emits `usleep(millis * 1000)` LLVM call; `emit_wait_stmt` helper; `MirStmt::Wait` in `emit_stmt` dispatch and line extraction
  - `src/type_checker.rs`: `Stmt::Wait(_) => Type::Nil`
  - `src/ownership.rs`: `Stmt::Wait(_) => Ok(())`
  - `src/linter.rs`: `Stmt::Wait(_) => {}`
  - `src/highlighter.rs`: `Token::Wait => SpanKind::Keyword`
  - `src/main.rs`: `Stmt::Wait(_)` no-ops in both desugar functions
  - `src/codegen_tools.rs`: `Stmt::Wait(_)` no-op in `collect_calls_in_stmt`
- Codegen emits `declare i32 @usleep(i32)` and calls `usleep(millis * 1000)` (microseconds)
- Example: `examples/wait.ely` with `wait 1000` and `wait 500`
- Verified: `cargo test` — all 116 tests pass; `cargo run -- check examples/wait.ely` passes; IR shows `call i32 @usleep(i32 <micros>)`

### Auth Package (Session, JWT, Passkey, OAuth2, Authorization, Multi-tenant) (added May 2026)
- Comprehensive auth operations via `auth.` method-call syntax: JWT sign/verify/decode, session CRUD, password hashing/verification, permission/role/scope checks, OAuth2 authorization code flow, passkey (WebAuthn) registration/authentication, API key generation/validation, RBAC access control, and multi-tenant management
- Desugaring in `main.rs` (`desugar_builtin_calls`) converts `auth.method(...)` → `__auth_method(...)` (same pattern as langchain/ble/zigbee)
- Type checker registers `__auth_*` builtins:
  - **String `→ String`**: `jwtSign(payload, expiresIn)`, `jwtVerify(token)`, `jwtDecode(token)`, `createSession(userId, data)`, `getSession(sessionId)`, `hashPassword(password)`, `verifyPassword(password, hash)`, `checkPermission(user, perm)`, `hasRole(user, role)`, `hasScope(token, scope)`, `oauth2Authorize(clientId, redirectUri, scope)`, `oauth2Token(code, clientId, clientSecret)`, `oauth2Refresh(refreshToken, clientId)`, `passkeyRegister(userId, userName)`, `passkeyAuthenticate(userId)`, `tenantContext(tenantId)`, `getTenant()`, `listTenants()`, `createTenant(tenantId, config)`, `grantRole(user, role)`, `grantPermission(user, perm)`, `revokeRole(user, role)`, `revokePermission(user, perm)`, `generateApiKey(userId)`, `validateApiKey(apiKey)`, `checkAccess(userId, resource, action)`, `setRoles(userId, rolesJson)`, `setPermissions(userId, permsJson)`
  - **String `→ Nil`**: `destroySession(sessionId)`
- MIR: Uses `MirStmt::AuthCall { result, method, args, dbg_line }` for both void and return-value contexts
- C backend codegen (`emit_auth_call`): emits `printf` stubs `[auth] method: use JS runtime` — real auth operations require the Node.js runtime
- JS runtime `npm-package/runtime/auth.js` provides full implementation:
  - **JWT**: HMAC-SHA256 signing/verification with custom claims, expiration (`exp`), issuer (`iss`), not-before (`nbf`)
  - **Session**: In-memory session map with TTL-based expiry, create/get/destroy
  - **Password**: Salted PBKDF2-SHA256 hashing with salt:hash format
  - **Authorization**: Role/permission checks on in-memory user-role and user-permission maps
  - **OAuth2**: Authorization code flow with client registration, code generation (10-min TTL), token exchange, refresh token rotation
  - **Passkey**: WebAuthn-style registration and authentication option generation with challenge/credential management
  - **API Key**: `ely_`-prefixed key generation with SHA-256 lookup
  - **RBAC**: Role-based access control with role-permission matrix (admin/editor/viewer) and direct permission matching
  - **Multi-tenant**: Tenant CRUD, tenant context isolation, default tenant initialization
  - Configurable via environment variables: `AUTH_JWT_SECRET`, `AUTH_JWT_ISSUER`, `AUTH_SESSION_TTL_MS`, `AUTH_TENANT_ID`
- Exported via `require('elysium-lang').auth`
- Example: `examples/auth.ely` demonstrates JWT, sessions, passwords, permissions, RBAC, OAuth2, passkey, API keys, and multi-tenant operations
- Type-checking verified via `cargo run -- check examples/auth.ely`

### WASM Package (WebAssembly) (added May 2026)
- WebAssembly operations via `wasm.` method-call syntax: compile, instantiate, call, memory read/write, exports listing, destroy, reset
- Desugaring in `main.rs` (`desugar_builtin_calls`) converts `wasm.method(...)` → `__wasm_method(...)` (same pattern as fs/transport/langchain)
- Type checker registers `__wasm_*` builtins — all return String:
  - `compile(source)`, `instantiate(moduleId, importsJson)`, `call(instanceId, name, argsJson)`
  - `memory(instanceId)`, `writeMemory(instanceId, offset, data)`, `readMemory(instanceId, offset, length)`
  - `exports(instanceId)`, `destroy(instanceId)`, `reset()`
- MIR: Uses `MirStmt::WasmCall { result, method, args, dbg_line }` for both void and return-value contexts
- C backend codegen (`emit_wasm_call`): emits `printf` stubs `[wasm] method: use JS runtime` — real WASM requires WebAssembly runtime
- JS runtime `npm-package/runtime/wasm.js` provides full implementation:
  - Compiles WAT or raw wasm bytes, instantiates with optional imports
  - Calls exported functions with JSON-encoded args, reads/writes memory via base64
  - Lists exports, destroys instances, resets all state
  - Falls back to mock if WebAssembly not available
- Exported via `require('elysium-lang').wasm`
- Example: `examples/wasm.ely` demonstrates compile, instantiate, call, exports, destroy, reset
- Type-checking verified via `cargo run -- check examples/wasm.ely`

### WebWorker Package (added May 2026)
- WebWorker operations via `webworker.` method-call syntax: create, postMessage, onMessage, waitMessage, terminate, pool management
- Desugaring in `main.rs` converts `webworker.method(...)` → `__webworker_method(...)` (same pattern)
- Type checker registers `__webworker_*` builtins — all return String:
  - `create(scriptOrCode)`, `postMessage(workerId, message)`, `onMessage(workerId)`, `waitMessage(workerId)`
  - `terminate(workerId)`, `isRunning(workerId)`, `activeCount()`, `terminateAll()`
- MIR: Uses `MirStmt::WebWorkerCall { result, method, args, dbg_line }`
- C backend codegen (`emit_webworker_call`): emits `printf` stubs `[webworker] method: use JS runtime`
- JS runtime `npm-package/runtime/webworker.js` provides full implementation:
  - Creates real Workers in browser/Node.js (worker_threads) with inline code or script URL
  - Falls back to mock worker context when native workers unavailable
  - Maintains per-worker message queues for non-blocking reads
  - Supports waitMessage with 5s timeout polling, terminateAll for pool management
- Exported via `require('elysium-lang').webworker`
- Example: `examples/webworker.ely` demonstrates create, postMessage, onMessage, isRunning, activeCount, terminate, terminateAll
- Type-checking verified via `cargo run -- check examples/webworker.ely`

### `elysium update` and `elysium migrate` Commands (added May 2026)
- Two new CLI subcommands for dependency management and source-level migration.

**`elysium update [package]`** — Checks the EPM registry for newer versions of dependencies:
  - `package` — optional target package; omit to check all dependencies
  - `--apply` / `-a` — writes updated version constraints to `elysium.json`
  - `--latest` — upgrades to absolute latest version (ignores constraint range)
  - `--force` — allows downgrade if latest is lower than current
  - Reads `elysium.json` from the project root, queries the local EPM registry cache (`~/.epm/.epm-registry/`), compares constraints against available versions using semver (caret `^` resolution by default), and reports which packages can be updated
  - After `--apply`, run `epm install` to update the lockfile and installed packages

**`elysium migrate [path]`** — Applies automatic source-level transformations to `.ely` files:
  - `file` — optional path to `.ely` file or directory (defaults to recursive cwd scan, skips `target`, `.git`, `node_modules`, `elysium_modules`, `.epm`)
  - `--check` — exit non-zero if any file needs migration (CI mode)
  - `--dry-run` — show what would change without writing
  - `--force` — apply migrations marked as "requires manual review"
  - **Registered migrations**:
    1. `webworker-to-worker` — Renames `webworker.*` calls to `worker.*` (webworker API merged into worker). Automatic.
    2. `bm-to-bench` — Replaces `bm` keyword with `bench` for consistency. Automatic.
    3. `normalize-imports` — Prepends `./` to relative import paths missing a prefix. Automatic.
    4. `describe-to-spec` — Replaces `describe`→`spec` and `it("...")`→`feat("...")`. **Requires manual review** (enabled only with `--force`).
  - Mutations are applied sequentially and idempotently (running twice produces the same result)
  - All changes are reported with per-file, per-change granularity

**Implementation**: `src/cli.rs` — `Update` and `Migrate` command options; `src/update.rs` — registry query, semver comparison, manifest writing; `src/migrate.rs` — migration registry, apply functions, recursive file collection


## Elysium Language Skill (added May 30, 2026)
- Created project skill at `.cursor/skills/elysium-lang/` to teach agents how to read, write, compile, and understand Elysium 2.0 end-to-end
- **3 files**:
  - `SKILL.md` (230 lines) — Main skill with essential reference, CLI usage, all built-in packages (fs, transport, string, regex, datetime, ble, zigbee, auth, wasm, worker), langchain/langgraph source packages, UI components, schedule, stubs, spec/testing, error handling, conventions, and the 7-layer builtin package creation pattern
  - `reference.md` (309 lines) — Full syntax reference covering variables, functions, control flow, types, enums, classes, error handling, memory, concurrency, human-centric constructs (bc, only, ellipsis), doc comments, imports, UI components, specs, schedule, wait, stubs, complete keyword list (57 tokens), operators, built-ins, type names, and environments
  - `examples.md` (401 lines) — 21 practical code examples: hello world, discount with bc/only, switch/case, filesystem, HTTP transport, string crypto, datetime, regex, imports, async/parallel, specs, classes, UI components, LangChain, LangGraph, Auth, schedule/wait, workers, WebAssembly, Result error handling, stubs, plus CLI commands reference
- **Always consult**: CONTRIBUTION_GUIDELINES.md for conventions, and the skill files for language reference

### Multi-line / Backtick Strings (added May 2026)
- Backtick string support added to the lexer: `` `[^`]*` `` — strings delimited by backticks that CAN contain double quotes `"` but NOT backticks
- Useful for inline JSON, HTML snippets, or any string that needs embedded `"` characters
- **Lexer**: `Token::BacktickString(String)` variant added to `src/lexer.rs`
- **Parser**: `is_string_lit()` and `expect_string()` both match `BacktickString` — it's treated identically to `StringLiteral` throughout the pipeline
- **Highlighter**: `BacktickString` → `SpanKind::String` in `src/highlighter.rs`
- Syntax: `` let json = `{"key": "value"}` `` — no escaping needed for double quotes
- Works with multi-line content: `` let multi = `{
  "name": "Elysium"
}` ``

### Inline JSON / Record Object Literals (added May 2026)
- Elysium now supports inline JSON object literal syntax: `{ "key": value, ... }`
- **Parser**: `parse_block_expr()` detects the pattern (string-literal followed by colon after `{`) and delegates to `parse_record_literal()` which produces `Expr::Record(fields)`
- **Desugaring**: `desugar_builtin_in_expr()` transforms `Expr::Record` into `__json_buildObject("k1", v1, "k2", v2)` calls — this happens before type-checking, so the type checker only sees a normal function call
- **Type checker**: `__json_buildObject` registered with 20 params (10 pairs of `String, String`) as a varargs proxy
- **JS runtime**: `json.buildObject()` in `npm-package/runtime/json.js` already handles nested JSON strings
- Example: `let body = {"model": "gpt-4o", "temperature": "0.7"}`
- Backtick strings and Record literals can be combined for cleaner code

### Dict, JSON, Math, Env, HTTP Builtins (Reworked May 2026)
- Five new builtin packages added: `dict`, `json`, `math`, `env`, `http`
- **JS runtime modules** (all in `npm-package/runtime/`):
  - `dict.js` — mutable key-value string dictionaries with `create/set/get/has/delete/keys/length/clear`
  - `json.js` — JSON parsing (`parse`, `parseInline`, `get`, `stringify`, `free`), plus `buildObject`, `buildMessage`, `buildArray` for programmatic JSON construction
  - `math.js` — scalar (`sqrt`, `pow`, `abs`, `floor`, `ceil`, `round`, `sin`, `cos`, `tan`, `log`, `log2`, `log10`, `exp`, `max`, `min`) and vector (`sum`, `mean`, `dot`, `cosineSimilarity`, `euclidean`) math
  - `env.js` — `get(key)`, `set(key, value)` wrapping `process.env`
  - `http.js` — `request(method, url, headers, body)` async fetch-based, `requestSync` stub
- **Compiler changes**: Desugaring prefixes (`__dict_`, `__json_`, `__math_`, `__env_`, `__http_`), MIR variants (`DictCall`, `JsonCall`, `MathCall`, `EnvCall`, `HttpCall`), MIR lowering, type checker registration, codegen stubs
- All packages are automatically available via module name (no import needed)

### BLE and Zigbee Pure Elysium Packages (added May 2026)
- BLE and Zigbee operations now available as importable Elysium source packages
- **`packages/@elysium/ble/ble.ely`**: Wraps `__ble_*` builtins with clean API — `scan()`, `stopScan()`, `connect(address)`, `disconnect(deviceId)`, `readCharacteristic(devId, serviceUuid, charUuid)`, `writeCharacteristic(devId, serviceUuid, charUuid, value)`, `readRssi(deviceId)`, `deviceName(deviceId)`, `isConnected(deviceId)`, `isScanning()`
- **`packages/@elysium/zigbee/zigbee.ely`**: Wraps `__zigbee_*` builtins — `start()`, `shutdown()`, `permitJoin(seconds)`, `scan()`, `on(devId, ep, cluster)`, `off(devId, ep, cluster)`, `toggle(devId, ep, cluster)`, `readAttribute(devId, ep, cluster, attr)`, `writeAttribute(devId, ep, cluster, attr, value)`, `addToGroup(devId, groupId)`, `removeFromGroup(devId, groupId)`, `bind(srcDev, srcEp, dstDev, dstEp)`, `getDeviceName(devId)`, `getManufacturer(devId)`, `getDeviceCount()`, `getPanId()`, `getChannel()`, `isJoined()`, `isOnline(devId)`, `isPermittingJoin()`
- Import via: `import "#/@elysium/ble" as ble` or `import "#/@elysium/zigbee" as zigbee`
- Type-checking verified via `cargo run -- check packages/@elysium/ble/ble.ely` and `cargo run -- check packages/@elysium/zigbee/zigbee.ely`
- Examples: `examples/ble.ely`, `examples/zigbee.ely`

### `is` Operator — Runtime Type Checking (added May 30, 2026)
- New `is` keyword for runtime type checking: `expr is TypeName` returns `Bool`
- **Lexer**: `Token::Is` keyword token added to `src/lexer.rs`
- **AST**: `Expr::Is { value: Box<Node<Expr>>, type_name: String }` variant in `src/ast.rs`
- **Parser**: Parsed in `parse_cmp_expr()` — after comparison operators (`==`, `!=`, `<`, `>`, `<=`, `>=`) but before falling through. Expects `expr is Identifier` where Identifier is the type/class name
- **Desugaring**: `desugar_builtin_in_expr()` transforms `Expr::Is` → `__is_instanceof(expr, "TypeName")` call before type checking
- **Type checker**: `__is_instanceof(value, typeName)` registered as `(Infer, String) → Bool` in `self.functions`
- **MIR**: `MirStmt::IsCall { result, value, type_name, dbg_line }` variant — hands the value and type name runtime arguments to the backend
- **Codegen**: `emit_is_call()` prints `[is] instanceof: use JS runtime` stub for C/LLVM backend
- **JS runtime** (`npm-package/runtime/is.js`): `__is_instanceof(value, typeName)` walks the prototype chain using `constructor.name` to find a match. Exported as `__is_instanceof` on the runtime index.
- **Highlighter**: `Token::Is` → `SpanKind::Keyword`
- **Parser fixes**: `peek_is()` and `token_eq()` updated with `(Token::Is, Token::Is) => true` arm
- Syntax: `let isDog = animal is Dog` — returns `true` if `animal` is an instance of class `Dog` (or a subclass)
- Example: `examples/is_operator.ely`
- Verified: `cargo run -- check examples/is_operator.ely` passes; all 116 unit tests pass

## BLE & Zigbee: full Elysium packages (not compiler builtins)
- `ble.ely` and `zigbee.ely` are pure Elysium packages in `packages/`, imported via `import "#/@elysium/ble" as ble`
- They use `transport.post("__ble__/<method>", encodedArgs)` to delegate to JS runtime
- Compiler-level: no `__ble_*` / `__zigbee_*` prefixes; no special `MirStmt::BleCall`/`MirStmt::ZigbeeCall`; no codegen for these
- `transport.js` routes `__ble__/*` and `__zigbee__/*` URLs to `ble.js` and `zigbee.js` runtime modules (lazy-loaded)
- These packages **survive both UI and backend build** because they only use `transport.post()` — which is a compiler-level builtin
- All BLE/Zigbee builtins were removed from type_checker.rs, mir.rs, codegen.rs, and main.rs (desugaring prefix)
- Verified: `cargo run -- check examples/ble.ely` and `examples/zigbee.ely` pass; all 116 unit tests pass

## Test embedding pattern
- Tests for packages live **inside the package file itself** using `spec { ... }` blocks, not in `tests/`
- This keeps the package self-contained: `ely test packages/@elysium/langchain/langchain.ely` runs the specs
- Functions inside the package are called directly (no import alias needed): `llm(...)` not `langchain.llm(...)`
- Separate `tests/test_*.ely` files are for project-level integration tests (importing packages via `#/`)
- Verified: `ely test packages/@elysium/langchain/langchain.ely` passes 14 tests, `ely test packages/@elysium/langgraph/langgraph.ely` passes 12 tests

## WASM is a compilation target, not a runtime API
- WASM (`__wasm_*` builtins, `wasm.js`, `wasm.ely`) has been **removed entirely** from compiler and runtime
- WASM is now a **compilation target** for browser UI builds, configured in `elysium.json`:
  ```json
  { "ui": { "browser": { "target": "wasm" } } }
  ```
- No `wasm.*` runtime calls in source code — the compiler handles WASM output at the build level
- Future targets: `android`, `ios` for native mobile UI
- All `__wasm_*` references removed from: main.rs, type_checker.rs, mir.rs, codegen.rs, npm-package/runtime/

## VSCode extension (vscode-elysium/)
- **Syntax file**: `syntaxes/elysium.tmLanguage.json` — TextMate grammar for `.ely`/`.elyx`
- **Language config**: `language-configuration.json` — auto-closing pairs, brackets, indentation
- **Snippets**: `snippets/elysium.json` — code completion snippets
- When adding new syntax features, update all three files in sync with the compiler's lexer
- New keywords (`wait`, `schedule`, `worker`, `is`) must be added to keyword categories in `.tmLanguage.json`
- New string syntax (backtick strings `` ` ``) needs a string pattern in `.tmLanguage.json` and auto-closing pairs in `language-configuration.json`
- Verified: all 3 JSON files validate; all 116 tests pass

## Shared manifest.rs with ui/ssr config
- Created `src/manifest.rs` with a shared `Manifest` struct used by both `main.rs` and `update.rs`
- Fields:
  - `ui.browser.target`: `"js"` (default) or `"wasm"` — compilation target for browser UI
  - `ssr.enabled`: `bool` — enable server-side rendering when code targets a server runtime
  - `ssr.runtime`: optional string — `"node"`, `"deno"`, `"bun"`, `"elysium-server"`
- `find_project_root()` and `resolve_env_alias()` moved from `main.rs`/`update.rs` into `manifest.rs`
- `load_manifest()` utility loads and parses `elysium.json` from project root
- JSON serde is `#[serde(default)]` everywhere so existing projects without `ui`/`ssr` fields continue to work
- Example `elysium.json` updated with both `ui` and `ssr` sections
- Documented in SKILL.md (Project Configuration section) and examples.md (example 19)

## All builtins emit LLVM `printf` stubs (removed May 31, 2026)
- The original `core/src/runtime/runtime.c` was deleted — it was entirely dead code.
- **Every** builtin (env, json, http, math, dict, auth, worker, is) emits `printf("[xxx] method: use JS runtime\n")` in LLVM IR codegen.
- Only standard C library functions (`time`, `fopen`, `printf`, `curl` via popen, etc.) are called directly from LLVM IR — no `__ely_*` wrappers.
- Native C runtime code only lives in extension directories (`extensions/*/runtime/*.c`) where `extension` keyword declares platform-specific runtimes.
