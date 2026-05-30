---
name: elysium-lang
description: Teach the agent how to read, write, compile, and understand Elysium 2.0 — a human-friendly, AI-compatible programming language. Use when the user asks about Elysium code, wants to create/modify .ely/.elyx files, build/run/check programs, use Elysium packages (langchain, langgraph, auth, fs, transport, ble, zigbee, worker, regex, datetime), or needs help with Elysium syntax, tooling, CLI, EPM, or the UI component framework.
---

# Elysium 2.0 Language Skill

Elysium 2.0 is a modern language combining readability, safety, and performance:
- Full type inference, algebraic data types, generics
- ARC memory management with optional ownership annotations
- Built-in `async`/`await` concurrency, `parallel` blocks, channels
- Declarative reactive UI components
- Spec-driven development with inline tests
- Compiles to native binaries via LLVM; JS runtime for browser/Node.js

## Quick Navigation

- For **full syntax reference**: see [reference.md](reference.md)
- For **practical code examples**: see [examples.md](examples.md)
- For **CONTRIBUTION_GUIDELINES.md**: always consult for conventions (naming, formatting, imports, docs)

## Project Structure

```
.ely or .elyx          Source files (snake_case for libs, kebab-case for examples)
src/                   Rust compiler source (14 .rs files)
elysium-rt/            Rust runtime library
epm/                   Elysium Package Manager
npm-package/           npm distribution (elysium-lang)
examples/              23+ example .ely/.elyx files
packages/              Pure-Elysium source packages (langchain.ely, langgraph.ely)
```

## CLI Usage

```bash
ely build <file>           # Compile .ely/.elyx to native binary
ely run <file>             # Compile and execute
ely check <file>           # Type-check without compiling
ely highlight <file>       # Syntax highlight (ansi/html)
ely lint <file>            # Lint source (text/json)
ely repl                   # Launch interactive REPL
ely doc <file>             # Generate Markdown docs
ely dep-graph <file>       # Generate dependency graph (DOT/JSON)
ely gen-test <file>        # Generate test stubs
ely port <file>            # Port TypeScript/JS to Elysium
```

Flags: `--debug` (DWARF), `--emit-ir` (LLVM IR), `--env <env>` (stub resolution: local/dev/test/prod)

## Project Configuration (elysium.json)

The `elysium.json` manifest at the project root defines the project and its build targets:

```json
{
  "name": "my-project",
  "version": "0.1.0",
  "description": "My Elysium project",
  "environments": {
    "local": "local",
    "dev": "dev",
    "test": "test",
    "prod": "prod"
  },
  "ui": {
    "browser": {
      "target": "js"       // or "wasm" for WebAssembly
    }
  },
  "ssr": {
    "enabled": false,       // enable for server-side rendering
    "runtime": "node"       // "node", "deno", "bun", or "elysium-server"
  }
}
```

- **`ui.browser.target`**: Compilation target for browser UI builds. `"js"` (default) for JavaScript, `"wasm"` for WebAssembly (performance mode). Future targets: `android`, `ios`.
- **`ssr.enabled`**: When targeting a server runtime, set to `true` so the compiler produces server-rendered output instead of client-only bundles.
- **`ssr.runtime`**: The JavaScript/TypeScript server runtime (e.g. `"node"`, `"deno"`, `"bun"`, `"elysium-server"`). Ignored when `ssr.enabled` is `false`.

## EPM (Elysium Package Manager)

```bash
epm install <package>      # Install from EPM registry
epm publish                # Publish your package
epm build                  # Build package
```

## Key Language Rules

### Conventions (from CONTRIBUTION_GUIDELINES.md)
- **Functions & variables**: `camelCase`
- **Types, classes, enums, components**: `PascalCase`
- **Enum variants**: `PascalCase` — e.g. `Circle(radius)`, `Rectangle(w, h)`
- **File names**: `snake_case.ely` for library files, `kebab-case.ely` for examples
- **Constants**: `SCREAMING_SNAKE_CASE`
- **Indentation**: 4 spaces (no tabs)
- **Braces**: opening brace on the same line
- **Doc comments**: Every public item **must** have `/// Summary: <one-line>`
- **Imports**: relative paths (`import "./foo.ely"`), aliased for namespacing (`import "./foo.ely" as foo`)
- **Error handling**: `Result<T, E>` + `?` operator + `try { } catch { } finally { }`

### Core Syntax

