# Elysium Language — VS Code Extension

Syntax highlighting, snippets, and language support for the [Elysium programming language](https://github.com/imstevetran/ellysium-pl).

Elysium is a human-friendly, AI-compatible language with: ARC memory management, spec-driven development, async/await, parallel blocks, declarative UI components, and more.

## Features

- **Syntax highlighting** for `.ely` and `.elyx` files
- **CLI commands** — run, build, check, test, init, install, publish, update (via `Ctrl+Alt+R/B/T` or right-click menu)
- **Code snippets** for common Elysium constructs (func, let, var, for, if, class, component, spec, extension, switch, render, etc.)
- **Language configuration**: bracket matching, auto-closing pairs, indentation rules, comment toggling

## Supported Constructs

| Category | Examples |
|----------|---------|
| Keywords | `let`, `var`, `func`, `if`, `else`, `then`, `for`, `in`, `while`, `return`, `match`, `case`, `switch`, `import`, `as`, `class`, `init`, `enum`, `component`, `state`, `render`, `extension`, `async`, `await`, `parallel`, `private`, `lazy`, `stub`, `try`, `catch`, `finally`, `do`, `bc`, `because`, `only`, `unsafe`, `weak`, `unowned` |
| Spec-driven | `spec`, `describe`, `feat`, `it`, `expect`, `todo`, `question`, `bench`, `bm` |
| Types | `Int`, `Float`, `Bool`, `String`, `Char`, `Nil`, `Option`, `Result`, `Array`, `Dict`, `Self` |
| Builtins | `print`, `console`, `fs`, `transport`, `string`, `regex`, `datetime`, `math`, `dict`, `json`, `env`, `http`, `worker` |
| Comments | `//` line, `///` doc, `/* */` block |
| .elyx UI | `component`, `state`, `render`, XML/JSX-like tags, `style`, event handlers |

## CLI Commands

| Command | Keybinding | Description |
|---------|-----------|-------------|
| `Elysium: Run active file` | `Ctrl+Alt+R` | Compile and run the current `.ely`/`.elyx` file |
| `Elysium: Build active file` | `Ctrl+Alt+B` | Compile to native binary |
| `Elysium: Check active file` | | Type-check only |
| `Elysium: Run tests` | `Ctrl+Alt+T` | Run all spec tests in the project |
| `Elysium: Init Package` | | Create a new package with manifest |
| `Elysium: Install Package` | | Install a package from the EPM registry |
| `Elysium: Publish Package` | | Publish the current package |
| `Elysium: Update Packages` | | Update all installed packages |

Commands are also available via the right-click context menu on `.ely`/`.elyx` files.

## Installation

1. Clone this repo
2. Open the `vscode-elysium` folder in VS Code
3. Press `F5` to launch a new Extension Development Host window
4. Open any `.ely` or `.elyx` file to see syntax highlighting

### Installing from VSIX

```bash
npx @vscode/vsce package
code --install-extension elysium-lang-0.1.0.vsix
```

## Color Theme Mapping

The grammar assigns standard TextMate scopes that integrate with any VS Code color theme:

| Elysium construct | TextMate scope |
|-------------------|----------------|
| Control flow | `keyword.control.elysium` |
| Declarations | `keyword.declaration.elysium` |
| Function names | `entity.name.function.elysium` |
| Class/component names | `entity.name.type.class.elysium` |
| Type names | `storage.type.elysium` |
| Builtin functions | `support.function.builtin.elysium` |
| Strings | `string.quoted.double.elysium` |
| Numbers | `constant.numeric.elysium` |
| Comments | `comment.line.elysium` |
| Doc comments | `comment.line.documentation.elysium` |
| XML tags (.elyx) | `meta.tag.elysium` |
