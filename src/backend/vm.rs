use std::sync::Arc;
use parking_lot::RwLock;
use std::sync::atomic::{AtomicBool, Ordering};
pub static SHUTDOWN: AtomicBool = AtomicBool::new(false);

use argon2::password_hash::{PasswordHasher, PasswordVerifier, PasswordHash, SaltString};
use rand::Rng;




#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MethodKind {
    Push, Pop, Len, Count, Size, IsEmpty, Clear, Contains, Get, Insert, Update, Delete, Find, Join, Show, Sort, Reverse,
    Add, Remove, Has, Length, Upper, Lower, Trim, IndexOf, LastIndexOf, Replace, Slice, Split, StartsWith, EndsWith,
    ToInt, ToFloat, Set, Keys, Values, Where, Year, Month, Day, Hour, Minute, Second, Format, Exists, Append, Inject, ToStr,
    Next, Run, IsDone, Close,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OpCode {
    Constant(usize),
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    Equal,
    NotEqual,
    Greater,
    Less,
    GreaterEqual,
    LessEqual,
    Print,
    GetVar(usize),
    SetVar(usize),
    GetLocal(usize),
    SetLocal(usize),
    Pop,
    Jump(usize),
    JumpIfFalse(usize),
    JumpIfTrue(usize),
    And,
    Or,
    Not,
    Has,
    Input,
    HaltAlert,
    HaltError,
    HaltFatal,
    TerminalExit,
    TerminalClear,
    Call(usize, usize),
    Return,
    ReturnVoid,
    Halt,
    ArrayInit(usize),
    MethodCall(MethodKind, usize),
    MethodCallCustom(usize, usize),
    SetInit(usize),
    SetUnion,
    SetIntersection,
    SetDifference,
    SetSymDifference,
    RandomChoice,
    IntConcat,
    SetRange,
    MapInit(usize),
    StoreWrite,
    StoreRead,
    StoreAppend,
    StoreExists,
    StoreDelete,
    TableInit(usize, usize),
    JsonParse,
    DateNow,
    JsonBind(usize),
    JsonBindLocal(usize),
    JsonInject(usize),
    JsonInjectLocal(usize),
    FiberCreate(usize, usize),
    Yield,
    YieldVoid,
    HttpCall(usize), // method index
    HttpRequest,     // pop map
    HttpRespond,     // pop status, body, [headers]
    HttpServe(usize), // pop port, [host], [workers], routes; name index
    Wait,             // pop Int(ms), sleep synchronously
    EnvGet,           // pop String(var_name), push String(value) or halt
    CryptoHash,       // pop password, algo
    CryptoVerify,     // pop password, hash, algo
    CryptoToken,      // pop length
    CastInt,
    CastFloat,
    CastString,
    CastBool,
    EnvArgs,
    TerminalRun,
}

#[derive(Debug, Clone)]
pub enum Value {
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
    Array(Arc<RwLock<Vec<Value>>>),
    Set(Arc<RwLock<std::collections::BTreeSet<Value>>>),
    Map(Arc<RwLock<Vec<(Value, Value)>>>),
    Date(chrono::NaiveDateTime),
    Table(Arc<RwLock<TableData>>),
    Function(usize),
    Row(Arc<RwLock<TableData>>, usize),
    Json(Arc<RwLock<serde_json::Value>>),
    Fiber(Arc<RwLock<FiberState>>),
}

impl Eq for Value {}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a.to_bits() == b.to_bits(),
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Array(a), Value::Array(b)) => Arc::ptr_eq(a, b) || *a.read() == *b.read(),
            (Value::Set(a), Value::Set(b)) => Arc::ptr_eq(a, b) || *a.read() == *b.read(),
            (Value::Map(a), Value::Map(b)) => Arc::ptr_eq(a, b) || *a.read() == *b.read(),
            (Value::Date(a), Value::Date(b)) => a == b,
            (Value::Table(a), Value::Table(b)) => Arc::ptr_eq(a, b),
            (Value::Function(a), Value::Function(b)) => a == b,
            (Value::Row(a, ai), Value::Row(b, bi)) => Arc::ptr_eq(a, b) && ai == bi,
            (Value::Json(a), Value::Json(b)) => Arc::ptr_eq(a, b) || *a.read() == *b.read(),
            (Value::Fiber(a), Value::Fiber(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl Ord for Value {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => a.cmp(b),
            (Value::Float(a), Value::Float(b)) => a.total_cmp(b),
            (Value::Int(a), Value::Float(b)) => (*a as f64).total_cmp(b),
            (Value::Float(a), Value::Int(b)) => a.total_cmp(&(*b as f64)),
            (Value::String(a), Value::String(b)) => a.cmp(b),
            (Value::Bool(a), Value::Bool(b)) => a.cmp(b),
            (Value::Date(a), Value::Date(b)) => a.cmp(b),
            (Value::Function(a), Value::Function(b)) => a.cmp(b),
            (a, b) if a.is_numeric() && b.is_numeric() => std::cmp::Ordering::Equal,
            (a, b) if a.variant_rank() != b.variant_rank() => a.variant_rank().cmp(&b.variant_rank()),
            (Value::Array(a), Value::Array(b)) => {
                if Arc::ptr_eq(a, b) { std::cmp::Ordering::Equal }
                else { a.read().cmp(&b.read()) }
            }
            (Value::Set(a), Value::Set(b)) => {
                if Arc::ptr_eq(a, b) { std::cmp::Ordering::Equal }
                else { a.read().cmp(&b.read()) }
            }
            (Value::Table(a), Value::Table(b)) => (Arc::as_ptr(a) as usize).cmp(&(Arc::as_ptr(b) as usize)),
            (Value::Row(a, ai), Value::Row(b, bi)) => {
                if Arc::ptr_eq(a, b) { ai.cmp(bi) }
                else { (Arc::as_ptr(a) as usize).cmp(&(Arc::as_ptr(b) as usize)) }
            }
            (Value::Json(a), Value::Json(b)) => a.read().to_string().cmp(&b.read().to_string()),
            (Value::Fiber(a), Value::Fiber(b)) => (Arc::as_ptr(a) as usize).cmp(&(Arc::as_ptr(b) as usize)),
            (Value::Map(a), Value::Map(b)) => {
                if Arc::ptr_eq(a, b) { std::cmp::Ordering::Equal }
                else {
                    let am = a.read();
                    let bm = b.read();
                    am.len().cmp(&bm.len()).then_with(|| {
                        for (ai, bi) in am.iter().zip(bm.iter()) {
                            let c = ai.0.cmp(&bi.0).then_with(|| ai.1.cmp(&bi.1));
                            if c != std::cmp::Ordering::Equal { return c; }
                        }
                        std::cmp::Ordering::Equal
                    })
                }
            }
            _ => std::cmp::Ordering::Equal,
        }
    }
}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Value {
    fn is_numeric(&self) -> bool {
        matches!(self, Value::Int(_) | Value::Float(_))
    }

    fn variant_rank(&self) -> usize {
        match self {
            Value::Int(_) => 0,
            Value::Float(_) => 1,
            Value::String(_) => 2,
            Value::Bool(_) => 3,
            Value::Date(_) => 4,
            Value::Array(_) => 5,
            Value::Set(_) => 6,
            Value::Map(_) => 7,
            Value::Table(_) => 8,
            Value::Function(_) => 9,
            Value::Row(_, _) => 10,
            Value::Json(_) => 11,
            Value::Fiber(_) => 12,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct VMColumn {
    pub name: String,
    pub ty: crate::parser::ast::Type,
    pub is_auto: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TableData {
    pub columns: Vec<VMColumn>,
    pub rows: Vec<Vec<Value>>,
}

#[derive(Debug, Clone)]
pub struct FiberState {
    pub func_id: usize,
    pub ip: usize,
    pub locals: Vec<Value>,
    pub stack: Vec<Value>,
    pub is_done: bool,
    pub yielded_value: Option<Value>,
}

impl PartialEq for FiberState {
    fn eq(&self, other: &Self) -> bool { std::ptr::eq(self, other) }
}
impl Eq for FiberState {}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Int(v) => write!(f, "{}", v),
            Value::Float(v) => write!(f, "{}", v),
            Value::String(v) => write!(f, "{}", v),
            Value::Bool(v) => write!(f, "{}", v),
            Value::Array(arr) => {
                let b = arr.read();
                write!(f, "[")?;
                for (i, val) in b.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", val)?;
                }
                write!(f, "]")
            }
            Value::Set(s) => {
                let b = s.read();
                write!(f, "{{")?;
                for (i, val) in b.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", val)?;
                }
                write!(f, "}}")
            }
            Value::Map(m) => {
                let b = m.read();
                write!(f, "{{")?;
                for (i, (k, v)) in b.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{} :: {}", k, v)?;
                }
                write!(f, "}}")
            }
            Value::Date(d) => write!(f, "{}", d.format("%Y-%m-%d")),
            Value::Table(t) => write!(f, "Table(rows: {})", t.read().rows.len()),
            Value::Function(id) => write!(f, "Function({})", id),
            Value::Row(_, idx) => write!(f, "Row({})", idx),
            Value::Json(v) => write!(f, "Json({})", v.read()),
            Value::Fiber(fiber_rc) => {
                let fiber = fiber_rc.read();
                if fiber.is_done { write!(f, "Fiber(done)") }
                else { write!(f, "Fiber(ip={})", fiber.ip) }
            }
        }
    }
}

#[derive(Clone)]
pub struct FunctionChunk {
    pub bytecode: Arc<Vec<OpCode>>,
    pub spans: Arc<Vec<crate::lexer::token::Span>>,
    pub is_fiber: bool,
    pub max_locals: usize,
}

#[derive(Clone)]
pub struct SharedContext {
    pub constants: Arc<Vec<Value>>,
    pub functions: Arc<Vec<FunctionChunk>>,
}

pub struct VM {
    pub globals: Arc<RwLock<Vec<Value>>>,
    pub error_count: std::sync::atomic::AtomicUsize,
    pub servers: Arc<RwLock<std::collections::HashMap<String, Arc<tiny_http::Server>>>>,
}

const MAX_CALL_DEPTH: usize = 800;

#[derive(Debug, Clone)]
enum OpResult {
    Continue,
    Jump(usize),
    Return(Option<Value>),
    Yield(Option<Value>),
    Halt,
}