| Construct | Syntax |
|-----------|--------|
| Immutable variable | `let name = value` |
| Mutable variable | `var name = value` |
| Exclusive ownership | `only let name = value` |
| Function | `func name(p: Type) -> Ret { body }` |
| Lambda | `(x) -> expr` or `{ x -> expr }` |
| If expression | `if cond then expr else expr` |
| For loop | `for item in collection { }` |
| While loop | `while cond { }` |
| Match | `match val { case pat => expr }` |
| Switch | `switch val { case pat then { } else { } }` |
| Range inclusive | `start…end` |
| Range exclusive | `start..end` |
| Spread | `…collection` |
| String methods | `"hello".length()`, `.toUpper()`, `.trim()`, `.replace()`, `.split()`, `.sha256()` |
| Error propagation | `expr?` |
| Async/Await | `async func name() { await expr }` |
| Channel | `Channel<T>(capacity: N)` with `send`/`receive`/`select` |
| Parallel block | `parallel { stmt1; stmt2 }` |
| Schedule | `schedule "expr" func name() { }` |
| Wait | `wait <millis> ` |
| Assert/Explain | `expr bc "reason"`, `bc cond, "msg"` |
| Guard | `only cond do { }` |
| Unsafe | `unsafe { }` |
| Type alias | `typealias Name = ExistingType` |
| Generic | `func name<T>(x: T) -> T` |
| Option | `T?` (sugar for `Option<T>`) |
| Result | `Result<T, E>` |
| Union | `A | B` |
| Comments | `// line`, `/// doc`, `/* block */` |

### Types
- **Primitives**: `Int`, `Float`, `Bool`, `String`, `Char`
- **Compound**: Record `{ name: String, age: Int }`, Enum `enum Option<T> { Some(T), None }`, Array `[T]`, Tuple `(Int, String)`, Union `Int | String`
- **Classes**: with `class`, fields, `init`, methods, `private`, `lazy`
- **Init shorthand**: fields without initializer use matching constructor param name

### Stubs
```
func name(args) -> Ret stub           # Always available
func name(args) -> Ret stub: [env]    # Only in listed environments
```

### Specs / Testing
```
spec "Name" { feat "desc" { expect expr; todo; todo "msg" } }
describe "Name" { it "desc" { expect expr } }
bench { ... } / bm { ... }
question / question "msg"
```

## Built-in Packages (desugared from `package.method()` to `__package_method()`)

### Console
`print(...)` — bare print with multiple args

### fs (filesystem)
`fs.writeFile(path, content)`, `fs.readFile(path)`, `fs.appendFile(path, content)`, `fs.copyFile(src, dst)`, `fs.rename(src, dst)`, `fs.removeFile(path)`, `fs.createDir(path)`, `fs.removeDir(path)`, `fs.exists(path)`

### transport (HTTP, WebSocket, MQTT)
`transport.get(url)`, `transport.post(url, body)`, `transport.put(url, body)`, `transport.delete(url)`, `transport.wsConnect(url)`, `transport.wsSend(id, msg)`, `transport.wsClose(id)`, `transport.mqttConnect(url, clientId)`, `transport.mqttPublish(client, topic, msg)`, `transport.mqttSubscribe(client, topic)`, `transport.mqttDisconnect(client)`

### string (via method calls on strings)
`.length()`, `.isEmpty()`, `.toUpper()`, `.toLower()`, `.trim()`, `.replace(old, new)`, `.slice(start, end)`, `.split(delim)`, `.contains(substr)`, `.startsWith(prefix)`, `.sha256()`, `.md5()`, `.base64Encode()`, `.base64Decode()`, `.hexEncode()`, `.hmac(secret)`

### regex
`regex.test(pattern, input)`, `regex.match(pattern, input)`, `regex.search(pattern, input)`, `regex.replace(pattern, input, replacement)`, `regex.split(pattern, input)`

### datetime
`datetime.now()`, `datetime.fromTimestamp(ts)`, `datetime.year(ts)`, `datetime.month(ts)`, `datetime.day(ts)`, `datetime.hour(ts)`, `datetime.minute(ts)`, `datetime.second(ts)`, `datetime.weekday(ts)`, `datetime.addDays(ts, n)`, `datetime.addHours(ts, n)`, `datetime.diffSeconds(t1, t2)`, `datetime.format(ts, fmt)`

### ble (Bluetooth Low Energy)
`ble.scan()`, `ble.stopScan()`, `ble.connect(addr)`, `ble.disconnect(addr)`, `ble.readCharacteristic(addr, svc, char)`, `ble.writeCharacteristic(addr, svc, char, value)`, `ble.readRssi(addr)`, `ble.deviceName(addr)`, `ble.isConnected(addr)`, `ble.isScanning`

### zigbee (Home Automation)
`zigbee.start()`, `zigbee.shutdown()`, `zigbee.permitJoin(secs)`, `zigbee.scan()`, `zigbee.on(dev, ep, cluster)`, `zigbee.off(dev, ep, cluster)`, `zigbee.toggle(dev, ep, cluster)`, `zigbee.readAttribute(dev, ep, cluster, attr)`, `zigbee.writeAttribute(dev, ep, cluster, attr, value)`, `zigbee.addToGroup(dev, gid)`, `zigbee.removeFromGroup(dev, gid)`, `zigbee.bind(src, srcEp, dst, dstEp)`, `zigbee.isJoined()`, `zigbee.getPanId()`, `zigbee.getChannel()`, `zigbee.getDeviceCount()`

