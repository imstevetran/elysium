# Elysium 2.0 — Code Examples

## 1. Hello World

```elysium
/// Summary: Greet the user by name.
func greet(name: String) -> String {
    "Hello, " + name + "!"
}

let message = greet("Alice")
print(message)
```

## 2. Variables, Functions, and bc

```elysium
/// Summary: Calculate discount for a customer.
func discount(age: Int, purchases: [Float]) -> Float bc "Discount policy v3.2" {
    let adultAge = 18 bc "legal adult age"

    only age > 0 do {
        let base = if age >= adultAge then 0.10 else 0.05

        let total = sum(…purchases) bc "total of all purchases"
        let extra = if total > 500 then 0.05 else 0.0

        min(base + extra, 0.20)
    }
}
```

## 3. Switch/Case

```elysium
func describe_score(score: Int) -> String {
    return switch score {
        case 100 then { "Perfect!" }
        case 90 then { "Excellent!" }
        case 80 then { "Great!" }
        case 70 then { "Good" }
        case 60 then { "Passing" }
        else { "Needs improvement" }
    }
}
```

## 4. Filesystem Operations

```elysium
func main() {
    fs.writeFile("test.txt", "Hello from Elysium!")
    let content = fs.readFile("test.txt")
    fs.appendFile("test.txt", "\nSecond line")
    fs.copyFile("test.txt", "backup.txt")
    fs.exists("test.txt")
    fs.removeFile("test.txt")
}
```

## 5. HTTP Requests via Transport

```elysium
func main() {
    transport.get("https://api.example.com/data")
    transport.post("https://api.example.com/data", "{\"key\":\"value\"}")
    transport.put("https://api.example.com/data/1", "{\"key\":\"updated\"}")
    transport.delete("https://api.example.com/data/1")
}
```

## 6. String Operations (including Crypto)

```elysium
func main() {
    let len = "hello".length()
    let upper = "hello".toUpper()
    let trimmed = "  hi  ".trim()
    let parts = "a,b,c".split(",")
    let hash = "hello".sha256()
    let b64 = "hello".base64Encode()
    let signature = "message".hmac("secret")
}
```

## 7. Datetime

```elysium
func main() {
    let now = datetime.now()
    let str = datetime.fromTimestamp(now)
    let y = datetime.year(now)
    let m = datetime.month(now)
    let d = datetime.day(now)
    let formatted = datetime.format(now, "%Y-%m-%d %H:%M:%S")
    let tomorrow = datetime.addDays(now, 1)
}
```

## 8. Regex

```elysium
func main() {
    regex.test("\\d+", "hello 123 world")
    regex.match("\\d+", "abc 456 def")
    regex.replace("foo", "hello foo world", "bar")
    regex.split(",", "a,b,c")
}
```

## 9. Import and Namespace

```elysium
// math.ely — library file
func square(x: Int) -> Int { x * x }
func double(x: Int) -> Int { x + x }

// main.ely — consumer
import "./math.ely" as math

let sq = math.square(5)
let db = math.double(5)
print(sq)
print(db)
```

## 10. Async, Await, Parallel

```elysium
async func fetchData() -> string {
    return "data"
}

func main() {
    parallel {
        print("task a\n")
        print("task b\n")
        print("task c\n")
    }
    print("all parallel tasks done\n")
}
```

## 11. Spec-Driven Development

```elysium
func add(a: Int, b: Int) -> Int { a + b }

spec "Calculator" {
    feat "adds two numbers" {
        let result = add(2, 3)
        expect result == 5
    }
    feat "adds negative numbers" {
        let result = add(-1, -2)
        expect result == -3
    }
}

describe "Greeter" {
    it "says hello" {
        let msg = greet("World")
        expect msg == "Hello, World"
    }
}
```

## 12. Classes

```elysium
/// Summary: A car with make and model.
class Car {
    let make
    let model
    private let vin

    init(make, model) {
        this.make = make
        this.model = model
    }

    private func validate() -> Bool { return true }
    func drive() { print("Driving the ", this.make, " ", this.model) }
}
```

## 13. UI Component

```elysium
/// Summary: A simple todo list application.
component TodoApp {
    state tasks = [String]()
    state newTask = ""

    Column padding: 20 {
        Text("My Todos") style { fontSize: 24, bold: true }

        Row {
            TextField(value = newTask) placeholder "Add a task"
            Button(label: "Add") onClick {
                only newTask.trim() != "", "Task cannot be empty"
                tasks = tasks + [newTask]
                newTask = ""
            }
        }

        if tasks.isEmpty then
            Text("No tasks yet!") style { color: "gray" }
        else
            Column {
                …tasks.map((task, index) =>
                    Row {
                        Text(task)
                        Button(label: "Delete") onClick {
                            tasks = tasks.filter((_, i) => i != index)
                        }
                    }
                )
            }
    }
}
```

## 14. LangChain — LLM & AI Operations

