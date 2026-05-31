# Elysium 2.0 — Complete Reference

## 1. Variables & Mutability

```elysium
let name = "Alice"         // immutable
var count = 0              // mutable
count = count + 1
only let data = readFile("x")  // exclusive ownership (no aliasing)
private let secret = 42        // private to module
lazy let x = expensive()       // evaluated on first access
```

## 2. Functions

```elysium
func add(a: Int, b: Int) -> Int { a + b }                    // named
func average(…values: Float) -> Float { sum(values) / values.count }  // rest param
func identity<T>(x: T) -> T { x }                            // generic
async func fetchData() -> String { let d = await get(); d }   // async
private func helper() -> Int { return 42 }                    // private
private lazy func cached() -> String { return "computed" }    // private + lazy

// Single-expression body
func add(a: Int, b: Int) -> Int => a + b

// Lambda
let square = (x) -> x * x
let square = { x -> x * x }
```

## 3. Control Flow

```elysium
// If-then-else (expression)
let status = if age >= 18 then "adult" else "minor"

// For
for item in [1, 2, 3] { print(item) }
for i in 1…5 { print(i) }    // inclusive: 1,2,3,4,5
for i in 1..5 { print(i) }   // exclusive: 1,2,3,4

// While
while condition { body }

// Match
match value {
    case ok(num) => print(num)
    case err(msg) => print("Error: " + msg)
}

// Switch (desugars to match)
switch score {
    case 100 then { "Perfect!" }
    case 90 then { "Excellent!" }
    else { "Needs improvement" }
}
```

## 4. Types

### Primitives
`Int`, `Float`, `Bool`, `String`, `Char`, `Nil`

### Compound
- Array: `[T]` — e.g. `[Int]`, `[String]`
- Tuple: `(Int, String)`
- Record: `{ name: String, age: Int }`
- Option: `T?` — sugar for `Option<T>`
- Result: `Result<T, E>`
- Union: `A | B`
- Function: `(ArgType) -> ReturnType`

### Enums

```elysium
enum Shape {
    Circle(radius: Float)
    Rectangle(width: Float, height: Float)
}

enum Option<T> {
    Some(T)
    None
}
```

### Classes

```elysium
class Car {
    let make
    let model
    private let vin           // private field

    init(make, model) {
        this.make = make
        this.model = model
    }

    private func validate() -> Bool { return true }   // private method
    func drive() { print("Driving ", this.make) }
}
```

### Type Aliases

```elysium
typealias Name = String
typealias Callback = (Int) -> String
```

## 5. Error Handling

```elysium
func parseNumber(s: String) -> Result<Int, String> {
    if let num = Int(s) { Result.ok(num) }
    else { Result.err("invalid: " + s) }
}

let value = doRisky()?    // Propagate Result error early, return from function

try {
    performRiskyOperation()
} catch ErrorType.Network {
    print("Network error")
} finally {
    cleanup()
}
```

## 6. Memory Model

- Automatic Reference Counting (ARC) — deterministic deletion
- `weak` / `unowned` references to break cycles
- `only let` for exclusive ownership (compile-time checked, no aliasing)
- `unsafe { }` for raw pointers (use sparingly)

## 7. Concurrency

```elysium
async func fetchData() -> String {
    let data = await httpGet("https://example.com")
    return data
}

// Lightweight tasks
async { producer(ch) }
async { consumer(ch) }

// Channels
let ch = Channel<Int>(capacity: 3)

async func producer(ch: Channel<Int>) {
    for i in 1..5 { await ch.send(i) }
    ch.close()
}

async func consumer(ch: Channel<Int>) {
    while let value = await ch.receive() {
        print("Got: ", value)
    }
}

// Select on multiple channels
select {
    case msg = ch1 => handle(msg)
    case msg = ch2 => handle(msg)
}

// Parallel block — each statement in its own thread
parallel {
    print("task a\n")
    print("task b\n")
    print("task c\n")
}
```

## 8. Human-Centric Constructs

### bc / because — inline explanation & assertion
```elysium
let age = 18 bc "minimum voting age"
bc age >= 16, "You must be at least 16 to drive."
let result = calculate() bc "result must be positive"
func discount(age: Int) -> Float bc "Discount policy v3.2" { ... }
```