impl VM {
    pub fn new() -> Self {
        #[cfg(windows)]
        enable_ansi_support();

        Self {
            globals: Arc::new(RwLock::new(vec![Value::Bool(false); 1024])),
            error_count: std::sync::atomic::AtomicUsize::new(0),
            servers: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    #[allow(dead_code)]
    pub fn get_global(&self, idx: usize) -> Option<Value> {
        self.globals.read().get(idx).cloned()
    }

    pub fn run(self: Arc<Self>, main_chunk: FunctionChunk, ctx: SharedContext) {
        let mut executor = Executor {
            vm: self.clone(),
            ctx,
            current_spans: None, // Will be set in run_frame
            stack: Vec::with_capacity(128),
            call_depth: 0,
            fiber_yielded: false,
        };
        executor.run_frame_owned(main_chunk);
    }
}

#[cfg(windows)]
fn enable_ansi_support() {
    use std::ptr;

    type DWORD = u32;
    type HANDLE = *mut std::ffi::c_void;
    const STD_OUTPUT_HANDLE: DWORD = -11i32 as u32;
    const ENABLE_VIRTUAL_TERMINAL_PROCESSING: DWORD = 0x0004;

    unsafe extern "system" {
        fn GetStdHandle(nStdHandle: DWORD) -> HANDLE;
        fn GetConsoleMode(hConsoleHandle: HANDLE, lpMode: *mut DWORD) -> i32;
        fn SetConsoleMode(hConsoleHandle: HANDLE, dwMode: DWORD) -> i32;
    }

    unsafe {
        let handle = GetStdHandle(STD_OUTPUT_HANDLE);
        if handle != ptr::null_mut() {
            let mut mode: DWORD = 0;
            if GetConsoleMode(handle, &mut mode) != 0 {
                SetConsoleMode(handle, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
            }
        }
    }
}

struct Executor {
    vm: Arc<VM>,
    ctx: SharedContext,
    current_spans: Option<Arc<Vec<crate::lexer::token::Span>>>,
    stack: Vec<Value>,
    call_depth: usize,
    fiber_yielded: bool,
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn json_path_to_value(root: &serde_json::Value, path: &str) -> Option<Value> {
    let pointer = normalize_json_path(path);
    let node = if pointer.is_empty() {
        root.clone()
    } else {
        root.pointer(&pointer).cloned().unwrap_or(serde_json::Value::Null)
    };
    if node.is_null() { return None; }
    Some(match node {
        serde_json::Value::Number(n) =>
            if let Some(i) = n.as_i64() { Value::Int(i) }
            else if let Some(f) = n.as_f64() { Value::Float(f) }
            else { Value::Int(0) },
        serde_json::Value::String(s) => Value::String(s),
        serde_json::Value::Bool(b) => Value::Bool(b),
        other => Value::Json(Arc::new(RwLock::new(other))),
    })
}

fn inject_json_into_table(
    table: &mut TableData,
    json: &serde_json::Value,
    mapping: &[(Value, Value)],
) {
    let items: Vec<serde_json::Value> = if let Some(arr) = json.as_array() {
        arr.clone()
    } else {
        vec![json.clone()]
    };

    for item in items {
        let mut new_row = Vec::with_capacity(table.columns.len());
        for col in &table.columns {
            let mut found = false;
            for (k, v) in mapping {
                if let (Value::String(col_match), Value::String(json_path)) = (k, v) {
                    if col_match == &col.name {
                        let pointer = normalize_json_path(json_path);
                        let raw = if pointer.is_empty() { item.clone() }
                                  else { item.pointer(&pointer).cloned().unwrap_or(serde_json::Value::Null) };
                        new_row.push(match col.ty {
                            crate::parser::ast::Type::Int =>
                                raw.as_i64().map(Value::Int).unwrap_or(Value::Int(0)),
                            crate::parser::ast::Type::Float =>
                                raw.as_f64().map(Value::Float).unwrap_or(Value::Float(0.0)),
                            crate::parser::ast::Type::Bool =>
                                raw.as_bool().map(Value::Bool).unwrap_or(Value::Bool(false)),
                            crate::parser::ast::Type::String =>
                                Value::String(raw.as_str().unwrap_or("").to_string()),
                            _ => Value::String(raw.to_string()),
                        });
                        found = true;
                        break;
                    }
                }
            }
            if !found {
                new_row.push(match col.ty {
                    crate::parser::ast::Type::Int => Value::Int(0),
                    _ => Value::String("".to_string()),
                });
            }
        }
        table.rows.push(new_row);
    }
}

fn set_op(
    a: &std::collections::BTreeSet<Value>,
    b: &std::collections::BTreeSet<Value>,
    op: u8,
) -> std::collections::BTreeSet<Value> {
    match op {
        0 => { let mut r = a.clone(); r.extend(b.iter().cloned()); r }
        1 => a.iter().filter(|x| b.contains(x)).cloned().collect(),
        2 => a.iter().filter(|x| !b.contains(x)).cloned().collect(),
        _ => a.symmetric_difference(b).cloned().collect(),
    }
}



enum JoinPred {
    Keys(String, String),
    Lambda(usize),
}

fn join_tables(
    left: &TableData,
    right: &TableData,
    pred: &JoinPred,
    right_name: &str,
    executor: &mut Executor,
) -> TableData {
    let right_key_name: Option<&str> = match pred {
        JoinPred::Keys(_, rk) => Some(rk.as_str()),
        JoinPred::Lambda(_) => None,
    };
    let left_col_names: std::collections::HashSet<&str> =
        left.columns.iter().map(|c| c.name.as_str()).collect();

    let mut out_cols: Vec<VMColumn> = left.columns.clone();
    let mut right_col_map: Vec<Option<usize>> = Vec::new();
    for (ci, col) in right.columns.iter().enumerate() {
        if right_key_name == Some(col.name.as_str()) {
            right_col_map.push(None);
            continue;
        }
        let out_name = if left_col_names.contains(col.name.as_str()) {
            format!("{}_{}", right_name, col.name)
        } else {
            col.name.clone()
        };
        right_col_map.push(Some(out_cols.len()));
        out_cols.push(VMColumn { name: out_name, ty: col.ty.clone(), is_auto: false });
        let _ = ci;
    }

    let left_rc  = Arc::new(RwLock::new(left.clone()));
    let right_rc = Arc::new(RwLock::new(right.clone()));
    let mut out_rows: Vec<Vec<Value>> = Vec::new();

    for li in 0..left.rows.len() {
        for ri in 0..right.rows.len() {
            let matches = match pred {
                JoinPred::Keys(lk, rk) => {
                    let lc = left.columns.iter().position(|c| &c.name == lk);
                    let rc = right.columns.iter().position(|c| &c.name == rk);
                    match (lc, rc) {
                        (Some(lci), Some(rci)) => left.rows[li][lci] == right.rows[ri][rci],
                        _ => false,
                    }
                }
                JoinPred::Lambda(fid) => {
                    let row_a = Value::Row(left_rc.clone(), li);
                    let row_b = Value::Row(right_rc.clone(), ri);
                    matches!(executor.run_frame(*fid, &[row_a, row_b]), Some(Value::Bool(true)))
                }
            };

            if matches {
                let mut row = vec![Value::Bool(false); out_cols.len()];
                for (ci, v) in left.rows[li].iter().enumerate() {
                    row[ci] = v.clone();
                }
                for (rci, out_idx) in right_col_map.iter().enumerate() {
                    if let Some(oi) = out_idx {
                        row[*oi] = right.rows[ri][rci].clone();
                    }
                }
                out_rows.push(row);
            }
        }
    }

    TableData { columns: out_cols, rows: out_rows }
}



impl Executor {
    fn current_span_info(&self, ip: usize) -> String {
        if let Some(spans) = &self.current_spans {
            if ip > 0 && ip <= spans.len() {
                let s = &spans[ip - 1];
                return format!(" [line: {}, col: {}]", s.line, s.col);
            }
        }
        "".to_string()
    }

    fn execute_step(
        &mut self,
        op: OpCode,
        _ip: &mut usize,
        locals: &mut Vec<Value>,
    ) -> OpResult {
        match op {
            OpCode::Constant(idx) => {
                self.stack.push(self.ctx.constants[idx].clone());
                OpResult::Continue
            }
            OpCode::GetVar(idx) => {
                let glbs = self.vm.globals.read();
                if idx >= glbs.len() {
                    self.stack.push(Value::Bool(false));
                    return OpResult::Halt;
                }
                self.stack.push(glbs[idx].clone());
                OpResult::Continue
            }
            OpCode::SetVar(idx) => {
                let val = self.stack.pop().expect("SetVar: empty stack");
                let mut glbs = self.vm.globals.write();
                if idx >= glbs.len() {
                    glbs.resize(idx + 1, Value::Bool(false));
                }
                glbs[idx] = val;
                OpResult::Continue
            }
            OpCode::GetLocal(idx) => {
                self.stack.push(locals.get(idx).cloned().unwrap_or(Value::Bool(false)));
                OpResult::Continue
            }
            OpCode::SetLocal(idx) => {
                let val = self.stack.pop().expect("SetLocal: empty stack");
                if idx >= locals.len() { locals.resize(idx + 1, Value::Bool(false)); }
                locals[idx] = val;
                OpResult::Continue
            }
            OpCode::Pop => { self.stack.pop(); OpResult::Continue }
 
            OpCode::Jump(offset) => OpResult::Jump(offset),
            OpCode::JumpIfFalse(offset) => {
                let val = self.stack.pop().unwrap_or(Value::Bool(true));
                if matches!(val, Value::Bool(false)) { OpResult::Jump(offset) } else { OpResult::Continue }
            }
            OpCode::JumpIfTrue(offset) => {
                let val = self.stack.pop().unwrap_or(Value::Bool(false));
                if matches!(val, Value::Bool(true)) { OpResult::Jump(offset) } else { OpResult::Continue }
            }
            OpCode::Return    => OpResult::Return(self.stack.pop()),
            OpCode::ReturnVoid => OpResult::Return(None),
            OpCode::Yield     => OpResult::Yield(self.stack.pop()),
            OpCode::YieldVoid => OpResult::Yield(None),
            OpCode::Halt      => OpResult::Halt,
            OpCode::TerminalExit  => OpResult::Halt,
            OpCode::TerminalClear => {
                #[cfg(windows)]
                {
                    if let Err(_) = std::process::Command::new("cmd").args(["/c", "cls"]).status() {
                        print!("\x1B[2J\x1B[1;1H");
                    }
                }
                #[cfg(not(windows))]
                {
                    print!("\x1B[2J\x1B[1;1H");
                }
                
                use std::io::Write;
                std::io::stdout().flush().unwrap();
                self.stack.push(Value::Bool(true));
                OpResult::Continue
            }
            OpCode::TerminalRun => {
                let filename_val = self.stack.pop().expect("TerminalRun: empty stack");
                let filename = match filename_val {
                    Value::String(s) => s,
                    other => other.to_string(),
                };
                
                let exe_path = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("xcx"));
                
                match std::process::Command::new(exe_path)
                    .arg(&filename)
                    .status() 
                {
                    Ok(status) => {
                        self.stack.push(Value::Bool(status.success()));
                    }
                    Err(e) => {
                        eprintln!("Failed to execute !run {}: {}{}", filename, e, self.current_span_info(*_ip));
                        self.stack.push(Value::Bool(false));
                    }
                }
                OpResult::Continue
            }
 
            OpCode::Wait => {
                let ms_val = self.stack.pop().expect("Wait: expected duration on stack");
                let ms: u64 = match ms_val {
                    Value::Int(n) if n >= 0 => n as u64,
                    Value::Float(f) if f >= 0.0 => f as u64,
                    other => {
                        eprintln!("halt.error: @wait requires a non-negative Int or Float, got {:?}{}", other, self.current_span_info(*_ip));
                        return OpResult::Halt;
                    }
                };
                std::thread::sleep(std::time::Duration::from_millis(ms));
                OpResult::Continue
            }
 
            OpCode::EnvGet => {
                let name_val = self.stack.pop().expect("EnvGet: expected var name on stack");
                let name = name_val.to_string();
                match std::env::var(&name) {
                    Ok(val) => {
                        self.stack.push(Value::String(val));
                    }
                    Err(_) => {
                        eprintln!("halt.error: env variable \"{}\" is not set{}", name, self.current_span_info(*_ip));
                        return OpResult::Halt;
                    }
                }
                OpResult::Continue
            }
 
            OpCode::EnvArgs => {
                let args: Vec<Value> = std::env::args()
                    .map(|s| Value::String(s))
                    .collect();
                self.stack.push(Value::Array(Arc::new(RwLock::new(args))));
                OpResult::Continue
            }
 
            OpCode::CryptoHash => {
                let algo_val = self.stack.pop().expect("CryptoHash: expected algo");
                let pwd_val  = self.stack.pop().expect("CryptoHash: expected password");
                let algo = algo_val.to_string();
                let password = pwd_val.to_string();
 
                let res = match algo.as_str() {
                    "bcrypt" => {
                        match bcrypt::hash(&password, bcrypt::DEFAULT_COST) {
                            Ok(h) => Value::String(h),
                            Err(_) => { eprintln!("halt.error: bcrypt hashing failed{}", self.current_span_info(*_ip)); return OpResult::Halt; }
                        }
                    }
                    "argon2" => {
                        let mut salt_bytes = [0u8; 16];
                        rand::rng().fill(&mut salt_bytes);
                        let salt = SaltString::encode_b64(&salt_bytes).unwrap();
                        let argon2_inst = argon2::Argon2::default();
                        match argon2_inst.hash_password(password.as_bytes(), &salt) {
                            Ok(p) => {
                                Value::String(p.to_string())
                            }
                            Err(_) => { eprintln!("halt.error: argon2 hashing failed{}", self.current_span_info(*_ip)); return OpResult::Halt; }
                        }
                    }
                    _ => {
                        eprintln!("halt.error: unknown crypto algorithm: {}{}", algo, self.current_span_info(*_ip));
                        return OpResult::Halt;
                    }
                };
                self.stack.push(res);
                OpResult::Continue
            }

            OpCode::CryptoVerify => {
                let algo_val = self.stack.pop().expect("CryptoVerify: expected algo");
                let hash_val = self.stack.pop().expect("CryptoVerify: expected hash");
                let pwd_val  = self.stack.pop().expect("CryptoVerify: expected password");
                
                let algo = algo_val.to_string();
                let hashed = hash_val.to_string();
                let password = pwd_val.to_string();

                let is_ok = match algo.as_str() {
                    "bcrypt" => {
                        bcrypt::verify(&password, &hashed).unwrap_or(false)
                    }
                    "argon2" => {
                        match PasswordHash::new(&hashed) {
                            Ok(parsed_hash) => argon2::Argon2::default().verify_password(password.as_bytes(), &parsed_hash).is_ok(),
                            Err(_) => { eprintln!("halt.error: invalid argon2 hash format{}", self.current_span_info(*_ip)); return OpResult::Halt; }
                        }
                    }
                    _ => {
                        eprintln!("halt.error: unknown crypto algorithm for verification: {}{}", algo, self.current_span_info(*_ip));
                        return OpResult::Halt;
                    }
                };
                self.stack.push(Value::Bool(is_ok));
                OpResult::Continue
            }

            OpCode::CryptoToken => {
                let len_val = self.stack.pop().expect("CryptoToken: expected length");
                let len = match len_val {
                    Value::Int(n) if n > 0 => n as usize,
                    _ => { eprintln!("halt.error: crypto.token requires a positive Int length{}", self.current_span_info(*_ip)); return OpResult::Halt; }
                };
                
                let mut bytes = vec![0u8; (len + 1) / 2];
                rand::rng().fill(&mut bytes[..]);
                let mut hex_str = hex::encode(&bytes);
                if hex_str.len() > len {
                    hex_str.truncate(len);
                }
                self.stack.push(Value::String(hex_str));
                OpResult::Continue
            }

            OpCode::Add => {
                let b = self.stack.pop().expect("Add: rhs");
                let a = self.stack.pop().expect("Add: lhs");
                let res = match (a, b) {
                    (Value::Int(a), Value::Int(b))     => Value::Int(a.wrapping_add(b)),
                    (Value::Float(a), Value::Float(b)) => Value::Float(a + b),
                    (Value::Int(a), Value::Float(b))   => Value::Float(a as f64 + b),
                    (Value::Float(a), Value::Int(b))   => Value::Float(a + b as f64),
                    (Value::String(a), b)              => Value::String(format!("{}{}", a, b)),
                    (a, Value::String(b))              => Value::String(format!("{}{}", a, b)),
                    (Value::Date(d), Value::Int(days)) => Value::Date(d + chrono::TimeDelta::days(days)),
                    (Value::Set(a), Value::Set(b))     => Value::Set(Arc::new(RwLock::new(
                        set_op(&a.read(), &b.read(), 0)))),
                    _ => Value::Bool(false),
                };
                self.stack.push(res);
                OpResult::Continue
            }
            OpCode::Sub => {
                let b = self.stack.pop().expect("Sub: rhs");
                let a = self.stack.pop().expect("Sub: lhs");
                let res = match (a, b) {
                    (Value::Int(a), Value::Int(b))     => Value::Int(a.wrapping_sub(b)),
                    (Value::Float(a), Value::Float(b)) => Value::Float(a - b),
                    (Value::Int(a), Value::Float(b))   => Value::Float(a as f64 - b),
                    (Value::Float(a), Value::Int(b))   => Value::Float(a - b as f64),
                    (Value::Date(d), Value::Int(days)) => Value::Date(d - chrono::TimeDelta::days(days)),
                    (Value::Date(a), Value::Date(b))   => Value::Int((a - b).num_days()),
                    (Value::Set(a), Value::Set(b))     => Value::Set(Arc::new(RwLock::new(
                        set_op(&a.read(), &b.read(), 2)))),
                    _ => Value::Bool(false),
                };
                self.stack.push(res);
                OpResult::Continue
            }
            OpCode::Mul => {
                let b = self.stack.pop().expect("Mul: rhs");
                let a = self.stack.pop().expect("Mul: lhs");
                match (a, b) {
                    (Value::Int(a), Value::Int(b))     => self.stack.push(Value::Int(a.wrapping_mul(b))),
                    (Value::Float(a), Value::Float(b)) => self.stack.push(Value::Float(a * b)),
                    (Value::Int(a), Value::Float(b))   => self.stack.push(Value::Float(a as f64 * b)),
                    (Value::Float(a), Value::Int(b))   => self.stack.push(Value::Float(a * b as f64)),
                    (Value::Set(a), Value::Set(b))     => {
                        self.stack.push(Value::Set(Arc::new(RwLock::new(
                            set_op(&a.read(), &b.read(), 1)))));
                    }
                    (a, b) => {
                        eprintln!("ERROR: Cannot multiply {:?} and {:?}{}", a, b, self.current_span_info(*_ip));
                        self.stack.push(Value::Int(0));
                        return OpResult::Halt;
                    }
                }
                OpResult::Continue
            }
            OpCode::Div => {
                let b = self.stack.pop().expect("Div: rhs");
                let a = self.stack.pop().expect("Div: lhs");
                match (a, b) {
                    (Value::Int(a), Value::Int(b)) => {
                        if b == 0 { eprintln!("R300: Division by zero{}", self.current_span_info(*_ip)); return OpResult::Halt; }
                        self.stack.push(Value::Int(a / b));
                    }
                    (Value::Float(a), Value::Float(b)) => {
                        if b == 0.0 { eprintln!("R300: Division by zero (float){}", self.current_span_info(*_ip)); return OpResult::Halt; }
                        self.stack.push(Value::Float(a / b));
                    }
                    (Value::Int(a), Value::Float(b)) => {
                        if b == 0.0 { eprintln!("R300: Division by zero{}", self.current_span_info(*_ip)); return OpResult::Halt; }
                        self.stack.push(Value::Float(a as f64 / b));
                    }
                    (Value::Float(a), Value::Int(b)) => {
                        if b == 0 { eprintln!("R300: Division by zero{}", self.current_span_info(*_ip)); return OpResult::Halt; }
                        self.stack.push(Value::Float(a / b as f64));
                    }
                    _ => return OpResult::Halt,
                }
                OpResult::Continue
            }
            OpCode::Mod => {
                let b = self.stack.pop().expect("Mod: rhs");
                let a = self.stack.pop().expect("Mod: lhs");
                match (a, b) {
                    (Value::Int(a), Value::Int(b)) =>
                        self.stack.push(if b != 0 { Value::Int(a % b) } else { Value::Bool(false) }),
                    (Value::Float(a), Value::Float(b)) => self.stack.push(Value::Float(a % b)),
                    _ => self.stack.push(Value::Bool(false)),
                }
                OpResult::Continue
            }
            OpCode::Pow => {
                let b = self.stack.pop().expect("Pow: rhs");
                let a = self.stack.pop().expect("Pow: lhs");
                match (a, b) {
                    (Value::Int(a), Value::Int(b))     => self.stack.push(Value::Int(a.pow(b as u32))),
                    (Value::Float(a), Value::Float(b)) => self.stack.push(Value::Float(a.powf(b))),
                    _ => self.stack.push(Value::Bool(false)),
                }
                OpResult::Continue
            }
            OpCode::IntConcat => {
                let b = self.stack.pop().expect("IntConcat: rhs");
                let a = self.stack.pop().expect("IntConcat: lhs");
                let s = format!("{}{}", a, b);
                self.stack.push(s.parse::<i64>().map(Value::Int).unwrap_or(Value::String(s)));
                OpResult::Continue
            }

            OpCode::Equal => {
                let b = self.stack.pop().expect("Equal: rhs");
                let a = self.stack.pop().expect("Equal: lhs");
                self.stack.push(Value::Bool(a == b)); OpResult::Continue
            }
            OpCode::NotEqual => {
                let b = self.stack.pop().expect("NotEqual: rhs");
                let a = self.stack.pop().expect("NotEqual: lhs");
                self.stack.push(Value::Bool(a != b)); OpResult::Continue
            }
            OpCode::Greater => {
                let b = self.stack.pop().expect("Greater: rhs");
                let a = self.stack.pop().expect("Greater: lhs");
                self.stack.push(Value::Bool(a > b)); OpResult::Continue
            }
            OpCode::Less => {
                let b = self.stack.pop().expect("Less: rhs");
                let a = self.stack.pop().expect("Less: lhs");
                self.stack.push(Value::Bool(a < b)); OpResult::Continue
            }
            OpCode::GreaterEqual => {
                let b = self.stack.pop().expect("GreaterEqual: rhs");
                let a = self.stack.pop().expect("GreaterEqual: lhs");
                self.stack.push(Value::Bool(a >= b)); OpResult::Continue
            }
            OpCode::LessEqual => {
                let b = self.stack.pop().expect("LessEqual: rhs");
                let a = self.stack.pop().expect("LessEqual: lhs");
                self.stack.push(Value::Bool(a <= b)); OpResult::Continue
            }
            OpCode::And => {
                let b = self.stack.pop().expect("And: rhs");
                let a = self.stack.pop().expect("And: lhs");
                match (a, b) {
                    (Value::Bool(a), Value::Bool(b)) => self.stack.push(Value::Bool(a && b)),
                    _ => self.stack.push(Value::Bool(false)),
                }
                OpResult::Continue
            }
            OpCode::Or => {
                let b = self.stack.pop().expect("Or: rhs");
                let a = self.stack.pop().expect("Or: lhs");
                match (a, b) {
                    (Value::Bool(a), Value::Bool(b)) => self.stack.push(Value::Bool(a || b)),
                    _ => self.stack.push(Value::Bool(false)),
                }
                OpResult::Continue
            }
            OpCode::Not => {
                let a = self.stack.pop().expect("Not: operand");
                match a {
                    Value::Bool(v) => self.stack.push(Value::Bool(!v)),
                    _ => self.stack.push(Value::Bool(false)),
                }
                OpResult::Continue
            }
            OpCode::Has => {
                let needle = self.stack.pop().expect("Has: needle");
                let col    = self.stack.pop().expect("Has: collection");
                let res = match (col, needle) {
                    (Value::String(av), Value::String(bv)) => av.contains(bv.as_str()),
                    (Value::Array(arr), needle) => arr.read().contains(&needle),
                    (Value::Set(s), needle)     => s.read().contains(&needle),
                    _ => false,
                };
                self.stack.push(Value::Bool(res));
                OpResult::Continue
            }

            OpCode::SetUnion | OpCode::SetIntersection | OpCode::SetDifference | OpCode::SetSymDifference => {
                let b = self.stack.pop().unwrap();
                let a = self.stack.pop().unwrap();
                if let (Value::Set(a), Value::Set(b)) = (a, b) {
                    let op_id: u8 = match op {
                        OpCode::SetUnion        => 0,
                        OpCode::SetIntersection => 1,
                        OpCode::SetDifference   => 2,
                        _                       => 3,
                    };
                    self.stack.push(Value::Set(Arc::new(RwLock::new(
                        set_op(&a.read(), &b.read(), op_id)
                    ))));
                } else {
                    self.stack.push(Value::Bool(false));
                }
                OpResult::Continue
            }

            OpCode::ArrayInit(count) => {
                let mut elems: Vec<Value> = (0..count)
                    .map(|_| self.stack.pop().expect("ArrayInit: underflow"))
                    .collect();
                elems.reverse();
                self.stack.push(Value::Array(Arc::new(RwLock::new(elems))));
                OpResult::Continue
            }
            OpCode::SetInit(count) => {
                let elems: std::collections::BTreeSet<Value> = (0..count)
                    .map(|_| self.stack.pop().expect("SetInit: underflow"))
                    .collect();
                self.stack.push(Value::Set(Arc::new(RwLock::new(elems))));
                OpResult::Continue
            }
            OpCode::MapInit(count) => {
                let mut map: Vec<(Value, Value)> = Vec::with_capacity(count);
                for _ in 0..count {
                    let v = self.stack.pop().expect("MapInit: val underflow");
                    let k = self.stack.pop().expect("MapInit: key underflow");
                    map.push((k, v));
                }
                map.reverse();
                self.stack.push(Value::Map(Arc::new(RwLock::new(map))));
                OpResult::Continue
            }

            OpCode::SetRange => {
                let has_step = matches!(
                    self.stack.pop().expect("SetRange: flag"),
                    Value::Bool(true)
                );
                let step_val = if has_step { self.stack.pop().expect("SetRange: step") } else { Value::Int(1) };
                let end_val   = self.stack.pop().expect("SetRange: end");
                let start_val = self.stack.pop().expect("SetRange: start");
 
                let mut elements: Vec<Value> = Vec::new();
                match (start_val, end_val, step_val) {
                    (Value::Int(start), Value::Int(end), Value::Int(step)) => {
                        if step > 0 { let mut c = start; while c <= end   { elements.push(Value::Int(c)); c += step; } }
                        else if step < 0 { let mut c = start; while c >= end { elements.push(Value::Int(c)); c += step; } }
                    }
                    (Value::Float(start), Value::Float(end), sv) => {
                        let step = match sv { Value::Float(f) => f, Value::Int(i) => i as f64, _ => 1.0 };
                        if step > 0.0 { let mut c = start; while c <= end + 1e-9 { elements.push(Value::Float(c)); c += step; } }
                        else if step < 0.0 { let mut c = start; while c >= end - 1e-9 { elements.push(Value::Float(c)); c += step; } }
                    }
                    (Value::String(start), Value::String(end), Value::Int(step))
                        if start.chars().count() == 1 && end.chars().count() == 1 =>
                    {
                        let sc = start.chars().next().unwrap() as u32;
                        let ec = end.chars().next().unwrap() as u32;
                        if step > 0 {
                            let mut c = sc;
                            while c <= ec {
                                if let Some(ch) = std::char::from_u32(c) { elements.push(Value::String(ch.to_string())); }
                                c = c.wrapping_add(step as u32);
                                if c > 0x10FFFF { break; }
                            }
                        } else if step < 0 {
                            let abs = step.unsigned_abs() as u32;
                            let mut c = sc;
                            while c >= ec {
                                if let Some(ch) = std::char::from_u32(c) { elements.push(Value::String(ch.to_string())); }
                                if c < abs { break; }
                                c -= abs;
                            }
                        }
                    }
                    _ => {}
                }
                self.stack.push(Value::Set(Arc::new(RwLock::new(
                    elements.into_iter().collect()
                ))));
                OpResult::Continue
            }

            OpCode::RandomChoice => {
                let receiver = self.stack.pop().unwrap();
                use rand::Rng;
                let mut rng = rand::rng();
                match receiver {
                    Value::Array(a) => {
                        let arr = a.read();
                        if arr.is_empty() { self.stack.push(Value::Bool(false)); }
                        else { self.stack.push(arr[rng.random_range(0..arr.len())].clone()); }
                    }
                    Value::Set(s) => {
                        let set = s.read();
                        if set.is_empty() { self.stack.push(Value::Bool(false)); }
                        else {
                            let val = set.iter().nth(rng.random_range(0..set.len())).unwrap().clone();
                            self.stack.push(val);
                        }
                    }
                    _ => self.stack.push(Value::Bool(false)),
                }
                OpResult::Continue
            }

            OpCode::Print => {
                let val = self.stack.pop().expect("Print: empty stack");
                let s = val.to_string();
                if s.contains('\x1B') || s.contains('\r') || s.ends_with('\n') {
                    print!("{}", s);
                } else {
                    println!("{}", s);
                }
                use std::io::Write;
                std::io::stdout().flush().unwrap();
                OpResult::Continue
            }
            OpCode::Input => {
                use std::io::{self, Write};
                print!("> ");
                io::stdout().flush().unwrap();
                let mut input = String::new();
                io::stdin().read_line(&mut input).unwrap();
                let t = input.trim();
                self.stack.push(
                    if let Ok(i) = t.parse::<i64>()      { Value::Int(i) }
                    else if let Ok(f) = t.parse::<f64>() { Value::Float(f) }
                    else if t == "true"                   { Value::Bool(true) }
                    else if t == "false"                  { Value::Bool(false) }
                    else                                  { Value::String(t.to_string()) }
                );
                OpResult::Continue
            }

            OpCode::StoreWrite => {
                let content  = self.stack.pop().unwrap();
                let path_val = self.stack.pop().unwrap();
                if let (Value::String(c), Value::String(p)) = (&content, &path_val) {
                    if !is_safe_path(p) { return OpResult::Halt; }
                    let path = std::path::Path::new(p);
                    if let Some(parent) = path.parent() {
                        if !parent.as_os_str().is_empty() && std::fs::create_dir_all(parent).is_err() { return OpResult::Halt; }
                    }
                    if std::fs::write(p, c).is_err() { return OpResult::Halt; }
                    self.stack.push(Value::Bool(true));
                } else { return OpResult::Halt; }
                OpResult::Continue
            }
            OpCode::StoreRead => {
                let path_val = self.stack.pop().unwrap();
                if let Value::String(p) = &path_val {
                    if !is_safe_path(p) { return OpResult::Halt; }
                    match std::fs::read_to_string(p) {
                        Ok(c) => self.stack.push(Value::String(c)),
                        Err(_) => return OpResult::Halt,
                    }
                } else { return OpResult::Halt; }
                OpResult::Continue
            }
            OpCode::StoreAppend => {
                let content  = self.stack.pop().unwrap();
                let path_val = self.stack.pop().unwrap();
                if let (Value::String(c), Value::String(p)) = (&content, &path_val) {
                    if !is_safe_path(p) { return OpResult::Halt; }
                    let path = std::path::Path::new(p);
                    if let Some(parent) = path.parent() {
                        if !parent.as_os_str().is_empty() && std::fs::create_dir_all(parent).is_err() { return OpResult::Halt; }
                    }
                    use std::io::Write as IoWrite;
                    match std::fs::OpenOptions::new().append(true).create(true).open(p) {
                        Ok(mut f) => { if write!(f, "{}", c).is_err() { return OpResult::Halt; } }
                        Err(_) => return OpResult::Halt,
                    }
                    self.stack.push(Value::Bool(true));
                } else { return OpResult::Halt; }
                OpResult::Continue
            }
            OpCode::StoreExists => {
                let path_val = self.stack.pop().unwrap();
                if let Value::String(p) = path_val {
                    if !is_safe_path(&p) { return OpResult::Halt; }
                    self.stack.push(Value::Bool(std::path::Path::new(&p).exists()));
                } else { return OpResult::Halt; }
                OpResult::Continue
            }
            OpCode::StoreDelete => {
                let path_val = self.stack.pop().unwrap();
                if let Value::String(p) = path_val {
                    if !is_safe_path(&p) { return OpResult::Halt; }
                    let path = std::path::Path::new(&p);
                    let res = if path.is_dir() {
                        std::fs::remove_dir_all(path).is_ok()
                    } else {
                        std::fs::remove_file(path).is_ok()
                    };
                    self.stack.push(Value::Bool(res));
                } else { return OpResult::Halt; }
                OpResult::Continue
            }

            OpCode::JsonParse => {
                let s_val = self.stack.pop().expect("JsonParse: string");
                if let Value::String(s) = s_val {
                    match serde_json::from_str::<serde_json::Value>(&s) {
                        Ok(v) => self.stack.push(Value::Json(Arc::new(RwLock::new(v)))),
                        Err(_) => return OpResult::Halt,
                    }
                } else { return OpResult::Halt; }
                OpResult::Continue
            }
            OpCode::JsonBind(idx) => {
                let path_val = self.stack.pop().expect("JsonBind: path");
                let json_val = self.stack.pop().expect("JsonBind: json");
                if let (Value::Json(j), Value::String(p)) = (json_val, path_val) {
                    if let Some(v) = json_path_to_value(&j.read(), &p) {
                        let mut glbs = self.vm.globals.write();
                        let target_val = glbs[idx].clone();
                        glbs[idx] = json_value_to_typed_value_raw(&v, &target_val);
                    } else { return OpResult::Halt; }
                }
                OpResult::Continue
            }
            OpCode::JsonBindLocal(idx) => {
                let path_val = self.stack.pop().expect("JsonBindLocal: path");
                let json_val = self.stack.pop().expect("JsonBindLocal: json");
                if let (Value::Json(j), Value::String(p)) = (json_val, path_val) {
                    if let Some(v) = json_path_to_value(&j.read(), &p) {
                        if idx >= locals.len() { locals.resize(idx + 1, Value::Bool(false)); }
                        let target_val = locals[idx].clone();
                        locals[idx] = json_value_to_typed_value_raw(&v, &target_val);
                    } else { return OpResult::Halt; }
                }
                OpResult::Continue
            }
            OpCode::JsonInject(idx) => {
                let mapping_val = self.stack.pop().expect("JsonInject: mapping");
                let json_val    = self.stack.pop().expect("JsonInject: json");
                if let (Value::Json(j), Value::Map(m)) = (json_val, mapping_val) {
                    let glbs = self.vm.globals.read();
                    if let Value::Table(t) = glbs[idx].clone() {
                        inject_json_into_table(&mut t.write(), &j.read(), &m.read());
                    }
                }
                OpResult::Continue
            }
            OpCode::JsonInjectLocal(idx) => {
                let mapping_val = self.stack.pop().expect("JsonInjectLocal: mapping");
                let json_val    = self.stack.pop().expect("JsonInjectLocal: json");
                if let (Value::Json(j), Value::Map(m)) = (json_val, mapping_val) {
                    if let Some(Value::Table(t)) = locals.get(idx).cloned() {
                        inject_json_into_table(&mut t.write(), &j.read(), &m.read());
                    }
                }
                OpResult::Continue
            }

            OpCode::DateNow => {
                self.stack.push(Value::Date(chrono::Utc::now().naive_utc()));
                OpResult::Continue
            }

            OpCode::TableInit(idx, row_count) => {
                let skeleton = self.ctx.constants[idx].clone();
                if let Value::Table(table_lock) = skeleton {
                    let col_def = {
                        let table = table_lock.read();
                        table.columns.clone()
                    };
                    let non_auto_col_count = col_def.iter().filter(|c| !c.is_auto).count();
                    
                    let mut rows = Vec::with_capacity(row_count);
                    for _ in 0..row_count {
                        let mut row_vals = Vec::with_capacity(col_def.len());
                        for _ in 0..col_def.len() {
                            row_vals.push(Value::Bool(false));
                        }
                        rows.push(row_vals);
                    }

                    for r in (0..row_count).rev() {
                        let mut stack_vals = Vec::with_capacity(non_auto_col_count);
                        for _ in 0..non_auto_col_count {
                            stack_vals.push(self.stack.pop().expect("TableInit: missing cell value"));
                        }
                        stack_vals.reverse();
                        
                        let mut stack_idx = 0;
                        for (c_idx, col) in col_def.iter().enumerate() {
                            if col.is_auto {
                                rows[r][c_idx] = Value::Int((r + 1) as i64);
                            } else {
                                rows[r][c_idx] = stack_vals[stack_idx].clone();
                                stack_idx += 1;
                            }
                        }
                    }

                    let new_table_data = TableData {
                        columns: col_def,
                        rows,
                    };
                    self.stack.push(Value::Table(Arc::new(RwLock::new(new_table_data))));
                } else {
                    return OpResult::Halt;
                }
                OpResult::Continue
            }

            OpCode::FiberCreate(func_id, arg_count) => {
                let mut args: Vec<Value> = (0..arg_count)
                    .map(|_| self.stack.pop().expect("FiberCreate: arg"))
                    .collect();
                args.reverse();
                let max = self.ctx.functions[func_id].max_locals.max(args.len());
                args.resize(max, Value::Bool(false));
                self.stack.push(Value::Fiber(Arc::new(RwLock::new(
                    FiberState { func_id, ip: 0, locals: args, stack: Vec::new(), is_done: false, yielded_value: None }
                ))));
                OpResult::Continue
            }

            OpCode::Call(func_id, arg_count) => {
                let mut args: Vec<Value> = (0..arg_count)
                    .map(|_| self.stack.pop().expect("Call: arg"))
                    .collect();
                args.reverse();
                if self.call_depth >= MAX_CALL_DEPTH { return OpResult::Halt; }
                self.call_depth += 1;
                let initial_errors = self.vm.error_count.load(Ordering::Relaxed);
                let res = self.run_frame(func_id, &args);
                self.call_depth -= 1;
                
                if self.vm.error_count.load(Ordering::Relaxed) > initial_errors {
                    return OpResult::Halt;
                }
                
                self.stack.push(res.unwrap_or(Value::Bool(false)));
                OpResult::Continue
            }
            OpCode::MethodCall(kind, arg_count) => {
                // Special-case Fiber methods to reduce stack depth in deep delegation
                if arg_count == 0 {
                    match kind {
                        MethodKind::Next | MethodKind::Run | MethodKind::IsDone | MethodKind::Close => {
                            let receiver = self.stack.pop().expect("MethodCall: receiver");
                            if let Value::Fiber(frc) = receiver {
                                return self.handle_fiber_method(frc, kind, *_ip);
                            }
                            // If not a fiber, fall back to general dispatch
                            self.stack.push(receiver);
                        }
                        _ => {}
                    }
                }

                let mut args: Vec<Value> = (0..arg_count)
                    .map(|_| self.stack.pop().expect("MethodCall: arg"))
                    .collect();
                args.reverse();
                let receiver = self.stack.pop().expect("MethodCall: receiver");
                self.handle_method_call(receiver, kind, args, *_ip)
            }
            OpCode::MethodCallCustom(name_idx, arg_count) => {
                let mut args: Vec<Value> = (0..arg_count)
                    .map(|_| self.stack.pop().expect("MethodCallCustom: arg"))
                    .collect();
                args.reverse();
                let receiver = self.stack.pop().expect("MethodCallCustom: receiver");
                let method_name = self.ctx.constants[name_idx].to_string();
                self.handle_method_call_custom(receiver, method_name, args, *_ip)
            }

            OpCode::HaltError => {
                eprintln!("ERROR: {}{}", self.stack.pop().expect("HaltError"), self.current_span_info(*_ip));
                OpResult::Halt
            }
            OpCode::HaltAlert => {
                println!("HALT.ALERT: {}{}", self.stack.pop().expect("HaltAlert"), self.current_span_info(*_ip));
                OpResult::Continue
            }
            OpCode::HaltFatal => {
                println!("HALT.FATAL: {}{}", self.stack.pop().expect("HaltFatal"), self.current_span_info(*_ip));
                OpResult::Halt
            }


            OpCode::HttpCall(method_idx) => {
                let body = self.stack.pop();
                let url_val = self.stack.pop().expect("HttpCall: url missing");
                let url = url_val.to_string();

                if let Err(e) = is_safe_url(&url) {
                    eprintln!("{}{}", e, self.current_span_info(*_ip));
                    // Return error response instead of halting — consistent with HttpRequest behavior
                    let mut res = serde_json::Map::new();
                    res.insert("status".to_string(), serde_json::Value::Number(0.into()));
                    res.insert("ok".to_string(), serde_json::Value::Bool(false));
                    res.insert("error".to_string(), serde_json::Value::String(e));
                    self.stack.push(Value::Json(Arc::new(RwLock::new(serde_json::Value::Object(res)))));
                    return OpResult::Continue;
                }

                let method = self.ctx.constants[method_idx].to_string().to_uppercase();
                let req = ureq::request(&method, &url)
                    .timeout(std::time::Duration::from_secs(10));

                let result = if let Some(b_val) = body {
                    if !matches!(b_val, Value::Bool(false)) {
                        let body_str = match &b_val {
                            Value::Json(j) => j.read().to_string(),
                            other => other.to_string(),
                        };
                        req.set("Content-Type", "application/json").send_string(&body_str)
                    } else {
                        req.call()
                    }
                } else {
                    req.call()
                };

                self.stack.push(Value::Json(Arc::new(RwLock::new(build_response_json(result)))));
                OpResult::Continue
            }

            OpCode::HttpRequest => {
                let config_val = self.stack.pop().unwrap();
                if let Value::Map(m_rc) = config_val {
                    let m = m_rc.read();
                    let mut method = "GET".to_string();
                    let mut url = String::new();
                    let mut url_safe = true;
                    let mut headers: Vec<(String, String)> = Vec::new();
                    let mut body: Option<String> = None;
                    let mut timeout = 10000u64;

                    for (k, v) in m.iter() {
                        match k.to_string().as_str() {
                            "method" => method = v.to_string().to_uppercase(),
                            "url" => {
                                url = v.to_string();
                                if let Err(e) = is_safe_url(&url) {
                                    eprintln!("{}", e);
                                    url_safe = false;
                                }
                            }
                            "headers" => {
                                if let Value::Map(h_rc) = v {
                                    for (hk, hv) in h_rc.read().iter() {
                                        headers.push((hk.to_string(), hv.to_string()));
                                    }
                                }
                            }
                            "body" => {
                                let bs = match v {
                                    Value::Json(j) => j.read().to_string(),
                                    other => other.to_string(),
                                };
                                body = Some(bs);
                            }
                            "timeout" => if let Value::Int(i) = v { timeout = *i as u64 },
                            _ => {}
                        }
                    }

                    if !url_safe {
                        let mut res = serde_json::Map::new();
                        res.insert("status".to_string(), serde_json::Value::Number(0.into()));
                        res.insert("ok".to_string(), serde_json::Value::Bool(false));
                        res.insert("error".to_string(), serde_json::Value::String("SSRF blocked".to_string()));
                        self.stack.push(Value::Json(Arc::new(RwLock::new(serde_json::Value::Object(res)))));
                        return OpResult::Continue;
                    }

                    let mut req = ureq::request(&method, &url)
                        .timeout(std::time::Duration::from_millis(timeout));

                    for (k, v) in headers {
                        req = req.set(&k, &v);
                    }

                    let result = if let Some(b) = body {
                        req.set("Content-Type", "application/json").send_string(&b)
                    } else {
                        req.call()
                    };

                    self.stack.push(Value::Json(Arc::new(RwLock::new(build_response_json(result)))));
                } else {
                    return OpResult::Halt;
                }
                OpResult::Continue
            }

            OpCode::HttpRespond => {
                let headers = self.stack.pop().unwrap_or(Value::Bool(false));
                let body    = self.stack.pop().unwrap();
                let status  = self.stack.pop().unwrap();

                let mut resp_obj = serde_json::Map::new();
                resp_obj.insert("status".to_string(), value_to_json(&status));
                resp_obj.insert("body".to_string(),   value_to_json(&body));
                resp_obj.insert("headers".to_string(), value_to_json(&headers));

                self.stack.push(Value::Json(Arc::new(RwLock::new(serde_json::Value::Object(resp_obj)))));
                OpResult::Continue
            }
            OpCode::HttpServe(name_idx) => {
                let routes   = self.stack.pop().unwrap();
                let workers_val = self.stack.pop().unwrap();
                let host_val = self.stack.pop().unwrap();
                let port_val = self.stack.pop().unwrap().to_string();

                let workers = if let Value::Int(n) = workers_val { n.max(1) as usize } else { 1 };
                let name = self.ctx.constants[name_idx].to_string();
                let host_str = match &host_val {
                    Value::Bool(false) => "127.0.0.1".to_string(),
                    other => other.to_string(),
                };
                let addr = format!("{}:{}", host_str, port_val);

                let server = if let Some(s) = self.vm.servers.read().get(&name) {
                    s.clone()
                } else {
                    let s = match tiny_http::Server::http(&addr) {
                        Ok(s) => std::sync::Arc::new(s),
                        Err(e) => {
                            eprintln!("Failed to start server '{}' on {}: {}", name, addr, e);
                            return OpResult::Halt;
                        }
                    };
                    self.vm.servers.write().insert(name.clone(), s.clone());
                    println!("Server: starting '{}' on http://{} with {} workers", name, addr, workers);
                    s
                };

                let mut handles = Vec::new();
                for i in 0..workers {
                    let server_clone = server.clone();
                    let vm_clone = self.vm.clone();
                    let ctx_clone = self.ctx.clone();
                    let routes_clone = routes.clone();
                    let name_clone = name.clone();

                    let handle = std::thread::spawn(move || {
                        let mut worker_executor = Executor {
                            vm: vm_clone,
                            ctx: ctx_clone,
                            current_spans: None,
                            stack: Vec::with_capacity(128),
                            call_depth: 0,
                            fiber_yielded: false,
                        };

                        loop {
                            if SHUTDOWN.load(Ordering::SeqCst) {
                                if i == 0 { println!("Server '{}' shutting down gracefully...", name_clone); }
                                break;
                            }

                            if let Ok(Some(mut request)) = server_clone.recv_timeout(std::time::Duration::from_millis(100)) {
                                let method = request.method().to_string().to_uppercase();
                                let raw_url = request.url().to_string();
                                let path_only = raw_url.split('?').next().unwrap_or(&raw_url).to_string();
                                
                                let query_str = if let Some(pos) = raw_url.find('?') {
                                    raw_url[pos + 1..].to_string()
                                } else {
                                    String::new()
                                };
                                
                                let query_obj: serde_json::Value = {
                                    let mut m = serde_json::Map::new();
                                    if !query_str.is_empty() {
                                        for pair in query_str.split('&') {
                                            let mut kv = pair.splitn(2, '=');
                                            let k = kv.next().unwrap_or("").to_string();
                                            let v = kv.next().unwrap_or("").to_string();
                                            if !k.is_empty() {
                                                let decoded_k = url_decode(&k);
                                                let decoded_v = url_decode(&v);
                                                m.insert(decoded_k, serde_json::Value::String(decoded_v));
                                            }
                                        }
                                    }
                                    serde_json::Value::Object(m)
                                };

                                let routes_map_rc = if let Value::Map(m) = &routes_clone { m } else { break; };
                                let routes_map = routes_map_rc.read();

                                let handler = routes_map
                                    .iter()
                                    .find(|(k, _)| {
                                        let k_str = k.to_string();
                                        if k_str == "*" { return false; }
                                        let k_parts: Vec<&str> = k_str.split_whitespace().collect();
                                        if k_parts.len() == 2 {
                                            let r_meth = k_parts[0].to_uppercase();
                                            let r_path = k_parts[1];
                                            r_meth == method && (r_path == path_only || r_path == "*")
                                        } else {
                                            false
                                        }
                                    })
                                    .or_else(|| {
                                        if method == "HEAD" {
                                            routes_map.iter().find(|(k, _)| {
                                                let k_str = k.to_string();
                                                let k_parts: Vec<&str> = k_str.split_whitespace().collect();
                                                k_parts.len() == 2 && k_parts[0].to_uppercase() == "GET" && k_parts[1] == path_only
                                            })
                                        } else {
                                            None
                                        }
                                    })
                                    .or_else(|| routes_map.iter().find(|(k, _)| k.to_string() == "*"))
                                    .map(|(_, v)| v.clone());

                                drop(routes_map);

                                if let Some(Value::Function(fid)) = handler {
                                    let mut h_map = serde_json::Map::new();
                                    for h in request.headers() {
                                        h_map.insert(h.field.to_string().to_lowercase(), serde_json::Value::String(h.value.to_string()));
                                    }

                                    let mut body_bytes = Vec::new();
                                    {
                                        let reader = request.as_reader();
                                        let limit = 10 * 1024 * 1024;
                                        let mut limited_reader = std::io::Read::take(reader, limit as u64 + 1);
                                        let _ = std::io::Read::read_to_end(&mut limited_reader, &mut body_bytes);
                                    }

                                    if body_bytes.len() > 10 * 1024 * 1024 {
                                        let resp = tiny_http::Response::from_string("{\"error\": \"Payload Too Large (10MB Limit)\"}")
                                            .with_status_code(413)
                                            .with_header(tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap());
                                        let _ = request.respond(resp);
                                        continue;
                                    }

                                    let body_str  = String::from_utf8_lossy(&body_bytes).to_string();
                                    let body_json = serde_json::from_str(&body_str).unwrap_or(serde_json::Value::String(body_str));

                                    let mut req_map = serde_json::Map::new();
                                    req_map.insert("method".to_string(),  serde_json::Value::String(method.clone()));
                                    req_map.insert("path".to_string(),    serde_json::Value::String(path_only.clone()));
                                    req_map.insert("query".to_string(),   query_obj);
                                    req_map.insert("headers".to_string(), serde_json::Value::Object(h_map));
                                    req_map.insert("body".to_string(),    body_json);
                                    req_map.insert("ip".to_string(),      serde_json::Value::String(request.remote_addr().map(|a| a.to_string()).unwrap_or_else(|| "127.0.0.1".to_string())));
                                    
                                    let req_val = Value::Json(Arc::new(RwLock::new(serde_json::Value::Object(req_map))));

                                    let mut resp_val = worker_executor.run_frame(fid, &[req_val]);
                                    if let Some(Value::Fiber(f_rc)) = resp_val {
                                        resp_val = worker_executor.resume_fiber(f_rc, true);
                                    }

                                    if let Some(Value::Json(resp_json_rc)) = resp_val {
                                        worker_executor.send_tiny_http_response(request, resp_json_rc);
                                    } else {
                                        let resp = tiny_http::Response::from_string("{\"error\": \"Internal Server Error\"}")
                                            .with_status_code(500)
                                            .with_header(tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap())
                                            .with_header(tiny_http::Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap());
                                        let _ = request.respond(resp);
                                    }
                                } else {
                                    let resp = tiny_http::Response::from_string("{\"error\": \"Not Found\"}")
                                        .with_status_code(404)
                                        .with_header(tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap())
                                        .with_header(tiny_http::Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap());
                                    let _ = request.respond(resp);
                                }
                            }
                        }
                    });
                    handles.push(handle);
                }

                for h in handles { let _ = h.join(); }
                OpResult::Halt
            }

            OpCode::CastInt => {
                let v = self.stack.pop().expect("CastInt");
                let res = match v {
                    Value::Int(i) => Value::Int(i),
                    Value::Float(f) => Value::Int(f as i64),
                    Value::String(s) => Value::Int(s.trim().parse::<i64>().unwrap_or(0)),
                    Value::Bool(b) => Value::Int(if b { 1 } else { 0 }),
                    _ => Value::Int(0),
                };
                self.stack.push(res);
                OpResult::Continue
            }
            OpCode::CastFloat => {
                let v = self.stack.pop().expect("CastFloat");
                let res = match v {
                    Value::Int(i) => Value::Float(i as f64),
                    Value::Float(f) => Value::Float(f),
                    Value::String(s) => Value::Float(s.trim().parse::<f64>().unwrap_or(0.0)),
                    Value::Bool(b) => Value::Float(if b { 1.0 } else { 0.0 }),
                    _ => Value::Float(0.0),
                };
                self.stack.push(res);
                OpResult::Continue
            }
            OpCode::CastString => {
                let v = self.stack.pop().expect("CastString");
                self.stack.push(Value::String(v.to_string()));
                OpResult::Continue
            }
            OpCode::CastBool => {
                let v = self.stack.pop().expect("CastBool");
                let res = match v {
                    Value::Int(i) => Value::Bool(i != 0),
                    Value::Float(f) => Value::Bool(f != 0.0),
                    Value::String(s) => Value::Bool(!s.is_empty()),
                    Value::Bool(b) => Value::Bool(b),
                    _ => Value::Bool(false),
                };
                self.stack.push(res);
                OpResult::Continue
            }
        }
    }

    // ── method dispatch ───────────────────────────────────────────────────────

    fn handle_method_call(&mut self, receiver: Value, kind: MethodKind, args: Vec<Value>, ip: usize) -> OpResult {
        match receiver {
            Value::Array(arr_rc)  => self.handle_array_method(arr_rc, kind, args, ip),
            Value::Set(set_rc)    => self.handle_set_method(set_rc, kind, args, ip),
            Value::Map(map_rc)    => self.handle_map_method(map_rc, kind, args, ip),
            Value::Table(t_rc)    => self.handle_table_method(t_rc, kind, args, ip),
            Value::Row(t_rc, idx) => self.handle_row_method(t_rc, idx, kind, ip),
            Value::Date(d)        => self.handle_date_method(d, kind, args, ip),
            Value::Json(j_rc)     => self.handle_json_method(j_rc.clone(), kind, args, ip),
            Value::Fiber(f_rc)    => self.handle_fiber_method(f_rc, kind, ip),
            Value::String(s)      => self.handle_string_method(s, kind, args, ip),
            Value::Int(i) => {
                match kind {
                    MethodKind::ToStr => {
                        self.stack.push(Value::String(i.to_string()));
                        OpResult::Continue
                    }
                    _ => {
                        eprintln!("Method {:?} not found on Int{}", kind, self.current_span_info(ip));
                        OpResult::Halt
                    }
                }
            }
            Value::Float(f) => {
                match kind {
                    MethodKind::ToStr => {
                        self.stack.push(Value::String(f.to_string()));
                        OpResult::Continue
                    }
                    _ => {
                        eprintln!("Method {:?} not found on Float{}", kind, self.current_span_info(ip));
                        OpResult::Halt
                    }
                }
            }
            Value::Bool(b) => {
                match kind {
                    MethodKind::ToStr => {
                        self.stack.push(Value::String(b.to_string()));
                        OpResult::Continue
                    }
                    _ => {
                        eprintln!("Method {:?} not found on Bool{}", kind, self.current_span_info(ip));
                        OpResult::Halt
                    }
                }
            }
            Value::Function(_) => {
                eprintln!("Method calls not supported for Function type{}", self.current_span_info(ip));
                OpResult::Halt
            }
        }
    }

    fn handle_method_call_custom(&mut self, receiver: Value, method_name: String, args: Vec<Value>, ip: usize) -> OpResult {
        match receiver {
            Value::Row(t_rc, idx) => self.handle_row_custom(t_rc, idx, &method_name, ip),
            Value::Json(j_rc) => self.handle_json_custom(j_rc, &method_name, args, ip),
            _ => {
                eprintln!("Method {} not found on {:?}{}", method_name, receiver, self.current_span_info(ip));
                OpResult::Halt
            }
        }
    }

    // ── fiber resume ──────────────────────────────────────────────────────────

    fn resume_fiber(
        &mut self,
        fiber_rc: Arc<RwLock<FiberState>>,
        is_next: bool,
    ) -> Option<Value> {
        let (func_id, mut ip, mut locals, fstack) = {
            let mut f = fiber_rc.write();
            if f.is_done { return if is_next { f.yielded_value.clone() } else { None }; }
            (f.func_id, f.ip, std::mem::take(&mut f.locals), std::mem::take(&mut f.stack))
        };

        let chunk = self.ctx.functions[func_id].clone();
        let old_spans = self.current_spans.replace(chunk.spans.clone());

        let stack_base = self.stack.len();
        self.stack.extend(fstack);
        self.fiber_yielded = false;
        
        let res = self.execute_bytecode(&chunk.bytecode, &mut ip, &mut locals);
        
        let fstack_after = self.stack.split_off(stack_base);

        {
            let mut f = fiber_rc.write();
            f.ip     = ip;
            f.locals = locals;
            f.stack  = fstack_after;
            if !self.fiber_yielded { f.is_done = true; }
        }
        self.current_spans = old_spans;
        res
    }

    // ── core execution loop ───────────────────────────────────────────────────

    fn run_frame_owned(&mut self, chunk: FunctionChunk) -> Option<Value> {
        self.current_spans = Some(chunk.spans.clone());
        let mut ip = 0;
        let mut locals = vec![Value::Bool(false); chunk.max_locals];
        let res = self.execute_bytecode(&chunk.bytecode, &mut ip, &mut locals);
        res
    }

    fn run_frame(&mut self, func_id: usize, params: &[Value]) -> Option<Value> {
        let chunk = self.ctx.functions[func_id].clone();
        let old_spans = self.current_spans.replace(chunk.spans.clone());
        let mut ip = 0;
        let mut locals = params.to_vec();
        locals.resize(chunk.max_locals.max(params.len()), Value::Bool(false));
        let res = self.execute_bytecode(&chunk.bytecode, &mut ip, &mut locals);
        self.current_spans = old_spans;
        res
    }

    fn execute_bytecode(&mut self, bytecode: &[OpCode], ip: &mut usize, locals: &mut Vec<Value>) -> Option<Value> {
        while *ip < bytecode.len() {
            let op = bytecode[*ip];
            *ip += 1;
            match self.execute_step(op, ip, locals) {
                OpResult::Continue => {}
                OpResult::Jump(t) => *ip = t,
                OpResult::Return(val) => {
                    *ip = bytecode.len();
                    self.fiber_yielded = false;
                    return val;
                }
                OpResult::Yield(val) => {
                    self.fiber_yielded = true;
                    return val;
                }
                OpResult::Halt => {
                    *ip = bytecode.len();
                    self.fiber_yielded = false;
                    self.vm.error_count.fetch_add(1, Ordering::Relaxed);
                    return None;
                }
            }
        }
        *ip = bytecode.len();
        self.fiber_yielded = false;
        None
    }

    // ── collection methods ────────────────────────────────────────────────────

    fn handle_array_method(&mut self, arr_rc: Arc<RwLock<Vec<Value>>>, kind: MethodKind, args: Vec<Value>, ip: usize) -> OpResult {
        match kind {
            MethodKind::Push => { arr_rc.write().push(args[0].clone()); self.stack.push(Value::Bool(true)); }
            MethodKind::Pop  => { let res = arr_rc.write().pop().unwrap_or(Value::Bool(false)); self.stack.push(res); }
            MethodKind::Len | MethodKind::Count | MethodKind::Size => self.stack.push(Value::Int(arr_rc.read().len() as i64)),
            MethodKind::Clear => { arr_rc.write().clear(); self.stack.push(Value::Bool(true)); }
            MethodKind::Contains => self.stack.push(Value::Bool(arr_rc.read().contains(&args[0]))),
            MethodKind::IsEmpty  => self.stack.push(Value::Bool(arr_rc.read().is_empty())),
            MethodKind::Get => {
                let arr = arr_rc.read();
                if let Value::Int(i) = args[0] {
                    if i >= 0 && (i as usize) < arr.len() {
                        self.stack.push(arr[i as usize].clone());
                    } else {
                        eprintln!("R303: Array index out of bounds: {}{}", i, self.current_span_info(ip));
                        return OpResult::Halt;
                    }
                } else { self.stack.push(Value::Bool(false)); }
            }
            MethodKind::Insert => {
                if let (Value::Int(i), val) = (args[0].clone(), args[1].clone()) {
                    let mut arr = arr_rc.write();
                    if i >= 0 && (i as usize) <= arr.len() {
                        arr.insert(i as usize, val);
                        self.stack.push(Value::Bool(true));
                    } else {
                        eprintln!("R303: Array insert index out of bounds: {}{}", i, self.current_span_info(ip));
                        return OpResult::Halt;
                    }
                } else { self.stack.push(Value::Bool(false)); }
            }
            MethodKind::Update => {
                if let (Value::Int(i), val) = (args[0].clone(), args[1].clone()) {
                    let mut arr = arr_rc.write();
                    if i >= 0 && (i as usize) < arr.len() {
                        arr[i as usize] = val;
                        self.stack.push(Value::Bool(true));
                    } else {
                        eprintln!("R303: Array update index out of bounds: {}{}", i, self.current_span_info(ip));
                        return OpResult::Halt;
                    }
                } else { self.stack.push(Value::Bool(false)); }
            }
            MethodKind::Delete => {
                if let Value::Int(i) = args[0] {
                    let mut arr = arr_rc.write();
                    if i >= 0 && (i as usize) < arr.len() {
                        arr.remove(i as usize);
                        self.stack.push(Value::Bool(true));
                    } else {
                        eprintln!("R303: Array delete index out of bounds: {}{}", i, self.current_span_info(ip));
                        return OpResult::Halt;
                    }
                } else { self.stack.push(Value::Bool(false)); }
            }
            MethodKind::Find => {
                let needle = &args[0];
                let arr = arr_rc.read();
                let idx = arr.iter().position(|v| v == needle).map(|i| i as i64).unwrap_or(-1);
                self.stack.push(Value::Int(idx));
            }
            MethodKind::Join => {
                let sep = if let Value::String(s) = &args[0] { s.as_str() } else { "" };
                let arr = arr_rc.read();
                let res = arr.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(sep);
                self.stack.push(Value::String(res));
            }
            MethodKind::Show => { println!("{}", Value::Array(arr_rc.clone())); self.stack.push(Value::Bool(true)); }
            MethodKind::Sort => {
                arr_rc.write().sort();
                self.stack.push(Value::Bool(true));
            }
            MethodKind::Reverse => {
                arr_rc.write().reverse();
                self.stack.push(Value::Bool(true));
            }
            _ => { eprintln!("Method {:?} not supported for Array{}", kind, self.current_span_info(ip)); return OpResult::Halt; }
        }
        OpResult::Continue
    }

    fn handle_set_method(&mut self, set_rc: Arc<RwLock<std::collections::BTreeSet<Value>>>, kind: MethodKind, args: Vec<Value>, ip: usize) -> OpResult {
        match kind {
            MethodKind::Add => { set_rc.write().insert(args[0].clone()); self.stack.push(Value::Bool(true)); }
            MethodKind::Remove   => { let ok = set_rc.write().remove(&args[0]); self.stack.push(Value::Bool(ok)); }
            MethodKind::Len | MethodKind::Count | MethodKind::Size => self.stack.push(Value::Int(set_rc.read().len() as i64)),
            MethodKind::Has | MethodKind::Contains => self.stack.push(Value::Bool(set_rc.read().contains(&args[0]))),
            MethodKind::IsEmpty  => self.stack.push(Value::Bool(set_rc.read().is_empty())),
            MethodKind::Clear    => { set_rc.write().clear(); self.stack.push(Value::Bool(true)); }
            MethodKind::Show     => { println!("{}", Value::Set(set_rc.clone())); self.stack.push(Value::Bool(true)); }
            _ => { eprintln!("Method {:?} not supported for Set{}", kind, self.current_span_info(ip)); return OpResult::Halt; }
        }
        OpResult::Continue
    }

    fn handle_string_method(&mut self, s: String, kind: MethodKind, args: Vec<Value>, ip: usize) -> OpResult {
        match kind {
            MethodKind::Length | MethodKind::Size => self.stack.push(Value::Int(s.chars().count() as i64)),
            MethodKind::Upper  => self.stack.push(Value::String(s.to_uppercase())),
            MethodKind::Lower  => self.stack.push(Value::String(s.to_lowercase())),
            MethodKind::Trim   => self.stack.push(Value::String(s.trim().to_string())),
            MethodKind::IndexOf => {
                if let Some(Value::String(sub)) = args.first() {
                    let idx = s.find(sub).map(|i| i as i64).unwrap_or(-1);
                    self.stack.push(Value::Int(idx));
                } else { self.stack.push(Value::Int(-1)); }
            }
            MethodKind::LastIndexOf => {
                if let Some(Value::String(sub)) = args.first() {
                    let idx = s.rfind(sub).map(|i| i as i64).unwrap_or(-1);
                    self.stack.push(Value::Int(idx));
                } else { self.stack.push(Value::Int(-1)); }
            }
            MethodKind::Replace => {
                if args.len() != 2 { return OpResult::Halt; }
                let from = args[0].to_string();
                let to   = args[1].to_string();
                if from.is_empty() { eprintln!("R307: .replace() called with empty 'from'{}", self.current_span_info(ip)); return OpResult::Halt; }
                self.stack.push(Value::String(s.replace(&from, &to)));
            }
            MethodKind::Slice => {
                if args.len() != 2 { return OpResult::Halt; }
                let start = if let Value::Int(i) = args[0] { i } else { return OpResult::Halt; };
                let end   = if let Value::Int(i) = args[1] { i } else { return OpResult::Halt; };
                let chars: Vec<char> = s.chars().collect();
                let len = chars.len() as i64;
                if start < 0 || end > len || start > end {
                    eprintln!("R303: String.slice out of bounds [{}, {}] for len {}{}", start, end, len, self.current_span_info(ip));
                    return OpResult::Halt;
                }
                self.stack.push(Value::String(chars[start as usize..end as usize].iter().collect()));
            }
            MethodKind::Split => {
                if args.is_empty() { return OpResult::Halt; }
                let sep = args[0].to_string();
                let parts: Vec<Value> = s.split(&sep).map(|p| Value::String(p.to_string())).collect();
                self.stack.push(Value::Array(Arc::new(RwLock::new(parts))));
            }
            MethodKind::StartsWith => {
                if args.is_empty() { return OpResult::Halt; }
                let prefix = args[0].to_string();
                self.stack.push(Value::Bool(s.starts_with(prefix.as_str())));
            }
            MethodKind::EndsWith => {
                if args.is_empty() { return OpResult::Halt; }
                let suffix = args[0].to_string();
                self.stack.push(Value::Bool(s.ends_with(suffix.as_str())));
            }
            MethodKind::ToInt => {
                match s.trim().parse::<i64>() {
                    Ok(n) => self.stack.push(Value::Int(n)),
                    Err(_) => {
                        eprintln!("halt.error: Cannot convert \"{}\" to Integer{}", s, self.current_span_info(ip));
                        return OpResult::Halt;
                    }
                }
            }
            MethodKind::ToFloat => {
                match s.trim().parse::<f64>() {
                    Ok(f) => self.stack.push(Value::Float(f)),
                    Err(_) => {
                        eprintln!("halt.error: Cannot convert \"{}\" to Float{}", s, self.current_span_info(ip));
                        return OpResult::Halt;
                    }
                }
            }
            _ => { eprintln!("Method {:?} not supported for String{}", kind, self.current_span_info(ip)); return OpResult::Halt; }
        }
        OpResult::Continue
    }

    fn handle_map_method(&mut self, map_rc: Arc<RwLock<Vec<(Value, Value)>>>, kind: MethodKind, args: Vec<Value>, ip: usize) -> OpResult {
        match kind {
            MethodKind::Get => {
                let key = &args[0];
                let map = map_rc.read();
                if let Some((_, v)) = map.iter().find(|(k, _)| k == key) {
                    self.stack.push(v.clone());
                } else {
                    eprintln!("R304: Map key not found: {}{}", key, self.current_span_info(ip));
                    return OpResult::Halt;
                }
            }
            MethodKind::Set | MethodKind::Insert => {
                let key = args[0].clone(); let val = args[1].clone();
                let mut map = map_rc.write();
                if let Some(e) = map.iter_mut().find(|(k, _)| *k == key) { e.1 = val; }
                else { map.push((key, val)); }
                self.stack.push(Value::Bool(true));
            }
            MethodKind::Len | MethodKind::Count | MethodKind::Size => self.stack.push(Value::Int(map_rc.read().len() as i64)),
            MethodKind::Keys => {
                let keys: Vec<Value> = map_rc.read().iter().map(|(k, _)| k.clone()).collect();
                self.stack.push(Value::Array(Arc::new(RwLock::new(keys))));
            }
            MethodKind::Values => {
                let vals: Vec<Value> = map_rc.read().iter().map(|(_, v)| v.clone()).collect();
                self.stack.push(Value::Array(Arc::new(RwLock::new(vals))));
            }
            MethodKind::Contains => {
                let has = map_rc.read().iter().any(|(k, _)| k == &args[0]);
                self.stack.push(Value::Bool(has));
            }
            MethodKind::Remove | MethodKind::Delete => {
                let key = &args[0];
                let mut map = map_rc.write();
                let before = map.len();
                map.retain(|(k, _)| k != key);
                self.stack.push(Value::Bool(map.len() < before));
            }
            MethodKind::Clear => { map_rc.write().clear(); self.stack.push(Value::Bool(true)); }
            MethodKind::Show  => { println!("{}", Value::Map(map_rc.clone())); self.stack.push(Value::Bool(true)); }
            _ => { eprintln!("Method {:?} not supported for Map{}", kind, self.current_span_info(ip)); return OpResult::Halt; }
        }
        OpResult::Continue
    }

    fn handle_table_method(&mut self, t_rc: Arc<RwLock<TableData>>, kind: MethodKind, args: Vec<Value>, ip: usize) -> OpResult {
        let t = t_rc.read();
        match kind {
            MethodKind::Count | MethodKind::Len | MethodKind::Size => {
                self.stack.push(Value::Int(t.rows.len() as i64));
            }
            MethodKind::Show => {
                for col in &t.columns { print!("{}\t", col.name); }
                println!();
                for row in &t.rows { for v in row { print!("{:?}\t", v); } println!(); }
                self.stack.push(Value::Bool(true));
            }
            MethodKind::Insert | MethodKind::Add => {
                drop(t);
                let mut t_mut = t_rc.write();
                let mut row = Vec::new();
                let mut ai = 0usize;
                let cols = t_mut.columns.clone();
                for col in &cols {
                    if col.is_auto {
                        let cidx = cols.iter().position(|c| c.name == col.name).unwrap();
                        let max = t_mut.rows.iter()
                            .filter_map(|r| if let Value::Int(i) = r[cidx] { Some(i) } else { None })
                            .max().unwrap_or(0);
                        row.push(Value::Int(max + 1));
                    } else {
                        row.push(args.get(ai).cloned().unwrap_or(Value::Bool(false)));
                        ai += 1;
                    }
                }
                t_mut.rows.push(row);
                self.stack.push(Value::Bool(true));
            }
            MethodKind::Update => {
                let idx = if let Value::Int(i) = args[0] { i } else { -1 };
                let vals = &args[1];
                drop(t);
                if idx >= 0 {
                    let mut t_mut = t_rc.write();
                    if (idx as usize) < t_mut.rows.len() {
                        if let Value::Array(arr_rc) = vals {
                            let arr = arr_rc.read();
                            let mut ai = 0usize;
                            for ci in 0..t_mut.columns.len() {
                                if !t_mut.columns[ci].is_auto {
                                    if ai < arr.len() {
                                        t_mut.rows[idx as usize][ci] = arr[ai].clone();
                                        ai += 1;
                                    }
                                }
                            }
                            self.stack.push(Value::Bool(true));
                        } else { self.stack.push(Value::Bool(false)); }
                    } else { self.stack.push(Value::Bool(false)); }
                } else { self.stack.push(Value::Bool(false)); }
            }
            MethodKind::Delete => {
                let idx = if let Value::Int(i) = args[0] { i } else { -1 };
                drop(t);
                if idx >= 0 {
                    let mut t_mut = t_rc.write();
                    if (idx as usize) < t_mut.rows.len() {
                        t_mut.rows.remove(idx as usize);
                        self.stack.push(Value::Bool(true));
                    } else { self.stack.push(Value::Bool(false)); }
                } else { self.stack.push(Value::Bool(false)); }
            }
            MethodKind::Where => {
                let filter_func = if let Value::Function(f) = args[0] { f } else { return OpResult::Halt; };
                let row_count = t.rows.len();
                drop(t);
                let mut filtered = Vec::new();
                for i in 0..row_count {
                    let mut run_args = vec![Value::Row(t_rc.clone(), i)];
                    run_args.extend_from_slice(&args[1..]);
                    if let Some(Value::Bool(true)) = self.run_frame(filter_func, &run_args) {
                        filtered.push(t_rc.read().rows[i].clone());
                    }
                }
                self.stack.push(Value::Table(Arc::new(RwLock::new(
                    TableData { columns: t_rc.read().columns.clone(), rows: filtered }
                ))));
            }
            MethodKind::Get => {
                let idx = if let Value::Int(i) = args[0] { i } else { -1 };
                if idx >= 0 && (idx as usize) < t.rows.len() {
                    self.stack.push(Value::Row(t_rc.clone(), idx as usize));
                } else {
                    eprintln!("R303: Table.get index out of bounds: {}{}", idx, self.current_span_info(ip));
                    return OpResult::Halt;
                }
            }
            MethodKind::Join => {
                if args.is_empty() { eprintln!("join: missing arguments{}", self.current_span_info(ip)); return OpResult::Halt; }
                let right_rc = match args[0].clone() {
                    Value::Table(r) => r,
                    _ => { eprintln!("join: first argument must be a table{}", self.current_span_info(ip)); return OpResult::Halt; }
                };
                let pred = if args.len() >= 3 {
                    match (args[1].clone(), args[2].clone()) {
                        (Value::String(lk), Value::String(rk)) => JoinPred::Keys(lk, rk),
                        _ => { eprintln!("join: key args must be strings{}", self.current_span_info(ip)); return OpResult::Halt; }
                    }
                } else if args.len() == 2 {
                    match args[1] {
                        Value::Function(fid) => JoinPred::Lambda(fid),
                        _ => { eprintln!("join: second arg must be a function{}", self.current_span_info(ip)); return OpResult::Halt; }
                    }
                } else {
                    eprintln!("join: requires 2 or 3 arguments{}", self.current_span_info(ip)); return OpResult::Halt;
                };
                let left_clone  = t.clone();
                let right_clone = right_rc.read().clone();
                drop(t);
                let result = join_tables(&left_clone, &right_clone, &pred, "b", self);
                self.stack.push(Value::Table(Arc::new(RwLock::new(result))));
            }
            MethodKind::Clear => {
                drop(t);
                t_rc.write().rows.clear();
                self.stack.push(Value::Bool(true));
            }
            _ => { eprintln!("Method {:?} not supported for Table{}", kind, self.current_span_info(ip)); return OpResult::Halt; }
        }
        OpResult::Continue
    }

    fn handle_row_method(&mut self, t_rc: Arc<RwLock<TableData>>, row_idx: usize, kind: MethodKind, ip: usize) -> OpResult {
        match kind {
            MethodKind::Show => {
                let t = t_rc.read();
                println!("{:?}", t.rows[row_idx]);
                self.stack.push(Value::Bool(true));
            }
            _ => {
                eprintln!("Method {:?} not supported for Row{}", kind, self.current_span_info(ip));
                return OpResult::Halt;
            }
        }
        OpResult::Continue
    }

    fn handle_row_custom(&mut self, t_rc: Arc<RwLock<TableData>>, row_idx: usize, method_name: &str, ip: usize) -> OpResult {
        let t = t_rc.read();
        if let Some(col_idx) = t.columns.iter().position(|c| c.name == method_name) {
            self.stack.push(t.rows[row_idx][col_idx].clone());
        } else {
            match method_name {
                "show" => { println!("{:?}", t.rows[row_idx]); self.stack.push(Value::Bool(true)); }
                _ => { eprintln!("Unknown Row member: {}{}", method_name, self.current_span_info(ip)); return OpResult::Halt; }
            }
        }
        OpResult::Continue
    }

    fn handle_date_method(&mut self, d: chrono::NaiveDateTime, kind: MethodKind, args: Vec<Value>, ip: usize) -> OpResult {
        use chrono::Datelike;
        use chrono::Timelike;
        match kind {
            MethodKind::Year   => self.stack.push(Value::Int(d.year() as i64)),
            MethodKind::Month  => self.stack.push(Value::Int(d.month() as i64)),
            MethodKind::Day    => self.stack.push(Value::Int(d.day() as i64)),
            MethodKind::Hour   => self.stack.push(Value::Int(d.hour() as i64)),
            MethodKind::Minute => self.stack.push(Value::Int(d.minute() as i64)),
            MethodKind::Second => self.stack.push(Value::Int(d.second() as i64)),
            MethodKind::Format => {
                let fmt_str = if let Some(Value::String(s)) = args.first() {
                    s.replace("YYYY", "%Y").replace("MM", "%m").replace("DD", "%d")
                     .replace("HH", "%H").replace("mm", "%M").replace("ss", "%S")
                } else {
                    "%Y-%m-%d %H:%M:%S".to_string()
                };
                self.stack.push(Value::String(d.format(&fmt_str).to_string()));
            }
            _ => { eprintln!("Method {:?} not supported for Date{}", kind, self.current_span_info(ip)); return OpResult::Halt; }
        }
        OpResult::Continue
    }

    // ── FIX: dynamic JSON field access ────────────────────────────────────────
    fn handle_json_method(&mut self, j_rc: Arc<RwLock<serde_json::Value>>, kind: MethodKind, args: Vec<Value>, ip: usize) -> OpResult {
        let mut j_mut = j_rc.write();
        match kind {
            MethodKind::Set | MethodKind::Insert => {
                if args.len() >= 2 {
                    if let Value::String(path) = &args[0] {
                        let val = value_to_json(&args[1]);
                        set_json_value_at_path(&mut j_mut, path, val);
                    }
                }
            }
            MethodKind::Push | MethodKind::Append => {
                if args.len() >= 2 {
                    if let Value::String(path) = &args[0] {
                        let val = value_to_json(&args[1]);
                        let pp = normalize_json_path(path);
                        if let Some(target) = j_mut.pointer_mut(&pp) {
                            if let Some(arr) = target.as_array_mut() {
                                arr.push(val);
                            }
                        }
                    }
                }
            }
            MethodKind::Count | MethodKind::Len | MethodKind::Size => {
                let n = j_mut.as_array().map(|a| a.len())
                    .or_else(|| j_mut.as_object().map(|o| o.len()))
                    .unwrap_or(0);
                self.stack.push(Value::Int(n as i64));
            }
            MethodKind::Exists => {
                if let Value::String(path) = &args[0] {
                    let pp = normalize_json_path(path);
                    let found = j_mut.pointer(&pp).map(|v| !v.is_null()).unwrap_or(false);
                    self.stack.push(Value::Bool(found));
                } else { self.stack.push(Value::Bool(false)); }
            }
            MethodKind::Get => {
                let path_storage;
                let path = if let Value::String(s) = &args[0] {
                    s.as_str()
                } else if let Value::Int(i) = &args[0] {
                    path_storage = format!("/{}", i);
                    &path_storage
                } else {
                    ""
                };
                let pp = normalize_json_path(path);
                if let Some(v) = j_mut.pointer(&pp) {
                    self.stack.push(json_serde_to_value(v));
                } else { self.stack.push(Value::Bool(false)); }
            }
            MethodKind::Inject => {
                if args.len() == 2 {
                    if let (Value::Map(m), Value::Table(t)) = (&args[0], &args[1]) {
                        inject_json_into_table(&mut t.write(), &j_mut, &m.read());
                        self.stack.push(Value::Bool(true));
                    } else { self.stack.push(Value::Bool(false)); }
                } else if args.len() == 3 {
                    if let (Value::String(key), Value::Map(m), Value::Table(t)) = (&args[0], &args[1], &args[2]) {
                        let pp = normalize_json_path(key);
                        let sub_json = j_mut.pointer(&pp).unwrap_or(&serde_json::Value::Null);
                        inject_json_into_table(&mut t.write(), sub_json, &m.read());
                        self.stack.push(Value::Bool(true));
                    } else { self.stack.push(Value::Bool(false)); }
                } else { self.stack.push(Value::Bool(false)); }
            }
            MethodKind::ToStr => self.stack.push(Value::String(j_mut.to_string())),
            _ => {
                eprintln!("Method {:?} not supported for JSON{}", kind, self.current_span_info(ip));
                return OpResult::Halt;
            }
        }
        OpResult::Continue
    }

    fn handle_json_custom(&mut self, j_rc: Arc<RwLock<serde_json::Value>>, field_name: &str, _args: Vec<Value>, _ip: usize) -> OpResult {
        let j_mut = j_rc.write();
        // First try as object key
        if let Some(v) = j_mut.get(field_name) {
            self.stack.push(json_serde_to_value(v));
        }
        // Then try array index if field_name is a number
        else if let Ok(idx) = field_name.parse::<usize>() {
            if let Some(v) = j_mut.get(idx) {
                self.stack.push(json_serde_to_value(v));
            } else {
                self.stack.push(Value::Bool(false));
            }
        }
        else {
            // Field not found — push false
            self.stack.push(Value::Bool(false));
        }
        OpResult::Continue
    }

    fn handle_fiber_method(&mut self, fiber_rc: Arc<RwLock<FiberState>>, kind: MethodKind, ip: usize) -> OpResult {
        match kind {
            MethodKind::Next => {
                let cached = fiber_rc.write().yielded_value.take();
                if let Some(val) = cached {
                    self.stack.push(val);
                } else if fiber_rc.read().is_done {
                    self.stack.push(Value::Bool(false));
                } else {
                    let res = self.resume_fiber(fiber_rc.clone(), true);
                    self.stack.push(res.unwrap_or(Value::Bool(false)));
                }
            }
            MethodKind::Run => {
                if !fiber_rc.read().is_done {
                    let cached = fiber_rc.write().yielded_value.take();
                    if cached.is_none() { self.resume_fiber(fiber_rc, false); }
                }
                self.stack.push(Value::Bool(true));
            }
            MethodKind::IsDone => {
                if fiber_rc.read().yielded_value.is_some() {
                    self.stack.push(Value::Bool(false));
                } else if fiber_rc.read().is_done {
                    self.stack.push(Value::Bool(true));
                } else {
                    let res = self.resume_fiber(fiber_rc.clone(), true);
                    if self.fiber_yielded {
                        fiber_rc.write().yielded_value = Some(res.unwrap_or(Value::Bool(false)));
                        self.stack.push(Value::Bool(false));
                    } else {
                        fiber_rc.write().yielded_value = Some(res.unwrap_or(Value::Bool(false)));
                        self.stack.push(Value::Bool(true));
                    }
                }
            }
            MethodKind::Close => {
                fiber_rc.write().is_done = true;
                self.stack.push(Value::Bool(true));
            }
            _ => { eprintln!("Method {:?} not supported for Fiber{}", kind, self.current_span_info(ip)); return OpResult::Halt; }
        }
        OpResult::Continue
    }

    // ── shared HTTP response builder ──────────────────────────────────────────────

    fn send_tiny_http_response(&mut self, request: tiny_http::Request, resp_json_rc: Arc<RwLock<serde_json::Value>>) {
        let resp_json = resp_json_rc.read();
        let (status, body_val, headers_val) = if let serde_json::Value::Object(m) = &*resp_json {
            let s = m.get("status").and_then(|v| v.as_u64()).unwrap_or(200) as u32;
            let b = m.get("body").cloned().unwrap_or(serde_json::Value::Null);
            let h = m.get("headers").cloned();
            (s, b, h)
        } else {
            (200, (*resp_json).clone(), None)
        };


        let body_str = match body_val {
            serde_json::Value::String(s) => s,
            other => other.to_string(),
        };

        let mut response = tiny_http::Response::from_string(body_str)
            .with_status_code(status);

        // Content-Type defaults to application/json but can be overridden
        let mut ct_set = false;
        if let Some(serde_json::Value::Object(h_map)) = headers_val {
            for (k, v) in h_map {
                let v_str = match v {
                    serde_json::Value::String(s) => s,
                    other => other.to_string(),
                };
                if k.to_lowercase() == "content-type" { ct_set = true; }
                if let Ok(h) = tiny_http::Header::from_bytes(k.as_bytes(), v_str.as_bytes()) {
                    response = response.with_header(h);
                }
            }
        }
        if !ct_set {
            response = response.with_header(tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap());
        }

        // Add standard CORS headers if not already set
        let has_header = |name: &str| -> bool {
            resp_json.as_object()
                .and_then(|m| m.get("headers"))
                .and_then(|h| h.as_object())
                .map(|h| h.keys().any(|k| k.to_lowercase() == name.to_lowercase()))
                .unwrap_or(false)
        };

        if !has_header("Access-Control-Allow-Origin") {
            response = response.with_header(tiny_http::Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap());
        }
        if !has_header("Access-Control-Allow-Methods") {
            response = response.with_header(tiny_http::Header::from_bytes(&b"Access-Control-Allow-Methods"[..], &b"GET, POST, PUT, DELETE, OPTIONS"[..]).unwrap());
        }
        if !has_header("Access-Control-Allow-Headers") {
            response = response.with_header(tiny_http::Header::from_bytes(&b"Access-Control-Allow-Headers"[..], &b"Content-Type, Authorization"[..]).unwrap());
        }

        let _ = request.respond(response);
    }
}

fn build_response_json(result: Result<ureq::Response, ureq::Error>) -> serde_json::Value {
    match result {
        Ok(resp) => {
            let status = resp.status();
            let mut h_map = serde_json::Map::new();
            for name in resp.headers_names() {
                if let Some(val) = resp.header(&name) {
                    h_map.insert(name, serde_json::Value::String(val.to_string()));
                }
            }
            let text = resp.into_string().unwrap_or_default();
            if text.len() > 10 * 1024 * 1024 {
                let mut res = serde_json::Map::new();
                res.insert("status".to_string(), serde_json::Value::Number(413.into()));
                res.insert("ok".to_string(),     serde_json::Value::Bool(false));
                res.insert("error".to_string(),  serde_json::Value::String("Body too large".to_string()));
                serde_json::Value::Object(res)
            } else {
                let body_val = serde_json::from_str(&text).unwrap_or(serde_json::Value::String(text));
                let mut res = serde_json::Map::new();
                res.insert("status".to_string(),  serde_json::Value::Number(status.into()));
                res.insert("ok".to_string(),      serde_json::Value::Bool(status >= 200 && status < 300));
                res.insert("body".to_string(),    body_val);
                res.insert("headers".to_string(), serde_json::Value::Object(h_map));
                serde_json::Value::Object(res)
            }
        }
        Err(e) => {
            let mut res = serde_json::Map::new();
            res.insert("status".to_string(), serde_json::Value::Number(0.into()));
            res.insert("ok".to_string(),     serde_json::Value::Bool(false));
            res.insert("error".to_string(),  serde_json::Value::String(e.to_string()));
            serde_json::Value::Object(res)
        }
    }
}

/// Convert a serde_json::Value to a VM Value — preserving nested JSON objects.
fn json_serde_to_value(v: &serde_json::Value) -> Value {
    match v {
        serde_json::Value::Null    => Value::Bool(false),
        serde_json::Value::Bool(b) => Value::Bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() { Value::Int(i) }
            else if let Some(f) = n.as_f64() { Value::Float(f) }
            else { Value::Int(0) }
        }
        serde_json::Value::String(s) => Value::String(s.clone()),
        // Arrays and objects stay as Json for further dot-access
        other => Value::Json(Arc::new(RwLock::new(other.clone()))),
    }
}

fn json_value_to_typed_value_raw(v: &Value, target: &Value) -> Value {
    // If we have a Json wrapper, unwrap it first for the logic below
    let inner_json = if let Value::Json(j) = v {
        Some(j.read().clone())
    } else {
        None
    };

    match target {
        Value::Int(_) => {
            match v {
                Value::Int(i) => Value::Int(*i),
                Value::Float(f) => Value::Int(*f as i64),
                Value::String(s) => s.parse::<i64>().map(Value::Int).unwrap_or(Value::Int(0)),
                Value::Json(_) => {
                    let j = inner_json.unwrap();
                    if let Some(i) = j.as_i64() { Value::Int(i) }
                    else if let Some(f) = j.as_f64() { Value::Int(f as i64) }
                    else { Value::Int(0) }
                }
                _ => Value::Int(0),
            }
        }
        Value::Float(_) => {
            match v {
                Value::Float(f) => Value::Float(*f),
                Value::Int(i) => Value::Float(*i as f64),
                Value::String(s) => s.parse::<f64>().map(Value::Float).unwrap_or(Value::Float(0.0)),
                Value::Json(_) => {
                    let j = inner_json.unwrap();
                    if let Some(f) = j.as_f64() { Value::Float(f) }
                    else if let Some(i) = j.as_i64() { Value::Float(i as f64) }
                    else { Value::Float(0.0) }
                }
                _ => Value::Float(0.0),
            }
        }
        Value::Array(_) => {
            if let Some(j) = inner_json {
                if let Some(arr) = j.as_array() {
                    let mut vec = Vec::with_capacity(arr.len());
                    for item in arr {
                        vec.push(json_serde_to_value(item));
                    }
                    return Value::Array(Arc::new(RwLock::new(vec)));
                }
            }
            v.clone()
        }
        Value::String(_) => {
             match v {
                 Value::String(s) => Value::String(s.clone()),
                 Value::Int(i) => Value::String(i.to_string()),
                 Value::Float(f) => Value::String(f.to_string()),
                 Value::Bool(b) => Value::String(b.to_string()),
                 Value::Json(j) => Value::String(j.read().to_string()),
                 _ => Value::String("".to_string()),
             }
        }
        Value::Bool(_) => {
            match v {
                Value::Bool(b) => Value::Bool(*b),
                Value::Int(i) => Value::Bool(*i != 0),
                Value::Json(j) => Value::Bool(j.read().as_bool().unwrap_or(false)),
                _ => Value::Bool(false),
            }
        }
        _ => v.clone(),
    }
}

// ── public helpers ────────────────────────────────────────────────────────────

pub fn is_safe_path(path_str: &str) -> bool {
    let path = std::path::Path::new(path_str);
    if path.is_absolute() { return false; }
    path.components().all(|c| !matches!(c, std::path::Component::ParentDir))
}

pub fn normalize_json_path(path: &str) -> String {
    if path.is_empty() { return String::new(); }
    let mut p = path.replace('.', "/").replace('[', "/").replace(']', "");
    if !p.starts_with('/') { p.insert(0, '/'); }
    p
}

fn set_json_value_at_path(target: &mut serde_json::Value, path: &str, value: serde_json::Value) {
    let pointer = normalize_json_path(path);
    let parts: Vec<&str> = pointer.split('/').filter(|s| !s.is_empty()).collect();
    
    if parts.is_empty() {
        *target = value;
        return;
    }

    let mut current = target;
    for (i, part) in parts.iter().enumerate() {
        let is_last = i == parts.len() - 1;
        
        if let Ok(idx) = part.parse::<usize>() {
            if !current.is_array() {
                *current = serde_json::Value::Array(Vec::new());
            }
            let arr = current.as_array_mut().unwrap();
            while arr.len() <= idx {
                arr.push(serde_json::Value::Null);
            }
            if is_last {
                arr[idx] = value;
                return;
            }
            current = &mut arr[idx];
        } else {
            if !current.is_object() {
                *current = serde_json::Value::Object(serde_json::Map::new());
            }
            let obj = current.as_object_mut().unwrap();
            if is_last {
                obj.insert(part.to_string(), value);
                return;
            }
            
            // Peek next part to see if we should create an array or object for missing path
            let next_is_array = if i + 1 < parts.len() {
                parts[i+1].parse::<usize>().is_ok()
            } else {
                false
            };

            current = obj.entry(part.to_string()).or_insert_with(|| {
                if next_is_array {
                    serde_json::Value::Array(Vec::new())
                } else {
                    serde_json::Value::Object(serde_json::Map::new())
                }
            });
        }
    }
}

pub fn value_to_json(v: &Value) -> serde_json::Value {
    match v {
        Value::Int(i)    => serde_json::Value::Number((*i).into()),
        Value::Float(f)  => serde_json::Number::from_f64(*f).map(serde_json::Value::Number).unwrap_or(serde_json::Value::Null),
        Value::String(s) => serde_json::Value::String(s.clone()),
        Value::Bool(b)   => serde_json::Value::Bool(*b),
        Value::Array(arr) => {
            let a = arr.read();
            serde_json::Value::Array(a.iter().map(value_to_json).collect())
        }
        Value::Map(m) => {
            let b = m.read();
            let mut obj = serde_json::Map::new();
            for (k, v) in b.iter() { obj.insert(k.to_string(), value_to_json(v)); }
            serde_json::Value::Object(obj)
        }
        Value::Json(j)  => j.read().clone(),
        Value::Date(d)  => serde_json::Value::String(d.format("%Y-%m-%d").to_string()),
        _               => serde_json::Value::Null,
    }
}


/// Decode percent-encoded URL components (%20 → space, %2F → /, etc.)
fn url_decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex_digit(bytes[i+1]), hex_digit(bytes[i+2])) {
                out.push(char::from(h * 16 + l));
                i += 3;
                continue;
            }
        }
        if bytes[i] == b'+' { out.push(' '); } else { out.push(char::from(bytes[i])); }
        i += 1;
    }
    out
}

