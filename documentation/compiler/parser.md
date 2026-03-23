# XCX Parser

The XCX Parser transforms the token stream into a high-level Abstract Syntax Tree (AST).

## Architecture: Pratt Parsing

XCX uses a **Pratt Parser** (Top-Down Operator Precedence).

- **File**: `src/parser/pratt.rs`
- **Lookahead**: One token (`current` + `peek`), advanced manually with `advance()`.
- **Error Recovery**: On syntax error, `synchronize()` skips tokens until the next semicolon or a known statement-starting keyword (`func`, `fiber`, `if`, `for`, etc.).

The `Parser` struct borrows the source string for the lifetime `'a`, and `Scanner<'a>` is parameterised by the same lifetime, reflecting the byte-slice-based scanner.

### Precedence Levels (lowest → highest)

| Level | Operators |
|---|---|
| `Lowest` | — |
| `Lambda` | `->` |
| `Assignment` | `=` |
| `LogicalOr` | `OR`, `\|\|` |
| `LogicalAnd` | `AND`, `&&` |
| `Equals` | `==`, `!=` |
| `LessGreater` | `>`, `<`, `>=`, `<=`, `HAS` |
| `Sum` | `+`, `-`, `++` |
| `SetOp` | `UNION`, `INTERSECTION`, `DIFFERENCE`, `SYMMETRIC_DIFFERENCE` |
| `Product` | `*`, `/`, `%` |
| `Power` | `^` |
| `Prefix` | `-x` |
| `Call` | `.`, `[` |

## Function Definition Styles

XCX supports two syntactically different styles for defining functions:

**Brace style** (C-like):
```xcx
func name(i: x, s: y -> i) {
    return x + 1;
}
```

**XCX style** (keyword block):
```xcx
func:i: name(i: x, s: y) do;
    return x + 1;
end;
```

Both produce identical `StmtKind::FunctionDef` AST nodes.

## Key Constructs Parsed

- **Variable declarations**: `i: name = expr;`, `const s: NAME = expr;`, `var name = expr;`
- **Control flow**: `if/elseif/else/end`, `while/do/end`, `for x in coll do/end`
- **Functions**: `func` (two styles, see above)
- **Fibers**: `fiber name(params) { body }` and `fiber:T: varname = fiberName(args);`
- **Yield**: `yield expr;`, `yield from expr;`, `yield;`
- **HTTP**: `serve: name { port=..., routes=... };`, `net.get(url)`, `net.request { ... } as resp;`
- **Collections**: Array `[a, b, c]`, Set `set:N { 1,,10 }`, Map `[k :: v, ...]`, Table `table { columns=[...] rows=[...] }`
- **Raw blocks**: `<<<...>>>` for inline JSON/strings
- **Include**: `include "path";` or `include "path" as alias;`
- **I/O**: `>! expr;` (print), `>? varname;` (input)
- **Halt**: `halt.alert >! msg;`, `halt.error >! msg;`, `halt.fatal >! msg;`
- **Wait**: `@wait(ms);`

## Expander (`src/parser/expander.rs`)

The Expander runs **after** parsing, **before** semantic analysis. It is a separate tree-rewriting pass, not part of the Pratt parser itself.

### Responsibilities

**Include resolution**: `include "file.xcx";` is replaced by the inlined AST of that file. Circular dependencies are detected and rejected. Files are deduplicated (each path included only once unless aliased).

**Alias prefixing**: `include "math.xcx" as math;` causes all top-level names from that file to be renamed to `math.name`. Call sites (`math.sin(x)`) are rewritten to `FunctionCall { name: "math.sin" }`. This is implemented via `prefix_program()` which walks the entire sub-AST.

**Include path search order**:
1. Relative to the current file's directory
2. In the `lib/` directory (relative to CWD or executable path)

## AST Definitions (`src/parser/ast.rs`)

### `Expr` — Expression nodes

Key variants:

| Variant | Description |
|---|---|
| `IntLiteral(i64)` | Integer constant |
| `FloatLiteral(f64)` | Float constant |
| `StringLiteral(StringId)` | Interned string |
| `BoolLiteral(bool)` | `true` / `false` |
| `Identifier(StringId)` | Variable or function name |
| `Binary { left, op, right }` | Binary operation |
| `Unary { op, right }` | Unary operation |
| `FunctionCall { name, args }` | Function call by interned name |
| `MethodCall { receiver, method, args }` | Dot-call on a value |
| `MemberAccess { receiver, member }` | Dot-access without call |
| `Index { receiver, index }` | Bracket index `a[i]` |
| `Lambda { params, return_type, body }` | Arrow lambda `x -> expr` |
| `ArrayLiteral`, `SetLiteral`, `MapLiteral`, `TableLiteral` | Collection literals |
| `DateLiteral { date_string, format }` | `date("2024-01-01")` |
| `NetCall`, `NetRespond` | HTTP expression nodes |
| `RawBlock(StringId)` | `<<<...>>>` raw content |
| `TerminalCommand(cmd, arg)` | `.terminal !cmd` |

### `Stmt` — Statement nodes

Key variants: `VarDecl`, `Assign`, `Print`, `Input`, `If`, `While`, `For`, `Break`, `Continue`, `FunctionDef`, `FiberDef`, `FiberDecl`, `Return`, `Yield`, `YieldFrom`, `YieldVoid`, `Include`, `Serve`, `NetRequestStmt`, `JsonBind`, `JsonInject`, `Halt`, `Wait`.

### `Type` — Type system

`Int`, `Float`, `String`, `Bool`, `Date`, `Json`, `Array(Box<Type>)`, `Set(SetType)`, `Map(Box<Type>, Box<Type>)`, `Table(Vec<ColumnDef>)`, `Fiber(Option<Box<Type>>)`, `Builtin(StringId)`, `Unknown`.

## String Interner

All string values (identifiers, string literals, method names) are interned via `Interner` into `StringId (u32)`. The interner is created in the parser and passed through all subsequent phases. This means the checker, compiler, and VM all use numeric IDs for name comparisons instead of `String` comparisons.