# Elysium 2.0 — Syntax Reference

> **Core Philosophy**  
> Write code that reads like a clear explanation, not a cryptic incantation. The language handles memory, type inference, and common patterns automatically, but gives you full power when you need it.

---

## 1. General Syntax Rules

- **Statement‑oriented, block‑based** (indentation or braces).
- **Method calls**: `object.method(args)` — always unambiguous.
- **Variable declaration**: `let name = value` (immutable), `var name = value` (mutable).
- **Comments**:
  - `//` line comment
  - `///` doc comment (or `! Summary:`) — enforced by compiler for production code.

---

## 2. Variables & Mutability

```elysium
let name = "Alice"        // immutable
var count = 0             // mutable
count = count + 1

only let data = readFile("data.txt")   // exclusive ownership (no aliasing)
```

| Keyword | Semantics                              |
| ------- | -------------------------------------- |
| `let`   | Immutable binding                      |
| `var`   | Mutable binding                        |
| `only`  | Exclusive ownership (compile‑time checked) |

---

## 3. Functions

```elysium
// Basic function
func add(a: Int, b: Int) -> Int {
    a + b
}

// Single-expression body (no braces needed)
func add(a: Int, b: Int) -> Int { a + b }

// Lambda
let square = (x) -> x * x
let square = { x -> x * x }

// Rest parameter
func average(…values: Float) -> Float {
    sum(values) / values.count
}

// Generic function
func identity<T>(x: T) -> T { x }

/// Summary: Adds two integers and returns the result.
/// Parameters: a - first integer, b - second integer
/// Returns: sum of a and b
func add(a: Int, b: Int) -> Int { a + b }
```

### Function syntax

```
func name(param: Type, ...) -> ReturnType { body }
func name(param: Type, ...) -> ReturnType => expression   (alternate single‑expr form)
```

- Parameters are **name: Type**.
- Return type follows `->`.

---

## 4. Control Flow

### 4.1 `if` / `else`

```elysium
// Statement form
if age >= 18 {
    print("Adult")
} else {
    print("Minor")
}

// Expression form (returns a value)
let status = if age >= 18 then "adult" else "minor"
let discount = if total > 100 then 0.1 else 0.0
let greeting = if hour < 12 then "Good morning" else "Good afternoon"
```

**`if … then` expression** — reads like natural English:

```
if Condition then Expression1 else Expression2
if Condition then Expression1   // else → nil / Option
```

- Both branches must be of compatible types.
- The `then` keyword is mandatory (avoids ambiguity with statement `if`).

### 4.2 `for` loops

```elysium
for item in [1, 2, 3, 4] {
    print(item)
}

for i in 1…5 {        // inclusive range: 1,2,3,4,5
    print(i)
}

for i in 1..5 {        // exclusive range: 1,2,3,4
    print(i)
}
```

### 4.3 `while` loops

```elysium
while condition {
    // body
}
```

### 4.4 `match` (Pattern Matching)

```elysium
match value {
    case pattern => expression
    case pattern => expression
    // ...
}

// Example
let result = parseNumber("42")
match result {
    case ok(num) => print(num)
    case err(msg) => print("Error: " + msg)
}
```

---

## 5. Types

### 5.1 Primitive Types

| Type     | Description  |
| -------- | ------------ |
| `Int`    | Integer      |
| `Float`  | Floating‑point |
| `Bool`   | Boolean      |
| `String` | Text string  |
| `Char`   | Single character |

### 5.2 Compound Types

| Type                       | Syntax / Example               |
| -------------------------- | ------------------------------ |
| Record / Struct            | `{ name: String, age: Int }`   |
| Enum (Algebraic Data Type) | `enum Option<T> { Some(T), None }` |
| Array                      | `[T]`, e.g. `[Int]`            |
| Tuple                      | `(Int, String)`                |
| Union                      | `Int | String`                 |
| Option                     | `T?` (sugar for `Option<T>`)   |
| Result                     | `Result<T, E>`                 |

### 5.3 Type Aliases

```elysium
typealias Name = String
```

### 5.4 Generics

```elysium
func identity<T>(x: T) -> T { x }

func first<T>(arr: [T]) -> T? {
    if arr.isEmpty { nil }
    else { arr[0] }
}
```

### 5.5 Pattern Matching on Enums

