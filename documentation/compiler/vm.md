# XCX Virtual Machine (VM)

The XCX VM is a custom, stack-based runtime for executing XCX bytecode.

## Architecture

- **File**: `src/backend/vm.rs`
- **Execution Model**: Fetch-Decode-Execute loop (`execute_bytecode`)
- **Value Stack**: A dynamic `Vec<Value>` — grows on demand, no fixed size limit
- **Locals**: Each function/fiber frame has its own `Vec<Value>` indexed by slot number
- **Globals**: A single flat `Vec<Value>` shared across all frames, indexed by pre-assigned slot numbers

### VM State

```rust
struct VM {
    stack:       Vec<Value>,                          // operand stack
    globals:     Vec<Value>,                          // global variable slots
    error_count: usize,                               // runtime error counter
    call_depth:  usize,                               // recursion guard (max 800)
    fiber_yielded: bool,                              // set by Yield opcode
    servers:     HashMap<String, Rc<tiny_http::Server>>, // active HTTP servers
}
```

## Value Types

```rust
enum Value {
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
    Array(Rc<RefCell<Vec<Value>>>),
    Set(Rc<RefCell<BTreeSet<Value>>>),
    Map(Rc<RefCell<Vec<(Value, Value)>>>),
    Date(NaiveDateTime),
    Table(Rc<RefCell<TableData>>),
    Row(Rc<RefCell<TableData>>, usize),   // reference into a table row
    Json(Rc<RefCell<serde_json::Value>>),
    Fiber(Rc<RefCell<FiberState>>),
    Function(usize),                      // index into functions slice
}
```

Collections (`Array`, `Set`, `Map`, `Table`, `Json`, `Fiber`) use `Rc<RefCell<T>>` — reference counted, no GC. `Value::Row` is a lightweight reference into a `Table` row (used in `.where()` predicates and join lambdas).

## Instruction Set (OpCodes)

### Stack / Variables
| OpCode | Description |
|---|---|
| `Constant(usize)` | Push `constants[idx]` onto stack |
| `GetVar(usize)` | Push `globals[idx]` |
| `SetVar(usize)` | Pop → `globals[idx]` |
| `GetLocal(usize)` | Push `locals[idx]` |
| `SetLocal(usize)` | Pop → `locals[idx]` |
| `Pop` | Discard top of stack |

### Arithmetic & Logic
`Add`, `Sub`, `Mul`, `Div`, `Mod`, `Pow`, `IntConcat` (`++`), `Equal`, `NotEqual`, `Greater`, `Less`, `GreaterEqual`, `LessEqual`, `And`, `Or`, `Not`, `Has`

### Control Flow
| OpCode | Description |
|---|---|
| `Jump(usize)` | Unconditional jump to instruction index |
| `JumpIfFalse(usize)` | Jump if top of stack is `Bool(false)` |
| `JumpIfTrue(usize)` | Jump if top of stack is `Bool(true)` |
| `Call(func_id, arg_count)` | Call function, push return value |
| `Return` | Return top of stack from current frame |
| `ReturnVoid` | Return `None` from current frame |
| `Halt` | Stop execution |

### Collections
| OpCode | Description |
|---|---|
| `ArrayInit(n)` | Pop n values → push `Array` |
| `SetInit(n)` | Pop n values → push `Set` |
| `SetRange` | Pop start/end/step/flag → push ranged `Set` |
| `MapInit(n)` | Pop n key-value pairs → push `Map` |
| `TableInit(const_id, row_count)` | Build `Table` from skeleton constant + row values |

### Set Operations
`SetUnion`, `SetIntersection`, `SetDifference`, `SetSymDifference`

### Fiber Operations
| OpCode | Description |
|---|---|
| `FiberCreate(func_id, arg_count)` | Instantiate a `FiberState`, push `Fiber` value |
| `FiberNext` | Resume fiber, expect a yielded value |
| `FiberRun` | Resume void fiber to next `yield;` |
| `FiberIsDone` | Resume fiber to check if it yields or finishes |
| `FiberClose` | Mark fiber as done |
| `Yield` | Suspend fiber, return top of stack to caller |
| `YieldVoid` | Suspend void fiber |

### I/O & System
| OpCode | Description |
|---|---|
| `Print` | Pop and print to stdout |
| `Input` | Read line from stdin, push parsed value |
| `Wait` | Pop Int(ms), sleep synchronously |
| `HaltAlert` | Print warning, continue |
| `HaltError` | Print error, halt frame |
| `HaltFatal` | Print fatal error, halt frame |
| `TerminalExit` | Halt VM |
| `TerminalClear` | Clear terminal screen |
| `TerminalRun` | Run another `.xcx` file via subprocess |
| `EnvGet` | Pop var name, push env variable value |
| `EnvArgs` | Push `Array<String>` of CLI arguments |

