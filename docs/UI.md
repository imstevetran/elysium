## UI Coding in Elysium 2.0

To make UI development as human‑friendly and AI‑compatible as the core language, I’ve added a **declarative, reactive UI layer** that leverages all of Elysium’s existing constructs (`bc`, `if … then`, `only`, `…`). The goal is to write UIs that read like simple instructions, with minimal syntax and no manual state management wiring.

---

### Core UI Concepts

1. **Component = Function that returns a View**  
   Components are plain functions (or structs) that describe a tree of views. They are automatically reactive – when their state changes, the UI updates efficiently.

2. **State**  
   Use the `state` keyword to declare a mutable, observable variable. Any view that reads a state variable automatically re‑renders when that variable changes.

3. **Views are immutable descriptions** – no direct manipulation of DOM elements. The runtime diffs and updates the actual UI.

4. **Event handlers** are expressed as closures attached to views.

---

### Syntax and Examples

#### 1. Basic Component with State

```elysium
/// Summary: A simple counter with increment button.
component Counter {
    state count = 0 bc "current counter value"

    // Return a view tree
    Column {
        Text("Count: ", count)   // Text automatically re‑renders when count changes
        Button(label: "Increment") onClick {
            count = count + 1
        }
    }
}
```

#### 2. Conditional Rendering with `if … then`

```elysium
component Greeting {
    state isLoggedIn = true

    // Use `if … then` directly inside view tree
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

#### 3. Using `only` as a Guard

```elysium
component SecureContent {
    state userRole = "admin"

    // Only render if userRole equals "admin"
    only userRole == "admin" do {
        Text("Top secret data")
    }
}
```

#### 4. Lists and `…` (Spread)

```elysium
/// Summary: Display a list of items.
component ItemList(items: [String]) {
    Column {
        // Spread the list of Text views
        …items.map(item =>
            Row {
                Checkbox(checked: false)
                Text(item)
            }
        )
    }
}
```

#### 5. Two‑way Binding (Optional)

Use `<-` (or `=`) for a binding between UI input and state.

```elysium
component NameEditor {
    state name = "Alice"

    Column {
        Text("Your name:")
        TextField(value = name)   // changes to the text field update `name`
        Text("Hello, ", name)
    }
}
```

#### 6. Event Handlers with `bc`

```elysium
component SubmitButton {
    Button(label: "Submit") onClick {
        bc isValidForm(), "Form must be valid before submitting"
        submitData()
    }
}
```

#### 7. Styling (Simple Inline Style)

Views accept an optional `style` attribute – a plain dictionary of style properties.

```elysium
Text("Important") style {
    color: "red",
    fontSize: 24,
    bold: true
}
```

---

### Built‑in Views

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

### How It Fits the Elysium Philosophy

- **Human‑friendly** – UI code reads like English sentences.
- **Less micromanagement** – no manual event listener attachment, no DOM queries, no state‑sync logic.
- **AI‑friendly** – the declarative tree structure is easy to parse and generate. `bc` annotations provide clear intent.
- **Freedom** – you can mix logic with UI naturally; no framework‑specific boilerplate.
- **Type‑safe** – views expect correct types (e.g., `onClick` must be a function, not a string).

---

### Full Example: Todo App

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

### Implementation Note

The UI layer is compiled to platform‑native widgets (or a virtual DOM) using a reactive engine. It is **not** a framework bolted onto the language – it is part of the language’s standard library, with first‑class syntax support for views, state, and events. The runtime handles diffing and updates automatically, so developers never think about “how” to update the UI – only “what” they want to show.

---

### Summary for AI

Elysium 2.0’s UI coding is **declarative, reactive, and component‑based**. Use `state` for mutable data, `if … then` for conditionals, `only` for guards, `…` for list spreading, and `bc` for documentation. All views are functions that return a view tree. No manual DOM or state management required. The syntax is designed to be self‑explanatory and easy to generate or parse by an AI.