```elysium
enum Shape {
    Circle(radius: Float)
    Rectangle(width: Float, height: Float)
}

func area(shape: Shape) -> Float {
    match shape {
        case Circle(radius) => 3.14159 * radius * radius
        case Rectangle(w, h) => w * h
    }
}
```

---

## 6. Classes / Records

```elysium
class Car {
    let make
    let model

    init(make, model) {
        this.make = make
        this.model = model
    }

    func drive() {
        print("Driving the ", this.make, " ", this.model)
    }
}
```

---

## 7. Error Handling

### 7.1 Try / Catch / Finally

```elysium
try {
    performRiskyOperation()
} catch ErrorType.Network {
    print("Network error occurred.")
} finally {
    cleanup()
}
```

### 7.2 Result Type & `?` Propagation

```elysium
func parseNumber(s: String) -> Result<Int, String> {
    if let num = Int(s) {
        Result.ok(num)
    } else {
        Result.err("invalid number: " + s)
    }
}

let result = parseNumber("42")
match result {
    case ok(num) => print(num)
    case err(msg) => print("Error: " + msg)
}

// Propagating error with ?
let value = doRisky()?   // returns early if Result is err
```

---

## 8. Memory Model

- **Automatic Reference Counting (ARC)** with optional ownership annotations (like Swift / Val).
- No garbage collection pause; deterministic deletion.
- `weak` and `unowned` references to break cycles.
- `only let` for exclusive ownership (no aliasing, compile‑time checked).
- For advanced scenarios: `unsafe { ... }` allows raw pointers (use sparingly).

---

## 9. Concurrency

```elysium
// Async function
async func fetchData() -> String {
    let data = await httpGet("https://example.com")
    return data
}

// Lightweight tasks (green threads)
async { producer(ch) }
async { consumer(ch) }

// Channels for safe message passing
let ch = Channel<Int>(capacity: 3)

async func producer(ch: Channel<Int>) {
    for i in 1..5 {
        await ch.send(i)
    }
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
```

---

## 10. Human‑Centric Constructs

### 10.1 `bc` / `because` — Inline Explanation & Assertion

**Syntax**:
```
expression bc "reason string"
expression because "reason string"
bc condition, "message"    // assertion form
```

**Semantics**:
- Attached to a value → returns the value unchanged. Reason stored in metadata for AI / debugging.
- As a statement → assertion: if condition is false at runtime, terminates with the message.

**Examples**:
```elysium
let age = 18 bc "minimum voting age"
bc age >= 16, "You must be at least 16 to drive."
let result = calculate() bc "result must be positive"
```

### 10.2 `if … then` — Conditional Expression

**Syntax**:
```
if Condition then Expression1 else Expression2
if Condition then Expression1   // result is nil/Option when condition is false
```

**Examples**:
```elysium
let status = if age >= 18 then "adult" else "minor"
let discount = if total > 100 then 0.1 else 0.0
```

### 10.3 `only` — Guard / Exclusive Match / Ownership

**Syntax**:
```
only <condition> do { ... }                // guard
match value { only Type => ... }           // exclusive type match
only let x = expr                           // exclusive ownership
```

**Examples**:
```elysium
// Guard in loops
for item in items {
    only item > 0 do
        process(item)
}

// Pattern matching exclusive
match value {
    only Int   => print("exact integer")
    only Float => print("exact float")
    _          => print("other")
}

// Ownership
only let data = readFile("data.txt")
```

### 10.4 `…` (Ellipsis) — Range / Rest / Spread

**Syntax**:
```
start … end     // inclusive range
start .. end     // exclusive range
func name(…param: Type)   // rest parameter
[1, 2, …list]    // spread into collection
let [first, …rest] = list // destructuring rest
```

**Examples**:
```elysium
// Range
for i in 1…5 { print(i) }   // 1,2,3,4,5

// Rest parameter
func average(…values: Float) -> Float {
    sum(values) / values.count
}

// Spread into array
let more = [4, 5]
let all = [1, 2, 3, …more]   // [1,2,3,4,5]

// Destructuring with rest
let (first, …rest) = (1, 2, 3, 4)
// first = 1, rest = [2,3,4]
```

---

## 11. AI / Documentation

Every module, function, and type **must** have a `/// Summary:` (or `! Summary:`) comment — enforced by the compiler for production code.

```
/// Summary: Greet the user by name.
func greet(name: String) -> String {
    "Hello, " + name + "!"
}
```