### HTTP
| OpCode | Description |
|---|---|
| `HttpCall(method_idx)` | Simple HTTP call (GET/POST/etc.), push JSON response |
| `HttpRequest` | HTTP call from config map, push JSON response |
| `HttpRespond` | Build response JSON object (status, body, headers) |
| `HttpServe(name_idx)` | Start HTTP server, enter blocking dispatch loop |

### Storage
`StoreRead`, `StoreWrite`, `StoreAppend`, `StoreExists`, `StoreDelete`

### Type Casting
`CastInt`, `CastFloat`, `CastString`, `CastBool`

### Crypto & Dates
`CryptoHash`, `CryptoVerify`, `CryptoToken`, `DateNow`

### JSON
`JsonParse`, `JsonBind(global_idx)`, `JsonBindLocal(local_idx)`, `JsonInject(global_idx)`, `JsonInjectLocal(local_idx)`

### Method Dispatch
`MethodCall(method_name_idx, arg_count)` — pops receiver + args, dispatches to type-specific handler (`handle_array_method`, `handle_table_method`, `handle_json_method`, etc.)

## Execution Flow

```
VM::run(bytecode, ctx)
  └─ Executor::execute_bytecode(bytecode, &mut ip, &mut locals)
       └─ loop: execute_step(op, ip, locals) → OpResult
            ├─ Continue      → ip stays, advance normally
            ├─ Jump(t)       → ip = t
            ├─ Return(val)   → exit frame, return val
            ├─ Yield(val)    → suspend (fiber), return val
            └─ Halt          → stop execution
```

`Executor` holds references to both `VM` (mutable state) and `VMContext` (immutable constants/functions). Functions are called via `run_frame(func_id, params)`, which creates a fresh `locals` vector.

## Fiber Execution Model

Fibers are **cooperative coroutines**, not threads.

```rust
struct FiberState {
    func_id:       usize,        // which function chunk to execute
    ip:            usize,        // suspended instruction pointer
    locals:        Vec<Value>,   // suspended local variables
    stack:         Vec<Value>,   // suspended operand stack
    is_done:       bool,
    yielded_value: Option<Value>, // cached value for FiberIsDone + FiberNext pattern
}
```

### Resume Sequence (`resume_fiber`)
1. Save current VM stack, swap in fiber's stack.
2. Run `execute_bytecode` starting from `fiber.ip` with `fiber.locals`.
3. If `Yield` is reached: `fiber_yielded = true`, save state back, return yielded value.
4. If `Return` / end of bytecode: `fiber.is_done = true`, return final value.

### For-loop over Fiber (`ForIterType::Fiber`)
The compiled loop uses `FiberIsDone` to pre-fetch the next value, caches it in `yielded_value`, then `FiberNext` retrieves it. This two-step pattern avoids consuming a value during the loop condition check.

## Compiler (`src/backend/mod.rs`)

### Two-Pass Compilation

**Pass 1** — `register_globals_recursive`:
- Assigns a slot index to every global variable (`globals: HashMap<StringId, usize>`)
- Assigns a function index to every function/fiber (`func_indices: HashMap<StringId, usize>`)
- Pre-allocates empty `FunctionChunk` slots in `functions: Vec<FunctionChunk>`

**Pass 2** — `FunctionCompiler::compile_stmt` / `compile_expr`:
- Emits bytecode into `Vec<OpCode>`
- Locals are tracked in a scope stack (`scopes: Vec<HashMap<StringId, usize>>`)
- Top-level statements in `main` use `SetVar`/`GetVar` (globals); nested statements use `SetLocal`/`GetLocal`

### `FunctionChunk`
```rust
struct FunctionChunk {
    bytecode:   Vec<OpCode>,
    is_fiber:   bool,
    max_locals: usize,   // used to pre-size locals Vec on call
}
```

## Memory Model

- **No garbage collector**. `Rc<RefCell<T>>` provides shared ownership with runtime borrow checking.
- **Value cloning**: `Value::clone()` on collections clones the `Rc` pointer (cheap), not the data.
- **Tables and Arrays**: Mutations via `.insert()`, `.update()`, `.delete()` operate on the shared `RefCell` directly — all references to the same collection see the change.

## Security Controls

### File System Sandbox (`is_safe_path`)
- Rejects absolute paths
- Rejects any path containing `..` (parent directory traversal)
- All `store.*` operations pass through this check

### Network SSRF Protection (`is_safe_url`)
Blocked targets:
- `file://` URLs
- `169.254.x.x` (link-local / AWS metadata)
- Private ranges: `10.x`, `192.168.x`, `172.16–31.x` (when not localhost)

### Stack Overflow Guard
`MAX_CALL_DEPTH = 800`. Exceeded call depth returns `OpResult::Halt` immediately.

### HTTP Body Limit
Incoming request bodies are capped at **10 MB**. Oversized payloads receive a `413` response without executing the handler.