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
    │  Produces: FunctionChunk (main) + Arc<Vec<Value>> constants + Arc<Vec<FunctionChunk>> functions
    ▼
6. VM                     — src/backend/vm.rs
       Executes bytecode (multi-threaded HTTP workers, graceful shutdown)
```

> **Note**: The Expander is part of the `src/parser/` module but runs as a distinct post-parse phase, before semantic analysis.

## Project Structure

```
src/
├── lexer/
│   ├── scanner.rs      # Byte-level scanner (&[u8])
│   └── token.rs        # TokenKind and Span definitions
├── parser/
│   ├── pratt.rs        # Pratt parser (token stream → AST)
│   ├── expander.rs     # Include resolution and alias prefixing
│   └── ast.rs          # AST node definitions (Expr, Stmt, Type, Program)
├── sema/
│   ├── checker.rs      # Type checker and variable resolver
│   ├── symbol_table.rs # Hierarchical scope/symbol table with parent-pointer chain
│   └── interner.rs     # String interner (str → StringId)
├── backend/
│   ├── mod.rs          # Bytecode compiler (AST → OpCode) with constant deduplication
│   ├── vm.rs           # Stack-based virtual machine (Arc + RwLock, multi-worker HTTP)
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

Runtime errors produced by the VM include source location information (line and column) derived from the `spans` table stored alongside each `FunctionChunk`'s bytecode.

## Key Design Decisions

- **String Interning**: All identifiers and string literals are interned via `Interner` into `StringId` (u32). This avoids repeated string allocations and heap comparisons throughout the pipeline.

- **Constant Deduplication**: The compiler maintains a `string_constants: HashMap<String, usize>` that ensures each unique string value is stored only once in the constants table. This is especially effective for built-in method names emitted frequently during compilation.

- **Method Dispatch via Enum**: Built-in method calls are compiled to `OpCode::MethodCall(MethodKind, arg_count)` where `MethodKind` is a `Copy` enum covering all ~50 built-in methods. Unknown or dynamic method names (e.g., JSON field access) use the separate `OpCode::MethodCallCustom(name_idx, arg_count)` path. This eliminates string comparisons in the VM dispatch loop.

- **Two-Pass Compilation**: The backend performs a first pass (`register_globals_recursive`) to assign indices to all globals, functions, and fibers before emitting any bytecode.

- **Span-annotated Bytecode**: Each emitted opcode is paired with a `Span` from the source AST. `FunctionChunk` stores `spans: Arc<Vec<Span>>` alongside `bytecode: Arc<Vec<OpCode>>`. The VM uses this to produce line-accurate runtime error messages.

- **Fiber-as-Coroutine**: Fibers are not OS threads. Each `FiberState` stores its own `ip`, `locals`, and `stack`, resumed cooperatively by the VM. Locals and stack are moved (not cloned) in and out of the fiber state on each resume, eliminating per-resume heap allocations.

- **Thread-safe Value Representation**: All shared mutable collections use `Arc<RwLock<T>>` (via `parking_lot`). `FunctionChunk` bytecode and spans are wrapped in `Arc<Vec<...>>` so they can be shared across HTTP worker threads without copying. `Value` is `Send + Sync`.

- **Graceful HTTP Shutdown**: A global `AtomicBool` (`SHUTDOWN`) is set by a Ctrl+C signal handler (via the `ctrlc` crate). All HTTP worker threads poll this flag and exit cleanly before the process terminates.

- **Scope Chain Symbol Table**: The `SymbolTable` uses a parent-pointer linked chain instead of deep cloning. Entering a function scope creates a new `SymbolTable` with a reference to the parent — lookup walks the chain upward. Function-scope entry is O(1) instead of O(n).

- **Byte-level Scanner**: The lexer operates on `&[u8]` (a reference to the original source bytes) without allocating a `Vec<char>`. Unicode handling is done only where needed. Comment detection uses `slice.starts_with(b"---")`.