# Elysium 2.0

**A human-friendly, AI-compatible programming language.**

Elysium 2.0 is a modern language that combines readability, safety, and performance. Its syntax is clean and unambiguous. The type system offers full inference, algebraic data types, generics, and `Result`/`Option` types. Memory uses automatic reference counting (ARC) with optional ownership annotations. Concurrency is built-in via `async`/`await` and channels. Code is documented with enforced `/// Summary:` blocks.

The language is designed to minimize micromanagement while giving you full control when you need it — ideal for beginners, rapid development, and AI-assisted coding.

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
| `match` | Pattern matching with algebraic data types |
| `Result<T, E>` | Type-safe error handling with `?` propagation |
| `async` / `await` | Lightweight concurrency with channels |
| `component` | Declarative, reactive UI components |

---

## Documentation

| File | Description |
|------|-------------|
| [`docs/SYNTAX.md`](docs/SYNTAX.md) | Complete syntax reference for the language |
| [`docs/UI.md`](docs/UI.md) | Declarative UI layer — components, state, events |

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

## Status

Elysium 2.0 is currently a **language specification and design document**. There is no compiler or runtime implementation yet. This repository captures the syntax, type system, memory model, concurrency model, and UI layer as a reference for future implementation.