fn hex_digit(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

pub fn is_safe_url(url_str: &str) -> Result<(), String> {
    if url_str.starts_with("file://") {
        return Err("HALT.FATAL: SSRF - file:// URLs are forbidden".to_string());
    }
    
    // Basic host extraction for SSRF checks
    let host = if let Some(start) = url_str.find("://") {
        let remainder = &url_str[start+3..];
        let end = remainder.find('/').unwrap_or(remainder.len());
        let mut host_port = &remainder[..end];
        if let Some(p) = host_port.find('@') { host_port = &host_port[p+1..]; } // strip user:pass
        if let Some(p) = host_port.find(':') { host_port = &host_port[..p]; } // strip port
        host_port.to_lowercase()
    } else {
        url_str.to_lowercase()
    };

    if host == "169.254.169.254" || host.starts_with("169.254.") {
        return Err("HALT.FATAL: SSRF - Link-local addresses are forbidden".to_string());
    }

    let is_localhost = host == "localhost" || host == "127.0.0.1" || host == "::1";

    if !is_localhost {
        if host.starts_with("10.") || 
           host.starts_with("192.168.") ||
           host.starts_with("172.16.") || host.starts_with("172.17.") ||
           host.starts_with("172.18.") || host.starts_with("172.19.") ||
           host.starts_with("172.20.") || host.starts_with("172.21.") ||
           host.starts_with("172.22.") || host.starts_with("172.23.") ||
           host.starts_with("172.24.") || host.starts_with("172.25.") ||
           host.starts_with("172.26.") || host.starts_with("172.27.") ||
           host.starts_with("172.28.") || host.starts_with("172.29.") ||
           host.starts_with("172.30.") || host.starts_with("172.31.") {
            return Err("HALT.ERROR: SSRF - Private IP ranges are blocked in production".to_string());
        }
    }
    Ok(())
}