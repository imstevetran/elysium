# Elysium 2.0

**A human-friendly, AI-compatible programming language.**

Elysium 2.0 is a modern language that combines readability, safety, and performance. Its syntax is clean and unambiguous. The type system offers full inference, algebraic data types, generics, and `Result`/`Option` types. Memory uses automatic reference counting (ARC) with optional ownership annotations. Concurrency is built-in via `async`/`await` and channels. Code is documented with enforced `/// Summary:` blocks.

The language is designed to minimize micromanagement while giving you full control when you need it — ideal for beginners, rapid development, and AI-assisted coding.

---

## 🌐 Documentation Site

The full documentation site is available at **[elysium-lang.dev](https://imstevetran.github.io/elysium)** (or your custom domain).

The site includes:
- **Language Guide** — complete syntax reference
- **UI Guide** — declarative component-based UI layer
- **Standard Library** — console, fs, transport, string, regex, datetime
- **Spec-Driven Development** — inline tests with spec/feat/expect
- **Tooling** — CLI, EPM package manager, npm package, linter
- **Recipes** — 16 practical code examples from beginner to advanced

The site source lives in [`docs/`](docs/).

---

## Philosophy

| Principle | Description |
|-----------|-------------|
| **Simplicity** | Syntax designed to be easily readable and writable |
| **Expressiveness** | High-level abstractions for concise representation of complex tasks |
| **Type Safety** | Strong typing with clear error messages |
| **Minimal Micromanagement** | Focus on high-level operations without requiring intricate details |
| **AI-Friendliness** | Well-defined summaries and structure for efficient AI interpretation |

---

## Quick Look

```elysium
/// Summary: Greet the user by name.
func greet(name: String) -> String {
    "Hello, " + name + "!"
}

let message = greet("Alice")
print(message)
```

### Key Features at a Glance

| Feature | Description |
|---------|-------------|
| `let` / `var` | Immutable and mutable bindings |
| `if … then` | Natural-language conditional expressions |
| `bc` / `because` | Inline explanation and assertions |
| `only` | Guards, exclusive matches, and ownership |
| `…` (ellipsis) | Ranges, rest parameters, and spread |
| `match` / `switch` | Pattern matching with algebraic data types |
| `Result<T, E>` | Type-safe error handling with `?` propagation |
| `async` / `await` | Lightweight concurrency with channels |
| `parallel` blocks | Thread-based parallelism |
| `component` | Declarative, reactive UI components |
| `spec` / `feat` / `expect` | Inline spec-driven development |
| `bench` / `bm` | Built-in benchmarking |

---

## Example: Todo App

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

---

## Installation

```bash
npm install -g elysium-lang
```

Then compile and run:

```bash
ely run hello.ely
```

---

## Documentation

| File | Description |
|------|-------------|
| [`docs/`](docs/) | GitHub Pages documentation site source |
| [`docs/SYNTAX.md`](docs/SYNTAX.md) | Complete syntax reference for the language |
| [`docs/UI.md`](docs/UI.md) | Declarative UI layer — components, state, events |
| [`AGENT_GUIDELINES.md`](AGENT_GUIDELINES.md) | Development log and design decisions |

---

## Project Structure

```
.
├── docs/                          # Documentation site (GitHub Pages)
│   ├── index.html                 # Home page
│   ├── 404.html                   # 404 page
│   ├── assets/css/style.css       # Shared styles
│   ├── assets/js/main.js          # Shared scripts
│   ├── guide/index.html           # Language guide
│   ├── ui/index.html              # UI framework guide
│   ├── std/index.html             # Standard library reference
│   ├── spec/index.html            # Spec-driven development
│   ├── tooling/index.html         # CLI, EPM, npm, linter
│   ├── SYNTAX.md                  # Complete syntax reference
│   ├── UI.md                      # Declarative UI layer docs
│   └── recipes/                   # 16 practical code recipes
├── src/                           # Compiler source (14 .rs files)
├── elysium-rt/                    # Rust runtime library
├── epm/                           # Elysium Package Manager
├── npm-package/                   # npm distribution
├── examples/                      # 23 example .ely/.elyx files
├── Cargo.toml                     # Rust workspace definition
├── AGENT_GUIDELINES.md            # Development log
└── README.md                      # This file
```

---

## Status

Elysium 2.0 has a **working Rust compiler** with an LLVM backend (via `inkwell`), a complete JavaScript runtime, a package manager (EPM), and a declarative UI framework. The compiler compiles `.ely` and `.elyx` files to native binaries.