### only — guard, exclusive match, ownership
```elysium
only item > 0 do { process(item) }        // guard
match value { only Int => print("int") }  // exclusive type match
only let data = readFile("data.txt")      // exclusive ownership
```

### … (ellipsis) — range, rest, spread
```elysium
1…5                  // inclusive range: 1,2,3,4,5
1..5                  // exclusive range: 1,2,3,4
func name(…p: Type)   // rest parameter
[1, 2, …list]         // spread into array
let [first, …rest] = list  // destructuring rest
```

## 9. Doc Comments (Required)

```elysium
/// Summary: Adds two integers and returns the result.
/// Parameters: a - first integer, b - second integer
/// Returns: sum of a and b
func add(a: Int, b: Int) -> Int { a + b }
```

Every public function, class, enum, component, and typealias **must** have `/// Summary:`.

## 10. Imports

```elysium
import "./math.ely"                  // relative import
import "./math.ely" as math          // aliased import for namespacing
import "#/@elysium/langchain" as langchain    // EPM package import
```

## 11. UI Components

```elysium
/// Summary: A simple counter with increment button.
component Counter {
    state count = 0 bc "current counter value"

    Column padding: 20 {
        Text("Count: ", count) style { fontSize: 24 }
        Button(label: "Increment") onClick {
            count = count + 1
        }
    }
}
```

## 12. Spec-Driven Development

```elysium
spec "Calculator" {
    feat "adds two numbers" {
        let result = add(2, 3)
        expect result == 5
        todo "test negative numbers"
    }
}

describe "Greeter" {
    it "says hello" {
        let msg = greet("World")
        expect msg == "Hello, World"
    }
}

bench { let _ = compute() }
bm { let _ = compute() }
question "why is this here?"
```

## 13. Schedule (Cron)

```elysium
schedule "*/5 * * * *" func everyFiveMinutes() { print("tick\n") }
schedule "every 10 minutes" func everyTen() { print("tick\n") }
schedule "daily at 08:00" func morningRoutine() { print("good morning\n") }
schedule "every Monday at 09:00" func mondayMorning() { print("monday\n") }
schedule "hourly" func eachHour() { print("once per hour\n") }
```

## 14. Wait

```elysium
wait 1000    // pause for 1 second
wait 500     // pause for 500ms
```

## 15. Stubs

```elysium
func placeholder(x: Int) -> Int stub               // available in all envs
func debugOnly(key: String) -> String stub: [local, dev]   // filtered by env
func testHelper() -> String stub: [test]            // test only
func crossEnv() -> Float stub                      // bare stub = all envs
```

## 16. Keywords — Complete List (57 tokens)

`let`, `var`, `func`, `if`, `else`, `then`, `for`, `in`, `while`, `return`, `match`, `case`, `try`, `catch`, `finally`, `async`, `await`, `class`, `init`, `enum`, `component`, `state`, `bc`, `because`, `only`, `unsafe`, `weak`, `unowned`, `typealias`, `import`, `as`, `true`, `false`, `nil`, `do`, `render`, `spec`, `describe`, `feat`, `it`, `expect`, `todo`, `question`, `bench`, `bm`, `stub`, `switch`, `private`, `lazy`, `parallel`, `schedule`, `wait`, `worker`

## 17. Operators

`+`, `-`, `*`, `/`, `%`, `==`, `!=`, `<`, `>`, `<=`, `>=`, `&&`, `||`, `!`, `=`, `->`, `.`, `,`, `:`, `;`, `(`, `)`, `[`, `]`, `{`, `}`, `|`, `?`, `...`, `…`, `..`, `<-`

## 18. Built-in Functions (bare calls)

`print(...)` — variadic print
`sum(...)`, `min(...)`, `max(...)`, `abs(n)`, `len(coll)`, `count(coll)`, `isEmpty(coll)`, `map(fn)`, `filter(fn)`, `reduce(fn)`

## 19. Type Names (built-in)

`Int`, `Float`, `Bool`, `String`, `Char`, `Nil`, `Option`, `Result`, `Array`, `Self`

## 20. Environments for Stub Resolution

Defined in `elysium.json`: `local`, `dev`, `test`, `prod`
Use with `--env` CLI flag (default: `local`)
