# Agent Guidelines — Elysium 2.0

## Design Decisions

### Spec-Driven Development Keywords
- `spec` / `describe` — test suite (both work, synonyms)
- `feat` / `it` — individual test case (both work, synonyms)
- `expect <expr>` — assertion statement
- `todo ["message"]` — todo marker in specs, compiles to nil
- `question ["message"]` — open question/concern marker, compiles to nil

The `question` keyword was chosen over `oq` or `concern` because it's the most intuitive plain-English word. Since `?` already uses `Token::Question` in the lexer, the keyword token is `Token::KwQuestion`.

### Import System
- `import "./path.ely"` — basic import (items are inlined)
- `import "./path.ely" as alias` — aliased import (desugars `alias.fn()` → `alias_fn()`)

### Benchmarking
- `bench { ... }` / `bm { ... }` — benchmark block that measures wall-clock time
- Both are synonyms; `bench` is the full keyword, `bm` is the shorthand
- Implementation emits LLVM IR with `clock_gettime(CLOCK_MONOTONIC, ...)` calls around the body block
- Output: `bench: %.6f s\n` via `printf` to stdout
- The body's statements are compiled inline between start/end timing calls
- Timespec struct is allocated as an `[2 x i64]` on the stack
