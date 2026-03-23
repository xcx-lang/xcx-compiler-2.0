# XCX Compiler — Changelog

## Version 2.1.0

> **Released:** 2026 — Compiler internals release. No changes to XCX language syntax or semantics.

XCX 2.1 is a compiler and runtime engineering release focused entirely on performance, correctness, and production readiness. All improvements are transparent to XCX programs — existing code runs unchanged and faster.

---

### Performance

**Constant table deduplication** — The compiler now maintains a deduplication index for string constants. Repeated string values (such as built-in method names emitted for every call site) are stored only once in the constant table. Programs with many method calls produce significantly smaller bytecode artifacts. *(src/backend/mod.rs)*

**Method dispatch via enum** — Built-in method calls are now dispatched through a `MethodKind` enum resolved at compile time, replacing runtime string comparison in the VM's hot dispatch loop. The ~50 built-in methods covering arrays, sets, maps, tables, strings, dates, fibers, and JSON are matched with a `Copy` enum variant — zero allocation, zero string lookup per call. Dynamic dispatch (`MethodCallCustom`) is reserved for JSON field access and alias-resolved calls only. *(src/backend/vm.rs, src/backend/mod.rs)*

**Fiber resume without heap allocation** — Resuming and suspending a fiber no longer clones the fiber's local variables or operand stack. `std::mem::take` transfers ownership of locals in and out of `FiberState`, and the fiber's stack segment is appended to and split from the main VM stack using `extend`/`split_off`. For generator-style fibers called in tight loops this eliminates the dominant allocation cost entirely. *(src/backend/vm.rs)*

**Byte-level scanner** — The lexer now operates directly on `&[u8]`, a borrowed reference to the original source string. The previous `Vec<char>` allocation (which copied the entire source file upfront) is gone. ASCII scanning is byte-level throughout. UTF-8 handling is done only where required. Multi-byte Unicode operators (`∪`, `∩`, `⊕`) are detected via `starts_with` on byte slices. The `Scanner` struct now carries a lifetime parameter (`Scanner<'a>`) reflecting the zero-copy design. *(src/lexer/scanner.rs, src/parser/pratt.rs)*

**Scope chain symbol table** — Entering a function or fiber scope during semantic analysis no longer performs a deep clone of the entire symbol table. `SymbolTable` now uses a parent-pointer chain: a new child table is created with a reference to the enclosing scope, and lookup walks the chain upward. Scope entry is O(1) instead of O(n). *(src/sema/symbol_table.rs, src/sema/checker.rs)*

---

### Correctness & Developer Experience

**Source-accurate runtime error messages** — Every compiled opcode is now paired with its source span at compile time. `FunctionChunk` stores `spans: Arc<Vec<Span>>` alongside `bytecode: Arc<Vec<OpCode>>`. The VM tracks the current span table across all call frames and fiber resumes. Runtime errors now include the exact line and column from the XCX source file:

```
R303: Array index out of bounds: 5 [line: 14, col: 7]
halt.error: Cannot convert "abc" to Integer [line: 22, col: 3]
```

*(src/backend/vm.rs, src/backend/mod.rs)*

**Pre-scan limited to top-level declarations** — The semantic checker's forward-declaration pass (`pre_scan_stmts`) no longer recurses into `if`, `while`, or `for` bodies. Since XCX requires functions and fibers to be declared at the top level of a file, this recursion was unnecessary. Each function body is now visited exactly once during the main checking pass, eliminating duplicate AST traversal. *(src/sema/checker.rs)*

**Dead code removed from parser** — An unreachable duplicate `LeftBrace` branch in `parse_var_decl` has been removed. The second branch was structurally identical to the first and could never be reached. *(src/parser/pratt.rs)*

---

### Concurrency & Production HTTP

**Multi-threaded HTTP server** — The `serve:` directive now actually uses its `workers` field. Each worker runs in a dedicated OS thread with its own `Executor` instance and private operand stack. Workers share globals and the compiled function table via `Arc`. *(src/backend/vm.rs)*

**Thread-safe value representation** — All shared mutable collection types (`Array`, `Set`, `Map`, `Table`, `Json`, `Fiber`) have been migrated from `Rc<RefCell<T>>` to `Arc<RwLock<T>>` using `parking_lot`. `Value` is now `Send + Sync`. `FunctionChunk` bytecode and spans are wrapped in `Arc<Vec<...>>` and shared across worker threads without copying. *(src/backend/vm.rs, src/backend/mod.rs)*

**Graceful shutdown** — A global `AtomicBool` (`SHUTDOWN`) is set by a Ctrl+C handler registered via the `ctrlc` crate. All HTTP worker threads poll this flag and exit cleanly. The main thread joins all workers before the process terminates. *(src/backend/vm.rs, src/main.rs)*

**`SharedContext` for zero-copy worker startup** — Constants and function chunks are now distributed to worker threads as `Arc<Vec<...>>` clones — two pointer increments per worker, regardless of program size. *(src/backend/vm.rs, src/backend/mod.rs)*

---

### Internal

- `Compiler::compile` now returns `(FunctionChunk, Arc<Vec<Value>>, Arc<Vec<FunctionChunk>>)` — the main script is represented as a `FunctionChunk` like any other function, providing a uniform entry point for the VM.
- `FunctionCompiler::emit(op, span)` replaces direct `.bytecode.push()` calls throughout the compiler, keeping bytecode and span tables in sync automatically.
- Obsolete opcodes `FiberNext`, `FiberRun`, `FiberIsDone`, `FiberClose` removed from `OpCode`. Fiber lifecycle is now handled entirely through `MethodCall(MethodKind::Next/Run/IsDone/Close, 0)`.
- Fixed a `TableInit` opcode bug that caused incorrect initialization of tables declared with row literals.
- Fixed `Value::Date` representation to use `NaiveDateTime` consistently.
- 160+ unit and integration tests pass with no data races detected.

---

### Dependencies Added

| Crate | Version | Purpose |
|---|---|---|
| `parking_lot` | 0.12 | Faster `RwLock` / `Mutex` replacing `std::sync` |
| `ctrlc` | 3.x | Cross-platform Ctrl+C signal handler |