```elysium
import "#/@elysium/langchain" as langchain

func main() {
    let response = langchain.llm("gpt-4", "Hello!")
    let summary = langchain.summarize("Long text here...")
    let embedding = langchain.embed("text to embed")
    let answer = langchain.rag("What is X?", "Context about X...")
    let analysis = langchain.analyze("text", "sentiment analysis")
    let result = langchain.agent("gpt-4", "You are a helper", "Calculate 2+2")
}
```

## 15. LangGraph — Stateful Agent Graphs

```elysium
import "#/@elysium/langgraph" as langgraph

func main() {
    let gid = langgraph.graph("simple_agent")
    langgraph.addNode(gid, "input", "function node")
    langgraph.addEdge(gid, "input", "output")
    langgraph.addConditionalEdges(gid, "input", "route", "continue, end")
    let compiled = langgraph.compile(gid)
    let result = langgraph.invoke(gid, "idle")
    let streamed = langgraph.stream(gid, "idle")
}
```

## 16. Auth — JWT, Sessions, Passwords, RBAC, OAuth2

```elysium
func main() {
    // JWT
    let token = auth.jwtSign('{"sub": "user123"}', "1h")
    let verified = auth.jwtVerify(token)

    // Password hashing
    let hashed = auth.hashPassword("my-secure-password")
    let match = auth.verifyPassword("my-secure-password", hashed)

    // Permissions
    let hasPerm = auth.checkPermission("user123", "delete:documents")
    let hasRole = auth.hasRole("user123", "admin")

    // RBAC
    auth.setRoles("user123", '["admin", "editor"]')
    let access = auth.checkAccess("user123", "documents", "write")

    // OAuth2
    let url = auth.oauth2Authorize("my-app", "http://localhost:3000/callback", "openid")
    let tokens = auth.oauth2Token("auth_code", "my-app", "client_secret")
}
```

## 17. Schedule (Cron) + Wait

```elysium
schedule "every 10 minutes" func everyTenMinutes() {
    print("tick\n")
}

schedule "daily at 08:00" func morningRoutine() {
    print("good morning\n")
}

func main() {
    print("starting...\n")
    wait 1000    // pause 1 second
    print("done\n")
}
```

## 18. Worker (Portable Threads)

```elysium
func main() -> String {
    let workerId = worker.create("echo worker")
    let running = worker.isRunning(workerId)
    worker.post(workerId, "Hello from Elysium")
    let reply = worker.receive(workerId)
    let echoResult = worker.send(workerId, "ping")
    worker.terminate(workerId)
    return "ok"
}
```

## 19. Build Configuration — WASM & SSR

WASM is a **compilation target** for browser UI builds, set in project config. SSR is a separate option for server-rendered output:

```json
// elysium.json
{
  "ui": {
    "browser": {
      "target": "wasm"       // "js" (default) or "wasm"
    }
  },
  "ssr": {
    "enabled": true,         // enable for server-side rendering
    "runtime": "node"        // "node", "deno", "bun", "elysium-server"
  }
}
```

- When `ui.browser.target: "wasm"`, the UI component code compiles to WebAssembly for performance.
- When `ssr.enabled: true`, the compiler generates a server-rendered bundle for the specified runtime.
- Future targets include `android` and `ios` for native mobile UI.
- No `wasm.*` or `ssr.*` runtime calls are needed in Elysium source code — all configuration is at the project level.

## 20. Error Handling with Result

```elysium
func parseNumber(s: String) -> Result<Int, String> {
    if let num = Int(s) {
        Result.ok(num)
    } else {
        Result.err("invalid number: " + s)
    }
}

func main() -> Result<Int, String> {
    let value = parseNumber("42")?   // propagates error if Result is err
    print(value)
    Result.ok(value)
}
```

## 21. Stubs with Environment Filtering

```elysium
func placeholder(x: Int) -> Int stub

func debugOnly(key: String) -> String stub: [local, dev]

func testHelper() -> String stub: [test]

func crossEnv() -> Float stub

func normalFunc() -> Int { return 42 }
```

## CLI Commands Reference

```bash
# Type-check only
ely check myfile.ely --env local

# Compile to native binary
ely build myfile.ely -o myprogram

# Compile and run
ely run myfile.ely

# With debug info (DWARF for lldb/gdb)
ely run myfile.ely --debug

# Generate LLVM IR
ely build myfile.ely --emit-ir

# Syntax highlight (ANSI terminal)
ely highlight myfile.ely

# Lint
ely lint myfile.ely --format json

# Launch REPL
ely repl

# Generate Markdown docs
ely doc myfile.ely -o docs/api.md

# Generate dependency graph
ely dep-graph myfile.ely --format dot -o graph.dot

# Generate test stubs
ely gen-test myfile.ely -o tests/stubs.ely

# Port TypeScript to Elysium
ely port myfile.ts -o myfile.ely

# EPM package manager
epm install @elysium/langchain
epm install @elysium/langgraph
```
