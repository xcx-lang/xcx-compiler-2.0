# XCX Semantic Analysis (Sema)

The Sema phase validates the AST for logical correctness and type consistency before bytecode generation. It consists of two components: the **Symbol Table** and the **Type Checker**.

## String Interner (`src/sema/interner.rs`)

`Interner` maps `&str → StringId(u32)`. It is the single source of truth for all string identities in the compiler. Created during lexing/parsing, passed by reference to the checker and compiler.

```
Interner::intern("foo") → StringId(42)
Interner::lookup(StringId(42)) → "foo"
```

## Symbol Table (`src/sema/symbol_table.rs`)

The `SymbolTable` manages variable bindings across nested scopes using a **parent-pointer linked chain**.

### Structure

```rust
pub struct SymbolTable<'a> {
    parent: Option<&'a SymbolTable<'a>>,  // reference to enclosing scope
    scopes: Vec<HashMap<String, Type>>,   // stack of scope frames (local to this table)
    consts: Vec<HashSet<String>>,         // which names are const, per scope
}
```

### Creating a Child Scope

When entering a function or fiber body, a new `SymbolTable` is created with a reference to the enclosing table instead of cloning it:

```rust
let mut func_symbols = SymbolTable::new_with_parent(symbols);
```

This is O(1) — only a single empty `HashMap` is allocated for the new frame. Lookup travels up the parent chain as needed.

### Scope Lifecycle

- `enter_scope()` / `exit_scope()` push/pop local frames within a table — used for `if`, `while`, `for` bodies.
- `new_with_parent(parent)` creates a child table linked to the parent — used for function and fiber bodies.
- `define(name, ty, is_const)` always writes to the **innermost** (current) scope of the current table.
- `lookup(name)` walks local scopes from innermost to outermost, then follows the `parent` pointer chain.
- `has_in_current_scope(name)` checks only the innermost frame of the current table — used to detect redefinition within the same block.

### Important: No Variable Shadowing

XCX does **not** support variable shadowing. Defining a variable that already exists in the **current scope** raises `RedefinedVariable`. Variables in parent scopes are accessible but cannot be re-declared in a child scope using the same name.

### `is_const(name)`

Looks up which scope owns `name`, then checks the corresponding `consts` set. Follows the parent chain if not found locally. Reassigning a const produces `ConstReassignment`.

## Type Checker (`src/sema/checker.rs`)

The `Checker` struct walks the AST and accumulates `TypeError` values. The program is only compiled if the resulting `Vec<TypeError>` is empty.

### Pre-Scan Pass (`pre_scan_stmts`)

Before checking any statement body, the checker performs a **forward-declaration scan** of all top-level `FunctionDef` and `FiberDef` nodes in the current statement list. This allows functions and fibers to be called before their definition in the source file (mutual recursion, call before declare).

The scan is **limited to top-level statements only** — it does not recurse into `if`/`while`/`for` bodies, since XCX requires functions and fibers to be declared at the top level. Each function's or fiber's body is visited exactly once during the main checking pass.

For each function/fiber found, the checker registers:
- A `FunctionSignature { params, return_type, is_fiber }` in `self.functions` (HashMap)
- An entry in the `SymbolTable` with `Type::Unknown` (for functions) or `Type::Fiber(...)` (for fibers)

### Context Flags

The checker maintains several runtime context flags:

| Field | Purpose |
|---|---|
| `loop_depth: usize` | Tracks nesting depth of `while`/`for`. Zero → `break`/`continue` are errors. |
| `fiber_context: Option<Option<Type>>` | `None` = not in fiber; `Some(None)` = void fiber; `Some(Some(T))` = typed fiber yielding `T`. |
| `fiber_has_yield: bool` | Set to `true` when a `yield` is encountered inside a fiber body. |
| `is_table_lambda: bool` | Set to `true` inside `.where()` predicates; allows bare column names as identifiers. |

### Type Inference Rules

- Expression types are inferred bottom-up from literals and propagate through operators.
- `Type::Unknown` acts as a wildcard — any operation involving `Unknown` passes without error (used for dynamically typed or unresolved expressions).
- `Type::Json` is compatible with any type in assignments and comparisons.
- Numeric promotion: `Int op Float → Float`.

### `is_compatible(expected, actual) -> bool`

Key compatibility rules:
- `Unknown` is compatible with everything (both directions).
- `Json` is compatible with everything.
- `Int` ↔ `Float` are mutually compatible (numeric promotion).
- `Set(X)` ↔ `Array(inner)` are compatible when inner element type matches.
- `Set(N)` ↔ `Set(Z)` are compatible (both integer-typed sets).
- `Set(S)` ↔ `Set(C)` are compatible (both string-typed sets).
- `Table([])` (empty column list) is compatible with any `Table(cols)`.
- `Map(k1,v1)` ↔ `Map(k2,v2)` checked recursively.

### Validated Error Codes

| Code | Condition |
|---|---|
| `UndefinedVariable` | Name used before declaration |
| `RedefinedVariable` | Name declared twice in the same scope |
| `ConstReassignment` | Assignment to a `const` variable |
| `TypeMismatch` | Expression type does not match expected type |
| `InvalidBinaryOp` | Operator used with incompatible types |
| `BreakOutsideLoop` | `break` outside `while`/`for` |
| `ContinueOutsideLoop` | `continue` outside `while`/`for` |
| `S208 YieldOutsideFiber` | `yield` used outside any fiber body |
| `S209 FiberTypeMismatch` | `yield expr;` inside a void fiber (should be `yield;`) |
| `S210 ReturnTypeMismatchInFiber` | Bare `return;` in a typed fiber |
| `S301 WherePredicateNameCollision` | Local variable name conflicts with a table column name in `.where()` |

### Special Checking: Table `.where()`

When checking a `.where(predicate)` call on a `Table(cols)`:
1. A temporary scope is opened.
2. `__row_tmp` is defined with type `Table(cols)` — used by `is_table_lambda` to resolve bare column names.
3. The checker scans all `Identifier` nodes in the predicate expression. If any identifier name matches both an existing outer-scope variable **and** a column name, `S301 WherePredicateNameCollision` is raised.
4. The predicate must evaluate to `Bool`.

## Error Reporting

`TypeError` values carry a `Span { line, col, len }`. After checking, `main.rs` passes each error to `Reporter::error()`, which prints the source line with a visual underline. Compilation halts immediately — no bytecode is generated.