# XCX Virtual Machine (VM)

The XCX VM is a custom, stack-based runtime for executing XCX bytecode.

## Architecture

- **File**: `src/backend/vm.rs`
- **Execution Model**: Fetch-Decode-Execute loop (`execute_bytecode`)
- **Value Stack**: A `Vec<Value>` owned by `Executor` — grows on demand, shared across the call stack within a single thread
- **Locals**: Each function/fiber frame has its own `Vec<Value>` indexed by slot number
- **Globals**: A single flat `Vec<Value>` behind `Arc<RwLock<Vec<Value>>>`, shared across all worker threads

### VM State

```rust
pub struct VM {
    pub globals:     Arc<RwLock<Vec<Value>>>,
    pub error_count: AtomicUsize,
    pub servers:     Arc<RwLock<HashMap<String, Arc<tiny_http::Server>>>>,
}
```

`VM` is wrapped in `Arc<VM>` and shared across HTTP worker threads. `Executor` holds an `Arc<VM>` clone and its own private stack — workers do not share stacks.

### Executor State

```rust
struct Executor {
    vm:            Arc<VM>,
    ctx:           SharedContext,         // Arc<Vec<Value>> + Arc<Vec<FunctionChunk>>
    current_spans: Option<Arc<Vec<Span>>>, // span table for current function
    stack:         Vec<Value>,
    call_depth:    usize,
    fiber_yielded: bool,
}
```

### SharedContext

```rust
pub struct SharedContext {
    pub constants: Arc<Vec<Value>>,
    pub functions: Arc<Vec<FunctionChunk>>,
}
```

`SharedContext` is cheaply cloned (two `Arc` pointer bumps) and passed to each worker thread independently.

## Value Types

```rust
pub enum Value {
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
    Array(Arc<RwLock<Vec<Value>>>),
    Set(Arc<RwLock<BTreeSet<Value>>>),
    Map(Arc<RwLock<Vec<(Value, Value)>>>),
    Date(NaiveDateTime),
    Table(Arc<RwLock<TableData>>),
    Row(Arc<RwLock<TableData>>, usize),   // lightweight reference into a table row
    Json(Arc<RwLock<serde_json::Value>>),
    Fiber(Arc<RwLock<FiberState>>),
    Function(usize),                      // index into functions slice
}
```

Collections use `Arc<RwLock<T>>` (via `parking_lot`). `Value` implements `Send + Sync`. `Value::Row` is a lightweight reference into a `Table` row, used in `.where()` predicates and join lambdas. Cloning a collection value clones only the `Arc` pointer — not the underlying data.

## Instruction Set (OpCodes)

### Stack / Variables
| OpCode | Description |
|---|---|
| `Constant(usize)` | Push `constants[idx]` onto stack |
| `GetVar(usize)` | Push `globals[idx]` (acquires read lock) |
| `SetVar(usize)` | Pop → `globals[idx]` (acquires write lock) |
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
| `Yield` | Suspend fiber, return top of stack to caller |
| `YieldVoid` | Suspend void fiber |

### Method Dispatch
| OpCode | Description |
|---|---|
| `MethodCall(MethodKind, arg_count)` | Dispatch built-in method by enum variant — no string lookup |
| `MethodCallCustom(name_idx, arg_count)` | Dispatch dynamic method (JSON field, alias) by string from constants |

`MethodKind` is a `#[derive(Copy)]` enum with ~50 variants covering all built-in collection, string, date, fiber, and JSON methods. Mapping from method name string to `MethodKind` happens once in the compiler via `map_method_kind()`. At runtime, dispatch is a `match` on a `Copy` integer — zero allocation, zero string comparison.

### I/O & System
| OpCode | Description |
|---|---|
| `Print` | Pop and print to stdout |
| `Input` | Read line from stdin, push parsed value |
| `Wait` | Pop Int(ms), sleep synchronously |
| `HaltAlert` | Print warning + span info, continue |
| `HaltError` | Print error + span info, halt frame |
| `HaltFatal` | Print fatal error + span info, halt frame |
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
| `HttpServe(name_idx)` | Start HTTP server, spawn worker threads, block main thread |

