# XCX Compiler Architecture

The XCX Compiler is implemented in Rust and follows a multi-stage pipeline architecture.

## Compilation Pipeline

```
Source Code
    │
    ▼
1. Lexer (Scanner)        — src/lexer/scanner.rs
    │  Produces: Token stream
    ▼
2. Parser (Pratt)         — src/parser/pratt.rs
    │  Produces: Raw AST (Program)
    ▼
3. Expander               — src/parser/expander.rs
    │  Produces: Expanded AST (include directives resolved, aliases prefixed)
    ▼
4. Type Checker (Sema)    — src/sema/checker.rs
    │  Produces: Validated, annotated AST
    ▼
5. Compiler (Backend)     — src/backend/mod.rs
    │  Produces: Bytecode (Vec<OpCode>) + constants + function chunks
    ▼
6. VM                     — src/backend/vm.rs
       Executes bytecode
```

> **Note**: The Expander is part of the `src/parser/` module but runs as a distinct post-parse phase, before semantic analysis.

## Project Structure

```
src/
├── lexer/
│   ├── scanner.rs      # Character-level scanner
│   └── token.rs        # TokenKind and Span definitions
├── parser/
│   ├── pratt.rs        # Pratt parser (token stream → AST)
│   ├── expander.rs     # Include resolution and alias prefixing
│   └── ast.rs          # AST node definitions (Expr, Stmt, Type, Program)
├── sema/
│   ├── checker.rs      # Type checker and variable resolver
│   ├── symbol_table.rs # Hierarchical scope/symbol table
│   └── interner.rs     # String interner (str → StringId)
├── backend/
│   ├── mod.rs          # Bytecode compiler (AST → OpCode)
│   ├── vm.rs           # Stack-based virtual machine
│   └── repl.rs         # Interactive REPL
└── diagnostic/
    └── report.rs       # Error reporter with source highlighting
```

## Diagnostic System

The compiler uses a `Reporter` struct to produce contextual error messages. Each error includes:
- **Level**: ERROR or HALT variant
- **Location**: line and column number
- **Visual highlight**: the relevant source line with a `~~~` underline

Semantic errors (`TypeError`) are collected in a `Vec` during the checking phase and reported all at once before bytecode generation begins. If any errors exist, compilation stops at that point.

## Key Design Decisions

- **String Interning**: All identifiers and string literals are interned via `Interner` into `StringId` (u32). This avoids repeated string allocations and heap comparisons throughout the pipeline.
- **Two-Pass Compilation**: The backend performs a first pass (`register_globals_recursive`) to assign indices to all globals, functions, and fibers before emitting any bytecode.
- **Fiber-as-Coroutine**: Fibers are not OS threads. Each `FiberState` stores its own `ip`, `locals`, and `stack`, resumed cooperatively by the VM.
- **No Garbage Collector**: Memory is managed via Rust's `Rc<RefCell<T>>` for shared mutable collections (Array, Set, Map, Table, Json, Fiber).