### auth (JWT, Sessions, Passwords, OAuth2, RBAC, Multi-tenant)
All return String: `auth.jwtSign(payload, expiresIn)`, `.jwtVerify(token)`, `.jwtDecode(token)`, `.createSession(userId, data)`, `.getSession(sessionId)`, `.destroySession(sessionId)`, `.hashPassword(password)`, `.verifyPassword(password, hash)`, `.checkPermission(user, perm)`, `.hasRole(user, role)`, `.hasScope(token, scope)`, `.oauth2Authorize(clientId, redirectUri, scope)`, `.oauth2Token(code, clientId, secret)`, `.oauth2Refresh(refreshToken, clientId)`, `.passkeyRegister(userId, userName)`, `.passkeyAuthenticate(userId)`, `.tenantContext(tenantId)`, `.getTenant()`, `.listTenants()`, `.createTenant(tenantId, config)`, `.grantRole(user, role)`, `.grantPermission(user, perm)`, `.revokeRole(user, role)`, `.revokePermission(user, perm)`, `.generateApiKey(userId)`, `.validateApiKey(apiKey)`, `.checkAccess(userId, resource, action)`, `.setRoles(userId, rolesJson)`, `.setPermissions(userId, permsJson)`

### worker (Portable Workers)
`worker.create(script)`, `.isRunning(id)`, `.post(id, msg)`, `.receive(id)`, `.send(id, msg)`, `.wait(id)`, `.activeCount()`, `.terminate(id)`, `.terminateAll()`

## Elysium Source Packages (import via EPM)

```
import "#/langchain" as langchain     # LLM, Chat, RAG, Agents (transport-based)
import "#/langgraph" as langgraph     # Stateful graph-based agent orchestration
```

### langchain API
`langchain.llm(model, prompt)`, `.chat(model, system, msg)`, `.embed(text)`, `.similarity(a, b)`, `.template(template, vars)`, `.rag(query, context)`, `.summarize(text)`, `.analyze(text, instruction)`, `.classify(text, labels)`, `.translate(text, targetLang)`, `.agent(model, instructions, query)`, `.agentStream(model, instructions, query)`, `.chain(steps, input)`, `.extract(text, schema)`

### langgraph API
`langgraph.graph(name)`, `.addNode(gid, name, action)`, `.addEdge(gid, from, to)`, `.addConditionalEdges(gid, node, route, targets)`, `.compile(gid)`, `.invoke(gid, state)`, `.stream(gid, state)`, `.getState(gid)`, `.updateState(gid, state)`, `.branch(gid, tasks)`, `.interrupt(gid, reason)`, `.resume(gid)`

## UI Components

```
component Name {
    state varName = initialValue    # Observable mutable state
    state varName: Type             # State with type annotation

    Column {
        Text("hello") style { color: "red", fontSize: 24, bold: true }
        Button(label: "Click") onClick { count = count + 1 }
        TextField(value = name)     # Two-way binding via = syntax
        if cond then Text("yes") else Text("no")
        only cond do { RestrictedContent }
        …items.map(item => Row { Text(item) })
    }
}
```

Built-in views: `Text`, `Button`, `TextField`, `Image`, `Column`, `Row`, `ScrollView`, `ListView`

## Error Handling Pattern

```elysium
func parseNumber(s: String) -> Result<Int, String> {
    if let num = Int(s) { Result.ok(num) }
    else { Result.err("invalid: " + s) }
}

let value = doRisky()?    # Propagates error early
try { risky() } catch { handle() } finally { cleanup() }
```

## Schedule (Cron) Formats

- Cron: `*/5 * * * *`, `0 8 * * *`, `0 */2 * * *`
- Friendly intervals: `every 5 minutes`, `every hour`, `hourly`, `minutely`, `daily`, `weekly`, `monthly`
- Time-of-day: `daily at 08:00`, `at 08:00 every day`, `every Monday at 09:00`

## Adding a New Builtin Package (7-layer pattern)

See CONTRIBUTION_GUIDELINES.md for detailed steps:
1. JS Runtime (`npm-package/runtime/<name>.js`)
2. Export (`npm-package/runtime/index.js`)
3. Desugaring (`src/main.rs`)
4. Type Checker (`src/type_checker.rs`)
5. MIR Enum (`src/mir.rs`)
6. MIR Lowering (`src/mir.rs`)
7. Codegen (`src/codegen.rs`)

## Always Consult These Files

- `CONTRIBUTION_GUIDELINES.md` — conventions for naming, formatting, imports, docs, testing, adding packages
- `reference.md` — full syntax reference
- `examples.md` — code examples for common tasks
- Project source code in `src/` for compiler internals
- Example files in `examples/` for idiomatic patterns