### Storage
`StoreRead`, `StoreWrite`, `StoreAppend`, `StoreExists`, `StoreDelete`

### Type Casting
`CastInt`, `CastFloat`, `CastString`, `CastBool`

### Crypto & Dates
`CryptoHash`, `CryptoVerify`, `CryptoToken`, `DateNow`

### JSON
`JsonParse`, `JsonBind(global_idx)`, `JsonBindLocal(local_idx)`, `JsonInject(global_idx)`, `JsonInjectLocal(local_idx)`

## Execution Flow

```
VM::run(main_chunk, ctx)                   [Arc<VM>]
  └─ Executor::run_frame_owned(main_chunk)
       └─ execute_bytecode(bytecode, &mut ip, &mut locals)
            └─ loop: execute_step(op, ip, locals) → OpResult
                 ├─ Continue      → advance normally
                 ├─ Jump(t)       → ip = t
                 ├─ Return(val)   → exit frame, return val
                 ├─ Yield(val)    → suspend (fiber), return val
                 └─ Halt          → stop, increment error_count
```

Functions are called via `run_frame(func_id, params)`, which creates a fresh `locals` vector pre-sized to `chunk.max_locals`. `current_spans` is swapped to the called function's span table and restored on return.

## Runtime Error Reporting

Every `eprintln!` in the VM appends `self.current_span_info(ip)` which returns `" [line: X, col: Y]"` by looking up `current_spans[ip - 1]`. This produces messages like:

```
R303: Array index out of bounds: 5 [line: 14, col: 7]
```

`current_spans` is updated to the correct `Arc<Vec<Span>>` on every `run_frame` call and restored on exit, so nested calls always report the correct source location.

## Fiber Execution Model

Fibers are **cooperative coroutines**, not threads.

```rust
pub struct FiberState {
    pub func_id:       usize,
    pub ip:            usize,
    pub locals:        Vec<Value>,        // moved out during resume, moved back after
    pub stack:         Vec<Value>,        // segment appended to VM stack during resume
    pub is_done:       bool,
    pub yielded_value: Option<Value>,     // cached value for IsDone + Next pattern
}
```

### Resume Sequence (`resume_fiber`)

1. **Move** `fiber.locals` and `fiber.stack` out of `FiberState` via `std::mem::take` — no clone.
2. Extend the VM's own stack with the fiber's saved stack segment.
3. Run `execute_bytecode` from `fiber.ip` with the moved locals.
4. On `Yield`: set `fiber_yielded = true`. Split the fiber's stack segment back off the VM stack. Move locals and stack back into `FiberState`. Return yielded value.
5. On `Return` / bytecode end: set `fiber.is_done = true`. Return final value.

This design means fiber resume/suspend involves no heap allocation — only moves and a `Vec::split_off`.

### For-loop over Fiber (`ForIterType::Fiber`)

The compiled loop calls `MethodCall(MethodKind::IsDone, 0)` to check completion. `IsDone` internally calls `resume_fiber` and caches the result in `yielded_value`. The subsequent `MethodCall(MethodKind::Next, 0)` takes the cached value without re-executing the fiber. `break` inside a fiber loop emits `MethodCall(MethodKind::Close, 0)` to mark the fiber done before jumping.

## HTTP Server (`HttpServe`)

`HttpServe` starts a `tiny_http::Server` and spawns `workers` OS threads (taken from the `workers` field of the `serve:` block):

```rust
for i in 0..workers {
    let server_clone = server.clone();    // Arc<tiny_http::Server>
    let vm_clone     = self.vm.clone();   // Arc<VM>
    let ctx_clone    = self.ctx.clone();  // Arc<Vec<...>> × 2
    let routes_clone = routes.clone();    // Value (Arc inside)
    std::thread::spawn(move || { ... });
}
```

Each worker runs its own `Executor` with its own stack. All workers share globals via `Arc<RwLock<Vec<Value>>>`. Each worker polls `SHUTDOWN.load(Ordering::SeqCst)` and exits when it is set to `true`.

### Graceful Shutdown

