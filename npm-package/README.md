# Elysium 2.0

**A human-friendly, AI-compatible programming language.**

## Quick Start

```bash
# Install globally
npm install -g elysium-lang

# Run a file
elysium run hello.ely

# Check for errors
elysium check hello.ely

# Interactive REPL
elysium repl

# Generate documentation
elysium doc hello.ely
```

## Usage as a Library

```javascript
const elysium = require('elysium-lang');

// Automatic Reference Counting
const ref = new elysium.Ref({ x: 42 });
console.log(ref.borrow()); // { x: 42 }

// Async task scheduler
const scheduler = new elysium.Scheduler(4);
scheduler.spawn(() => console.log('Hello from Elysium!'));

// Message channels
const chan = new elysium.Channel({ capacity: 10 });
chan.send('hello');
chan.receive().then(console.log); // 'hello'

// Virtual DOM diffing
const { View, diff } = elysium;
const oldViews = [View.text('hello')];
const newViews = [View.text('world')];
const patches = diff(oldViews, newViews);
```

## Commands

| Command | Description |
|---------|-------------|
| `elysium` or `ely` | CLI commands — `ely` is a shorter alias |
| `elysium build <file>` | Compile to native binary |
| `elysium run <file>` | Compile and run |
| `elysium check <file>` | Type-check only |
| `elysium highlight <file>` | Syntax highlighting (ANSI/HTML) |
| `elysium lint <file>` | Lint source code |
| `elysium doc <file>` | Generate Markdown documentation |
| `elysium dep-graph <file>` | Generate dependency graph (DOT/JSON) |
| `elysium gen-test <file>` | Generate test stubs |
| `elysium repl` | Interactive REPL |

## Runtime API

The runtime provides four modules mirroring the Rust `elysium-rt` crate:

- **arc** — Reference counting (`Ref`, `Weak`, `Unowned`)
- **task** — Async scheduler (`Scheduler`, `Task`)
- **channel** — Message passing (`Channel`)
- **ui** — Virtual DOM with diffing (`View`, `Style`, `ComponentState`, `diff`)

## License

MIT