Recommended schema:

```
/// Summary: <one‑line description>
/// Parameters: <param> - <description>, ...
/// Returns: <description>
```

---

## 12. UI Layer (Declarative, Reactive, Component‑Based)

### 12.1 Component Syntax

```
component Name {
    state varName = initialValue

    // View tree
    ViewType { ... }
}
```

### 12.2 State

```elysium
component Counter {
    state count = 0

    Column {
        Text("Count: ", count)
        Button(label: "Increment") onClick {
            count = count + 1
        }
    }
}
```

- `state` declares a mutable, observable variable.
- Any view that reads a state variable automatically re‑renders when it changes.

### 12.3 Conditional Rendering

```elysium
component Greeting {
    state isLoggedIn = true

    Column {
        if isLoggedIn then
            Text("Welcome back!")
        else
            Button(label: "Login") {
                isLoggedIn = true
            }
    }
}
```

### 12.4 Guards with `only`

```elysium
component SecureContent {
    state userRole = "admin"

    only userRole == "admin" do {
        Text("Top secret data")
    }
}
```

### 12.5 Lists & Spread

```elysium
component ItemList(items: [String]) {
    Column {
        …items.map(item =>
            Row {
                Checkbox(checked: false)
                Text(item)
            }
        )
    }
}
```

### 12.6 Two‑Way Binding

```elysium
component NameEditor {
    state name = "Alice"

    Column {
        Text("Your name:")
        TextField(value = name)
        Text("Hello, ", name)
    }
}
```

### 12.7 Event Handlers with `bc`

```elysium
component SubmitButton {
    Button(label: "Submit") onClick {
        bc isValidForm(), "Form must be valid before submitting"
        submitData()
    }
}
```

### 12.8 Styling

```elysium
Text("Important") style {
    color: "red",
    fontSize: 24,
    bold: true
}
```

### 12.9 Built‑in Views

| View         | Description                 | Common Attributes             |
| ------------ | --------------------------- | ----------------------------- |
| `Text`       | Simple text label           | `content`, `style`            |
| `Button`     | Clickable button            | `label`, `onClick`            |
| `TextField`  | Text input                  | `value` (two‑way), `onChange` |
| `Image`      | Image display               | `src`, `width`, `height`      |
| `Column`     | Vertical stack layout       | children views                |
| `Row`        | Horizontal stack layout     | children views                |
| `ScrollView` | Scrollable container        | `axis` (vertical/horizontal)  |
| `ListView`   | Virtualised scrollable list | `items`, `renderItem`         |

---

## 13. Full Example

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

---

## 14. Quick Reference (One‑Page Summary)

| Construct            | Syntax                                         |
| -------------------- | ---------------------------------------------- |
| Immutable variable   | `let name = value`                             |
| Mutable variable     | `var name = value`                             |
| Exclusive ownership  | `only let name = value`                        |
| Function             | `func name(p: Type) -> Ret { body }`           |
| Lambda               | `(x) -> expr` or `{ x -> expr }`               |
| Rest parameters      | `func name(…p: Type)`                          |
| If expression        | `if cond then expr else expr`                  |
| For loop             | `for item in collection { }`                   |
| While loop           | `while cond { }`                               |
| Match                | `match val { case pat => expr }`               |
| Range (inclusive)    | `start…end`                                    |
| Range (exclusive)    | `start..end`                                   |
| Spread               | `…collection`                                  |
| Try / catch / finally | `try { } catch pat { } finally { }`            |
| Error propagation    | `expr?`                                        |
| Async function       | `async func name() { }`                        |
| Await                | `await task`                                   |
| Channel              | `Channel<T>(capacity: N)`                      |
| Explain / assert     | `expr bc "reason"` / `bc cond, "msg"`          |
| Guard                | `only cond do { }`                             |
| Ownership            | `only let x = expr`                            |
| Unsafe block         | `unsafe { }`                                   |
| Component            | `component Name { state x = v; View { } }`     |
| State declaration    | `state name = value`                           |
| Style block          | `View style { prop: val }`                     |
| Doc comment          | `/// Summary: ...`                             |
| Type alias           | `typealias Name = ExistingType`                |
| Generic              | `func name<T>(x: T) -> T`                      |
| Option type          | `T?`                                           |
| Result type          | `Result<T, E>`                                 |
| Union type           | `A | B`                                        |