`SHUTDOWN` is a `pub static AtomicBool` in `vm.rs`. A Ctrl+C handler registered in `main.rs` via the `ctrlc` crate sets it to `true`. Workers check it once per request loop iteration (every `recv_timeout(100ms)` cycle). After all worker threads join, `HttpServe` returns `OpResult::Halt`.

## Compiler (`src/backend/mod.rs`)

### Constant Deduplication

`CompileContext` carries `string_constants: &mut HashMap<String, usize>`. `add_constant` checks this map before inserting a new `Value::String` into the constants table:

```rust
pub fn add_constant(&mut self, val: Value) -> usize {
    if let Value::String(ref s) = val {
        if let Some(&idx) = self.string_constants.get(s) {
            return idx;   // reuse existing constant
        }
        let idx = self.constants.len();
        self.string_constants.insert(s.clone(), idx);
        self.constants.push(val);
        return idx;
    }
    self.constants.push(val);
    self.constants.len() - 1
}
```

For programs with many method calls, this eliminates hundreds of duplicate `"size"`, `"get"`, `"insert"` entries from the constants table.

### `FunctionChunk`

```rust
pub struct FunctionChunk {
    pub bytecode:   Arc<Vec<OpCode>>,
    pub spans:      Arc<Vec<Span>>,    // spans[i] corresponds to bytecode[i]
    pub is_fiber:   bool,
    pub max_locals: usize,
}
```

Bytecode and spans are wrapped in `Arc` so they can be shared between the `SharedContext` and individual worker thread executors without copying. The main compiler result is returned as `(FunctionChunk, Arc<Vec<Value>>, Arc<Vec<FunctionChunk>>)`.

### Two-Pass Compilation

**Pass 1** — `register_globals_recursive`:
- Assigns a slot index to every global variable (`globals: HashMap<StringId, usize>`)
- Assigns a function index to every function/fiber (`func_indices: HashMap<StringId, usize>`)
- Pre-allocates empty `FunctionChunk` slots in `functions: Vec<FunctionChunk>`

**Pass 2** — `FunctionCompiler::compile_stmt` / `compile_expr`:
- Emits bytecode via `self.emit(op, span)` — each opcode is paired with its AST span
- Locals are tracked in a scope stack (`scopes: Vec<HashMap<StringId, usize>>`)
- Top-level statements in `main` use `SetVar`/`GetVar` (globals); nested statements use `SetLocal`/`GetLocal`

### `FunctionCompiler::emit`

All opcode emission goes through `emit(op, span)` which pushes to both `bytecode` and `spans` simultaneously, keeping the two vectors in sync:

```rust
fn emit(&mut self, op: OpCode, span: &Span) {
    self.bytecode.push(op);
    self.spans.push(span.clone());
}
```

## Memory Model

- **No garbage collector**. `Arc<RwLock<T>>` provides shared ownership with runtime borrow checking (via `parking_lot`).
- **Value cloning**: `Value::clone()` on collections clones the `Arc` pointer (cheap), not the underlying data.
- **Tables and Arrays**: Mutations via `.insert()`, `.update()`, `.delete()` acquire a write lock on the shared `RwLock` — all handles to the same collection see the change.
- **Read locks**: All read-only method calls (`.size()`, `.get()`, `.contains()`) acquire a read lock — multiple concurrent readers are allowed.

## Security Controls

### File System Sandbox (`is_safe_path`)
- Rejects absolute paths
- Rejects any path containing `..` (parent directory traversal)
- All `store.*` operations pass through this check

### Network SSRF Protection (`is_safe_url`)
Blocked targets:
- `file://` URLs
- `169.254.x.x` (link-local / AWS metadata endpoint)
- Private ranges: `10.x`, `192.168.x`, `172.16–31.x` (when not localhost)

### Stack Overflow Guard
`MAX_CALL_DEPTH = 800`. Exceeded call depth returns `OpResult::Halt` immediately, incrementing `error_count`.

### HTTP Body Limit
Incoming request bodies are capped at **10 MB**. Oversized payloads receive a `413` response without invoking the handler.