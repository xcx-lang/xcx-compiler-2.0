pub mod vm;
pub mod repl;

#[cfg(test)]
mod tests;

use crate::parser::ast::{Program, Stmt, Expr};
use crate::backend::vm::{OpCode, Value, FunctionChunk};
use crate::lexer::token::TokenKind;
use crate::sema::interner::{Interner, StringId};
use std::collections::HashMap;

pub struct Compiler {
    pub globals: HashMap<StringId, usize>,
    pub func_indices: HashMap<StringId, usize>,
    pub functions: Vec<FunctionChunk>,
    pub constants: Vec<Value>,
}

pub struct CompileContext<'a> {
    pub constants: &'a mut Vec<Value>,
    pub functions: &'a mut Vec<FunctionChunk>,
    pub func_indices: &'a HashMap<StringId, usize>,
    pub globals: &'a HashMap<StringId, usize>,
    pub interner: &'a mut Interner,
}

impl<'a> CompileContext<'a> {
    pub fn add_constant(&mut self, val: Value) -> usize {
        self.constants.push(val);
        self.constants.len() - 1
    }
}

pub struct FunctionCompiler {
    pub bytecode: Vec<OpCode>,
    pub scopes: Vec<HashMap<StringId, usize>>,
    pub next_local: usize,
    pub loop_stack: Vec<(usize, Vec<usize>, Vec<usize>, Option<usize>)>,
    pub has_return_type: bool,
    pub parent_locals: Option<HashMap<StringId, usize>>,
    pub captures: Vec<StringId>,
    pub is_main: bool,
    pub is_table_lambda: bool,
}

impl FunctionCompiler {
    pub fn new(is_main: bool, parent_locals: Option<HashMap<StringId, usize>>) -> Self {
        Self {
            bytecode: Vec::new(),
            scopes: vec![HashMap::new()],
            next_local: 0,
            loop_stack: Vec::new(),
            has_return_type: false,
            parent_locals,
            captures: Vec::new(),
            is_main,
            is_table_lambda: false,
        }
    }

    pub fn enter_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn exit_scope(&mut self) {
        self.scopes.pop();
    }

    fn lookup_local(&self, id: &StringId) -> Option<usize> {
        for scope in self.scopes.iter().rev() {
            if let Some(&slot) = scope.get(id) {
                return Some(slot);
            }
        }
        None
    }

    fn define_local(&mut self, id: StringId, slot: usize) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(id, slot);
        }
    }

    fn convert_to_flat_locals(&self) -> HashMap<StringId, usize> {
        let mut flat = HashMap::new();
        for scope in &self.scopes {
            for (&id, &slot) in scope {
                flat.insert(id, slot);
            }
        }
        flat
    }

    pub fn compile_expr(&mut self, expr: &Expr, ctx: &mut CompileContext) {
        match &expr.kind {
            crate::parser::ast::ExprKind::IntLiteral(v) => {
                let i = ctx.add_constant(Value::Int(*v));
                self.bytecode.push(OpCode::Constant(i));
            }
            crate::parser::ast::ExprKind::FloatLiteral(v) => {
                let i = ctx.add_constant(Value::Float(*v));
                self.bytecode.push(OpCode::Constant(i));
            }
            crate::parser::ast::ExprKind::StringLiteral(id) => {
                let s = ctx.interner.lookup(*id).to_string();
                let i = ctx.add_constant(Value::String(s));
                self.bytecode.push(OpCode::Constant(i));
            }
            crate::parser::ast::ExprKind::BoolLiteral(v) => {
                let i = ctx.add_constant(Value::Bool(*v));
                self.bytecode.push(OpCode::Constant(i));
            }
            crate::parser::ast::ExprKind::Identifier(id) => {
                if let Some(slot) = self.lookup_local(id) {
                    self.bytecode.push(OpCode::GetLocal(slot));
                } else if let Some(&idx) = ctx.globals.get(id) {
                    self.bytecode.push(OpCode::GetVar(idx));
                } else if let Some(&fid) = ctx.func_indices.get(id) {
                    let i = ctx.add_constant(Value::Function(fid));
                    self.bytecode.push(OpCode::Constant(i));
                } else if let Some(&_parent_slot) = self.parent_locals.as_ref().and_then(|p| p.get(id)) {
                    let slot = self.next_local;
                    self.define_local(*id, slot);
                    self.next_local += 1;
                    self.captures.push(*id);
                    self.bytecode.push(OpCode::GetLocal(slot));
                } else if self.is_table_lambda {
                    self.bytecode.push(OpCode::GetLocal(0));
                    let mi = ctx.add_constant(Value::String(ctx.interner.lookup(*id).to_string()));
                    self.bytecode.push(OpCode::MethodCall(mi, 0));
                } else {
                    let name = ctx.interner.lookup(*id).to_string();
                    let i = ctx.add_constant(Value::String(name));
                    self.bytecode.push(OpCode::Constant(i));
                }
            }
            crate::parser::ast::ExprKind::FunctionCall { name, args } => {
                let n = ctx.interner.lookup(*name);
                if n == "json.parse" && args.len() == 1 {
                    self.compile_expr(&args[0], ctx);
                    self.bytecode.push(OpCode::JsonParse);
                } else if n == "terminal.input" {
                    self.bytecode.push(OpCode::Input);
                } else if n == "i" && args.len() == 1 {
                    self.compile_expr(&args[0], ctx);
                    self.bytecode.push(OpCode::CastInt);
                } else if n == "f" && args.len() == 1 {
                    self.compile_expr(&args[0], ctx);
                    self.bytecode.push(OpCode::CastFloat);
                } else if n == "s" && args.len() == 1 {
                    self.compile_expr(&args[0], ctx);
                    self.bytecode.push(OpCode::CastString);
                } else if n == "b" && args.len() == 1 {
                    self.compile_expr(&args[0], ctx);
                    self.bytecode.push(OpCode::CastBool);
                } else {
                    for arg in args {
                        self.compile_expr(arg, ctx);
                    }
                    if let Some(&fid) = ctx.func_indices.get(name) {
                        if ctx.functions[fid].is_fiber {
                            self.bytecode.push(OpCode::FiberCreate(fid, args.len()));
                        } else {
                            self.bytecode.push(OpCode::Call(fid, args.len()));
                        }
                    }
                }
            }
            crate::parser::ast::ExprKind::ArrayLiteral { elements } => {
                for e in elements {
                    self.compile_expr(e, ctx);
                }
                self.bytecode.push(OpCode::ArrayInit(elements.len()));
            }
            crate::parser::ast::ExprKind::SetLiteral { elements, range, .. } => {
                if let Some(r) = range {
                    self.compile_expr(&r.start, ctx);
                    self.compile_expr(&r.end, ctx);
                    if let Some(s) = &r.step {
                        self.compile_expr(s, ctx);
                        let t = ctx.add_constant(Value::Bool(true));
                        self.bytecode.push(OpCode::Constant(t));
                    } else {
                        let f = ctx.add_constant(Value::Bool(false));
                        self.bytecode.push(OpCode::Constant(f));
                    }
                    self.bytecode.push(OpCode::SetRange);
                } else {
                    for e in elements {
                        self.compile_expr(e, ctx);
                    }
                    self.bytecode.push(OpCode::SetInit(elements.len()));
                }
            }
            crate::parser::ast::ExprKind::MapLiteral { elements, .. } => {
                for (k, v) in elements {
                    self.compile_expr(k, ctx);
                    self.compile_expr(v, ctx);
                }
                self.bytecode.push(OpCode::MapInit(elements.len()));
            }
            crate::parser::ast::ExprKind::RandomChoice { set } => {
                self.compile_expr(set, ctx);
                self.bytecode.push(OpCode::RandomChoice);
            }
            crate::parser::ast::ExprKind::DateLiteral { date_string, format } => {
                let date_str = ctx.interner.lookup(*date_string).to_string();
                let date = if let Some(fmt_id) = format {
                    let fmt_str = ctx.interner.lookup(*fmt_id).to_string();
                    let chrono_fmt = fmt_str
                        .replace("YYYY", "%Y").replace("MM", "%m").replace("DD", "%d")
                        .replace("M", "%-m").replace("D", "%-d");
                    chrono::NaiveDate::parse_from_str(&date_str, &chrono_fmt)
                        .unwrap_or_else(|_| chrono::NaiveDate::from_ymd_opt(1970, 1, 1).unwrap())
                } else {
                    chrono::NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                        .unwrap_or_else(|_| chrono::NaiveDate::from_ymd_opt(1970, 1, 1).unwrap())
                };
                let dt = date.and_hms_opt(0, 0, 0).unwrap();
                let i = ctx.add_constant(Value::Date(dt));
                self.bytecode.push(OpCode::Constant(i));
            }
            crate::parser::ast::ExprKind::MethodCall { receiver, method, args } => {
                let method_name = ctx.interner.lookup(*method).to_string();
                let mut is_store = false;
                let mut is_date  = false;
                let mut is_json  = false;
                let mut is_env   = false;
                let mut is_crypto = false;
                if let crate::parser::ast::ExprKind::Identifier(rid) = &receiver.kind {
                    let rname = ctx.interner.lookup(*rid);
                    if rname == "store" { is_store = true; }
                    if rname == "date"  { is_date  = true; }
                    if rname == "json"  { is_json  = true; }
                    if rname == "env"   { is_env   = true; }
                    if rname == "crypto" { is_crypto = true; }
                }

                // ── store.* ────────────────────────────────────────────────────
                if is_store {
                    for arg in args { self.compile_expr(arg, ctx); }
                    match method_name.as_str() {
                        "write"  => self.bytecode.push(OpCode::StoreWrite),
                        "read"   => self.bytecode.push(OpCode::StoreRead),
                        "append" => self.bytecode.push(OpCode::StoreAppend),
                        "exists" => self.bytecode.push(OpCode::StoreExists),
                        "delete" => self.bytecode.push(OpCode::StoreDelete),
                        _ => {}
                    }
                    return;
                }
                // ── date.now() ─────────────────────────────────────────────────
                if is_date && method_name == "now" {
                    self.bytecode.push(OpCode::DateNow);
                    return;
                }
                // ── json.parse() ───────────────────────────────────────────────
                if is_json && method_name == "parse" {
                    self.compile_expr(&args[0], ctx);
                    self.bytecode.push(OpCode::JsonParse);
                    return;
                }
                // ── env.get("VAR") / env.args() ────────────────────────────────
                if is_env {
                    if method_name == "get" {
                        if let Some(arg) = args.first() {
                            self.compile_expr(arg, ctx);
                        }
                        self.bytecode.push(OpCode::EnvGet);
                    } else if method_name == "args" {
                        self.bytecode.push(OpCode::EnvArgs);
                    }
                    return;
                }

                // ── crypto.* ───────────────────────────────────────────────────
                if is_crypto {
                    for arg in args {
                        self.compile_expr(arg, ctx);
                    }
                    match method_name.as_str() {
                        "hash"   => self.bytecode.push(OpCode::CryptoHash),
                        "verify" => self.bytecode.push(OpCode::CryptoVerify),
                        "token"  => self.bytecode.push(OpCode::CryptoToken),
                        _ => {} // Fallback to normal method call if unknown
                    }
                    return;
                }

                // ── table.where(pred) ──────────────────────────────────────────
                if method_name == "where" && args.len() == 1 {
                    self.compile_expr(receiver, ctx);
                    self.compile_lambda_method(method_name, &args[0], ctx);
                    return;
                }
                if method_name == "join" && args.len() >= 2 {
                    self.compile_expr(receiver, ctx);

                    // Always compile the right table as first arg
                    self.compile_expr(&args[0], ctx);

                    let second = &args[1];
                    if let crate::parser::ast::ExprKind::Lambda { params, body, .. } = &second.kind {
                        let lambda_params: Vec<(crate::parser::ast::Type, StringId)> = params.clone();

                        let mut lambda_fc = FunctionCompiler::new(false, Some(self.convert_to_flat_locals()));
                        lambda_fc.is_table_lambda = true;
                        for (i, (_, pname)) in lambda_params.iter().enumerate() {
                            lambda_fc.define_local(*pname, i);
                        }
                        lambda_fc.next_local = lambda_params.len();

                        let ret_stmt = crate::parser::ast::Stmt {
                            kind: crate::parser::ast::StmtKind::Return(Some(*body.clone())),
                            span: second.span.clone(),
                        };
                        lambda_fc.compile_stmt(&ret_stmt, ctx);

                        let chunk = FunctionChunk {
                            bytecode: lambda_fc.bytecode,
                            is_fiber: false,
                            max_locals: lambda_fc.next_local,
                        };
                        let fid = ctx.functions.len();
                        ctx.functions.push(chunk);

                        let ci = ctx.add_constant(Value::Function(fid));
                        self.bytecode.push(OpCode::Constant(ci));

                        // Reify captures
                        for captured_id in &lambda_fc.captures {
                            if let Some(outer_slot) = self.lookup_local(captured_id) {
                                self.bytecode.push(OpCode::GetLocal(outer_slot));
                            }
                        }

                        let mi = ctx.add_constant(Value::String("join".to_string()));
                        self.bytecode.push(OpCode::MethodCall(mi, 2 + lambda_fc.captures.len()));
                    } else {
                        // Key-based join: second arg is left-col string, third is right-col string
                        self.compile_expr(second, ctx);
                        if args.len() >= 3 {
                            self.compile_expr(&args[2], ctx);
                            let mi = ctx.add_constant(Value::String("join".to_string()));
                            self.bytecode.push(OpCode::MethodCall(mi, 3));
                        } else {
                            // Only 2 args — treat second as a function value expression
                            let mi = ctx.add_constant(Value::String("join".to_string()));
                            self.bytecode.push(OpCode::MethodCall(mi, 2));
                        }
                    }
                    return;
                }

                // ── general method call ────────────────────────────────────────
                self.compile_expr(receiver, ctx);
                for arg in args {
                    self.compile_expr(arg, ctx);
                }

                match method_name.as_str() {
                    "next"   => self.bytecode.push(OpCode::FiberNext),
                    "run"    => self.bytecode.push(OpCode::FiberRun),
                    "isDone" => self.bytecode.push(OpCode::FiberIsDone),
                    "close"  => self.bytecode.push(OpCode::FiberClose),
                    _ => {
                        let mi = ctx.add_constant(Value::String(method_name));
                        self.bytecode.push(OpCode::MethodCall(mi, args.len()));
                    }
                }
            }
            crate::parser::ast::ExprKind::Binary { left, op, right } => {
                self.compile_expr(left, ctx);
                self.compile_expr(right, ctx);
                match op {
                    TokenKind::Plus         => self.bytecode.push(OpCode::Add),
                    TokenKind::Minus        => self.bytecode.push(OpCode::Sub),
                    TokenKind::Star         => self.bytecode.push(OpCode::Mul),
                    TokenKind::Slash        => self.bytecode.push(OpCode::Div),
                    TokenKind::Percent      => self.bytecode.push(OpCode::Mod),
                    TokenKind::Caret        => self.bytecode.push(OpCode::Pow),
                    TokenKind::EqualEqual   => self.bytecode.push(OpCode::Equal),
                    TokenKind::BangEqual    => self.bytecode.push(OpCode::NotEqual),
                    TokenKind::Greater      => self.bytecode.push(OpCode::Greater),
                    TokenKind::Less         => self.bytecode.push(OpCode::Less),
                    TokenKind::GreaterEqual => self.bytecode.push(OpCode::GreaterEqual),
                    TokenKind::LessEqual    => self.bytecode.push(OpCode::LessEqual),
                    TokenKind::And          => self.bytecode.push(OpCode::And),
                    TokenKind::Or           => self.bytecode.push(OpCode::Or),
                    TokenKind::Has          => self.bytecode.push(OpCode::Has),
                    TokenKind::Union        => self.bytecode.push(OpCode::SetUnion),
                    TokenKind::Intersection => self.bytecode.push(OpCode::SetIntersection),
                    TokenKind::Difference   => self.bytecode.push(OpCode::SetDifference),
                    TokenKind::SymDifference => self.bytecode.push(OpCode::SetSymDifference),
                    TokenKind::PlusPlus     => self.bytecode.push(OpCode::IntConcat),
                    _ => {}
                }
            }
            crate::parser::ast::ExprKind::Unary { op, right } => {
                match op {
                    TokenKind::Not | TokenKind::Bang => {
                        self.compile_expr(right, ctx);
                        self.bytecode.push(OpCode::Not);
                    }
                    TokenKind::Minus => {
                        let zero = if matches!(right.kind, crate::parser::ast::ExprKind::FloatLiteral(_)) {
                            ctx.add_constant(Value::Float(0.0))
                        } else {
                            ctx.add_constant(Value::Int(0))
                        };
                        self.bytecode.push(OpCode::Constant(zero));
                        self.compile_expr(right, ctx);
                        self.bytecode.push(OpCode::Sub);
                    }
                    _ => {}
                }
            }
            crate::parser::ast::ExprKind::Index { receiver, index } => {
                self.compile_expr(receiver, ctx);
                self.compile_expr(index, ctx);
                let gi = ctx.add_constant(Value::String("get".to_string()));
                self.bytecode.push(OpCode::MethodCall(gi, 1));
            }
            crate::parser::ast::ExprKind::Lambda { .. } => {
                // Lambdas are compiled when they are passed as arguments to methods like .where() or .join()
                // or when they are assigned to variables.
                // If a lambda appears as a standalone expression, it evaluates to a function value.
                let f = ctx.add_constant(Value::Bool(false)); // Placeholder for now, actual function value will be created by FunctionCompiler
                self.bytecode.push(OpCode::Constant(f));
            }
            crate::parser::ast::ExprKind::Tuple(exprs) => {
                for e in exprs { self.compile_expr(e, ctx); }
                self.bytecode.push(OpCode::ArrayInit(exprs.len()));
            }
            crate::parser::ast::ExprKind::ArrayOrSetLiteral { elements } => {
                for e in elements { self.compile_expr(e, ctx); }
                self.bytecode.push(OpCode::ArrayInit(elements.len()));
            }
            crate::parser::ast::ExprKind::TerminalCommand(cmd_id, arg) => {
                let cmd = ctx.interner.lookup(*cmd_id);
                if cmd == "exit"       { self.bytecode.push(OpCode::TerminalExit); }
                else if cmd == "clear" { self.bytecode.push(OpCode::TerminalClear); }
                else if cmd == "run"   {
                    if let Some(a) = arg {
                        self.compile_expr(a, ctx);
                        self.bytecode.push(OpCode::TerminalRun);
                    }
                }
            }
            crate::parser::ast::ExprKind::MemberAccess { receiver, member } => {
                self.compile_expr(receiver, ctx);
                let mi = ctx.add_constant(Value::String(ctx.interner.lookup(*member).to_string()));
                self.bytecode.push(OpCode::MethodCall(mi, 0));
            }
            crate::parser::ast::ExprKind::TableLiteral { columns, rows } => {
                for row in rows { for val in row { self.compile_expr(val, ctx); } }
                let vm_cols = columns.iter().map(|c| crate::backend::vm::VMColumn {
                    name: ctx.interner.lookup(c.name).to_string(),
                    ty: c.ty.clone(),
                    is_auto: c.is_auto,
                }).collect();
                let skeleton = Value::Table(std::rc::Rc::new(std::cell::RefCell::new(
                    crate::backend::vm::TableData { columns: vm_cols, rows: Vec::new() }
                )));
                let ci = ctx.add_constant(skeleton);
                self.bytecode.push(OpCode::TableInit(ci, rows.len()));
            }
            crate::parser::ast::ExprKind::RawBlock(id) => {
                let s = ctx.interner.lookup(*id).to_string();
                let i = ctx.add_constant(Value::String(s));
                self.bytecode.push(OpCode::Constant(i));
            }
            crate::parser::ast::ExprKind::NetCall { method, url, body } => {
                self.compile_expr(url, ctx);
                if let Some(b) = body {
                    self.compile_expr(b, ctx);
                } else {
                    let f = ctx.add_constant(Value::Bool(false));
                    self.bytecode.push(OpCode::Constant(f));
                }
                let mi = ctx.add_constant(Value::String(ctx.interner.lookup(*method).to_string()));
                self.bytecode.push(OpCode::HttpCall(mi));
            }
            crate::parser::ast::ExprKind::NetRespond { status, body, headers } => {
                self.compile_expr(status, ctx);
                self.compile_expr(body, ctx);
                if let Some(h) = headers {
                    self.compile_expr(h, ctx);
                } else {
                    let f = ctx.add_constant(Value::Bool(false));
                    self.bytecode.push(OpCode::Constant(f));
                }
                self.bytecode.push(OpCode::HttpRespond);
            }
        }
    }

    /// Helper: compile a single-argument lambda/expr predicate into an anonymous
    /// function, then emit MethodCall(method_name, 1).
    /// Used by `.where()` and can be reused for similar single-pred methods.
    fn compile_lambda_method(
        &mut self,
        method_name: String,
        pred_expr: &Expr,
        ctx: &mut CompileContext,
    ) {
        let (lambda_body, lambda_params) =
            if let crate::parser::ast::ExprKind::Lambda { params, body, .. } = &pred_expr.kind {
                (
                    *body.clone(),
                    params.clone(),
                )
            } else {
                let row_tmp = ctx.interner.intern("__row_tmp");
                (pred_expr.clone(), vec![(crate::parser::ast::Type::Int, row_tmp)])
            };

        let mut lambda_fc = FunctionCompiler::new(false, Some(self.convert_to_flat_locals()));
        lambda_fc.is_table_lambda = true;
        for (i, (_, pname)) in lambda_params.iter().enumerate() {
            lambda_fc.define_local(*pname, i);
        }
        lambda_fc.next_local = lambda_params.len();

        let ret_stmt = crate::parser::ast::Stmt {
            kind: crate::parser::ast::StmtKind::Return(Some(lambda_body)),
            span: pred_expr.span.clone(),
        };
        lambda_fc.compile_stmt(&ret_stmt, ctx);

        let chunk = FunctionChunk {
            bytecode: lambda_fc.bytecode,
            is_fiber: false,
            max_locals: lambda_fc.next_local,
        };
        let fid = ctx.functions.len();
        ctx.functions.push(chunk);

        let ci = ctx.add_constant(Value::Function(fid));
        self.bytecode.push(OpCode::Constant(ci));

        // Reify captures from outer scope
        for captured_id in &lambda_fc.captures {
            if let Some(outer_slot) = self.lookup_local(captured_id) {
                self.bytecode.push(OpCode::GetLocal(outer_slot));
            }
        }

        let mi = ctx.add_constant(Value::String(method_name));
        self.bytecode.push(OpCode::MethodCall(mi, 1 + lambda_fc.captures.len()));
    }

    pub fn compile_stmt(&mut self, stmt: &Stmt, ctx: &mut CompileContext) {
        match &stmt.kind {
            crate::parser::ast::StmtKind::VarDecl { ty, name, value, .. } => {
                if let Some(val) = value {
                    self.compile_expr(val, ctx);
                } else {
                    let default_val = match ty {
                        crate::parser::ast::Type::Int    => Value::Int(0),
                        crate::parser::ast::Type::Float  => Value::Float(0.0),
                        crate::parser::ast::Type::String => Value::String("".to_string()),
                        crate::parser::ast::Type::Bool   => Value::Bool(false),
                        crate::parser::ast::Type::Array(_) => Value::Array(
                            std::rc::Rc::new(std::cell::RefCell::new(Vec::new()))
                        ),
                        crate::parser::ast::Type::Set(_) => Value::Set(
                            std::rc::Rc::new(std::cell::RefCell::new(std::collections::BTreeSet::new()))
                        ),
                        crate::parser::ast::Type::Map(_, _) => Value::Map(
                            std::rc::Rc::new(std::cell::RefCell::new(Vec::new()))
                        ),
                        crate::parser::ast::Type::Date => Value::Date(
                            chrono::DateTime::from_timestamp(0, 0).unwrap().naive_utc()
                        ),
                        crate::parser::ast::Type::Table(cols) => {
                            let vm_cols = cols.iter().map(|c| crate::backend::vm::VMColumn {
                                name: ctx.interner.lookup(c.name).to_string(),
                                ty: c.ty.clone(),
                                is_auto: c.is_auto,
                            }).collect();
                            Value::Table(std::rc::Rc::new(std::cell::RefCell::new(
                                crate::backend::vm::TableData { columns: vm_cols, rows: Vec::new() }
                            )))
                        }
                        crate::parser::ast::Type::Json => Value::Json(
                            std::rc::Rc::new(std::cell::RefCell::new(serde_json::Value::Null))
                        ),
                        crate::parser::ast::Type::Builtin(_) => Value::String("builtin".to_string()),
                        crate::parser::ast::Type::Unknown    => Value::Int(0),
                        crate::parser::ast::Type::Fiber(_)   => Value::Bool(false),
                    };
                    let idx = ctx.add_constant(default_val);
                    self.bytecode.push(OpCode::Constant(idx));
                }
                if self.is_main && self.scopes.len() == 1 {
                    let idx = *ctx.globals.get(name).expect("Global not registered in 1st pass");
                    self.bytecode.push(OpCode::SetVar(idx));
                } else {
                    let slot = self.next_local;
                    self.define_local(*name, slot);
                    self.next_local += 1;
                    self.bytecode.push(OpCode::SetLocal(slot));
                }
            }
            crate::parser::ast::StmtKind::Print(expr) => {
                self.compile_expr(expr, ctx);
                self.bytecode.push(OpCode::Print);
            }
            crate::parser::ast::StmtKind::FunctionCallStmt { name, args } => {
                for arg in args {
                    self.compile_expr(arg, ctx);
                }
                if let Some(&func_id) = ctx.func_indices.get(name) {
                    self.bytecode.push(OpCode::Call(func_id, args.len()));
                }
            }
            crate::parser::ast::StmtKind::Input(name) => {
                self.bytecode.push(OpCode::Input);
                if let Some(slot) = self.lookup_local(name) {
                    self.bytecode.push(OpCode::SetLocal(slot));
                } else if let Some(&global_idx) = ctx.globals.get(name) {
                    self.bytecode.push(OpCode::SetVar(global_idx));
                }
            }
            crate::parser::ast::StmtKind::Assign { name, value } => {
                self.compile_expr(value, ctx);
                if let Some(slot) = self.lookup_local(name) {
                    self.bytecode.push(OpCode::SetLocal(slot));
                } else if let Some(&global_idx) = ctx.globals.get(name) {
                    self.bytecode.push(OpCode::SetVar(global_idx));
                }
            }
            crate::parser::ast::StmtKind::If { condition, then_branch, else_ifs, else_branch } => {
                let mut end_jumps = Vec::new();
                self.compile_expr(condition, ctx);
                let jmp_idx = self.bytecode.len();
                self.bytecode.push(OpCode::JumpIfFalse(0));
                
                self.enter_scope();
                for s in then_branch {
                    self.compile_stmt(s, ctx);
                }
                self.exit_scope();

                if !else_ifs.is_empty() || else_branch.is_some() {
                    end_jumps.push(self.bytecode.len());
                    self.bytecode.push(OpCode::Jump(0));
                }
                self.bytecode[jmp_idx] = OpCode::JumpIfFalse(self.bytecode.len());
                for (elif_cond, elif_branch) in else_ifs {
                    self.compile_expr(elif_cond, ctx);
                    let elif_jmp = self.bytecode.len();
                    self.bytecode.push(OpCode::JumpIfFalse(0));
                    self.enter_scope();
                    for s in elif_branch {
                        self.compile_stmt(s, ctx);
                    }
                    self.exit_scope();
                    end_jumps.push(self.bytecode.len());
                    self.bytecode.push(OpCode::Jump(0));
                    self.bytecode[elif_jmp] = OpCode::JumpIfFalse(self.bytecode.len());
                }
                if let Some(branch) = else_branch {
                    self.enter_scope();
                    for s in branch {
                        self.compile_stmt(s, ctx);
                    }
                    self.exit_scope();
                }
                let final_idx = self.bytecode.len();
                for idx in end_jumps { self.bytecode[idx] = OpCode::Jump(final_idx); }
            }
            crate::parser::ast::StmtKind::While { condition, body } => {
                let start_p = self.bytecode.len();
                self.loop_stack.push((start_p, Vec::new(), Vec::new(), None));
                self.compile_expr(condition, ctx);
                let exit_jmp = self.bytecode.len();
                self.bytecode.push(OpCode::JumpIfFalse(0));
                self.enter_scope();
                for s in body {
                    self.compile_stmt(s, ctx);
                }
                self.exit_scope();
                self.bytecode.push(OpCode::Jump(start_p));
                self.bytecode[exit_jmp] = OpCode::JumpIfFalse(self.bytecode.len());
                let (_, breaks, continues, _) = self.loop_stack.pop().unwrap();
                let end_label = self.bytecode.len();
                for b in breaks    { self.bytecode[b] = OpCode::Jump(end_label); }
                for c in continues { self.bytecode[c] = OpCode::Jump(start_p); }
            }
            crate::parser::ast::StmtKind::For { var_name, start, end, step, body, iter_type } => {
                match iter_type {
                    crate::parser::ast::ForIterType::Array => {
                        // Push collection, store in temp slot
                        self.compile_expr(start, ctx);
                        let array_slot = self.next_local;
                        self.next_local += 1;
                        self.bytecode.push(OpCode::SetLocal(array_slot));

                        // Index counter starts at 0
                        let zero_idx = ctx.add_constant(Value::Int(0));
                        self.bytecode.push(OpCode::Constant(zero_idx));
                        let index_slot = self.next_local;
                        self.next_local += 1;
                        self.bytecode.push(OpCode::SetLocal(index_slot));

                        // Loop variable slot
                        let loop_var_slot = if let Some(s) = self.lookup_local(var_name) { s } else {
                            let s = self.next_local;
                            self.define_local(*var_name, s);
                            self.next_local += 1;
                            s
                        };

                        let start_label = self.bytecode.len();
                        self.enter_scope(); // For loop variable and body
                        self.loop_stack.push((start_label, Vec::new(), Vec::new(), None));

                        // Condition: collection.size() > index
                        self.bytecode.push(OpCode::GetLocal(array_slot));
                        let size_id = ctx.add_constant(Value::String("size".to_string()));
                        self.bytecode.push(OpCode::MethodCall(size_id, 0));
                        self.bytecode.push(OpCode::GetLocal(index_slot));
                        self.bytecode.push(OpCode::Greater);
                        let exit_jmp = self.bytecode.len();
                        self.bytecode.push(OpCode::JumpIfFalse(0));

                        // Load element: collection.get(index)
                        self.bytecode.push(OpCode::GetLocal(array_slot));
                        self.bytecode.push(OpCode::GetLocal(index_slot));
                        let get_id = ctx.add_constant(Value::String("get".to_string()));
                        self.bytecode.push(OpCode::MethodCall(get_id, 1));
                        self.bytecode.push(OpCode::SetLocal(loop_var_slot));

                        for s in body { self.compile_stmt(s, ctx); }

                        // Increment index
                        let cont_label = self.bytecode.len();
                        self.bytecode.push(OpCode::GetLocal(index_slot));
                        if let Some(s) = step {
                            self.compile_expr(s, ctx);
                        } else {
                            let one_idx = ctx.add_constant(Value::Int(1));
                            self.bytecode.push(OpCode::Constant(one_idx));
                        }
                        self.bytecode.push(OpCode::Add);
                        self.bytecode.push(OpCode::SetLocal(index_slot));
                        self.bytecode.push(OpCode::Jump(start_label));

                        let end_label = self.bytecode.len();
                        self.bytecode[exit_jmp] = OpCode::JumpIfFalse(end_label);
                        let (_, breaks, continues, _) = self.loop_stack.pop().unwrap();
                        self.exit_scope();
                        for b in breaks    { self.bytecode[b] = OpCode::Jump(end_label); }
                        for c in continues { self.bytecode[c] = OpCode::Jump(cont_label); }
                    }
                    crate::parser::ast::ForIterType::Range => {
                        self.compile_expr(start, ctx);
                        let loop_var_slot = if let Some(s) = self.lookup_local(var_name) { s } else {
                            let s = self.next_local;
                            self.define_local(*var_name, s);
                            self.next_local += 1;
                            s
                        };
                        self.bytecode.push(OpCode::SetLocal(loop_var_slot));

                        let start_p = self.bytecode.len();
                        self.enter_scope();
                        self.loop_stack.push((start_p, Vec::new(), Vec::new(), None));

                        // Condition: end >= var (i.e. var <= end)
                        self.compile_expr(end, ctx);
                        self.bytecode.push(OpCode::GetLocal(loop_var_slot));
                        self.bytecode.push(OpCode::GreaterEqual);
                        let exit_jmp = self.bytecode.len();
                        self.bytecode.push(OpCode::JumpIfFalse(0));

                        for s in body { self.compile_stmt(s, ctx); }

                        // Increment
                        let cont_label = self.bytecode.len();
                        self.bytecode.push(OpCode::GetLocal(loop_var_slot));
                        if let Some(s) = step {
                            self.compile_expr(s, ctx);
                        } else {
                            let one_idx = ctx.add_constant(Value::Int(1));
                            self.bytecode.push(OpCode::Constant(one_idx));
                        }
                        self.bytecode.push(OpCode::Add);
                        self.bytecode.push(OpCode::SetLocal(loop_var_slot));
                        self.bytecode.push(OpCode::Jump(start_p));

                        let end_p = self.bytecode.len();
                        self.bytecode[exit_jmp] = OpCode::JumpIfFalse(end_p);
                        let (_, breaks, continues, _) = self.loop_stack.pop().unwrap();
                        self.exit_scope();
                        for idx in breaks    { self.bytecode[idx] = OpCode::Jump(end_p); }
                        for idx in continues { self.bytecode[idx] = OpCode::Jump(cont_label); }
                    }
                    crate::parser::ast::ForIterType::Fiber => {
                        // Store fiber in a temp slot, then loop while !isDone()
                        self.compile_expr(start, ctx);
                        let fiber_slot = self.next_local;
                        self.next_local += 1;
                        self.bytecode.push(OpCode::SetLocal(fiber_slot));

                        let loop_var_slot = if let Some(s) = self.lookup_local(var_name) { s } else {
                            let s = self.next_local;
                            self.define_local(*var_name, s);
                            self.next_local += 1;
                            s
                        };

                        let start_label = self.bytecode.len();
                        self.enter_scope();
                        self.loop_stack.push((start_label, Vec::new(), Vec::new(), Some(fiber_slot)));

                        // Condition: !fiber.isDone()
                        self.bytecode.push(OpCode::GetLocal(fiber_slot));
                        self.bytecode.push(OpCode::FiberIsDone);
                        let exit_jmp = self.bytecode.len();
                        self.bytecode.push(OpCode::JumpIfTrue(0));

                        // Get next value (it's already been pre-loaded by FiberIsDone)
                        self.bytecode.push(OpCode::GetLocal(fiber_slot));
                        self.bytecode.push(OpCode::FiberNext);
                        self.bytecode.push(OpCode::SetLocal(loop_var_slot));

                        for s in body { self.compile_stmt(s, ctx); }

                        let cont_label = self.bytecode.len();
                        self.bytecode.push(OpCode::Jump(start_label));

                        let end_label = self.bytecode.len();
                        self.bytecode[exit_jmp] = OpCode::JumpIfTrue(end_label);
                        let (_, breaks, continues, _) = self.loop_stack.pop().unwrap();
                        self.exit_scope();
                        for b in breaks    { self.bytecode[b] = OpCode::Jump(end_label); }
                        for c in continues { self.bytecode[c] = OpCode::Jump(cont_label); }
                    }
                }
            }
            crate::parser::ast::StmtKind::Break => {
                // If we are in a fiber loop, emit FiberClose before jumping
                if let Some(&(_, _, _, Some(fiber_slot))) = self.loop_stack.last() {
                    self.bytecode.push(OpCode::GetLocal(fiber_slot));
                    self.bytecode.push(OpCode::FiberClose);
                }
                let jmp = self.bytecode.len();
                self.bytecode.push(OpCode::Jump(0));
                if let Some(l) = self.loop_stack.last_mut() { l.1.push(jmp); }
            }
            crate::parser::ast::StmtKind::Continue => {
                let jmp = self.bytecode.len();
                self.bytecode.push(OpCode::Jump(0));
                if let Some(l) = self.loop_stack.last_mut() { l.2.push(jmp); }
            }
            crate::parser::ast::StmtKind::ExprStmt(expr) => {
                self.compile_expr(expr, ctx);
                self.bytecode.push(OpCode::Pop);
            }
            crate::parser::ast::StmtKind::Halt { level, message } => {
                self.compile_expr(message, ctx);
                match level {
                    crate::parser::ast::HaltLevel::Alert => self.bytecode.push(OpCode::HaltAlert),
                    crate::parser::ast::HaltLevel::Error => self.bytecode.push(OpCode::HaltError),
                    crate::parser::ast::HaltLevel::Fatal => self.bytecode.push(OpCode::HaltFatal),
                }
            }
            crate::parser::ast::StmtKind::Return(expr) => {
                if let Some(e) = expr {
                    self.compile_expr(e, ctx);
                    self.bytecode.push(OpCode::Return);
                } else if self.has_return_type {
                    self.bytecode.push(OpCode::Return);
                } else {
                    self.bytecode.push(OpCode::ReturnVoid);
                }
            }
            crate::parser::ast::StmtKind::FunctionDef { name, params, body, return_type } => {
                let mut fc = FunctionCompiler::new(false, Some(self.convert_to_flat_locals()));
                fc.has_return_type = return_type.is_some();
                for (i, (_, pname)) in params.iter().enumerate() {
                    fc.define_local(*pname, i);
                }
                fc.next_local = params.len();
                for s in body { fc.compile_stmt(s, ctx); }
                
                let chunk = FunctionChunk {
                    bytecode: fc.bytecode,
                    is_fiber: false,
                    max_locals: fc.next_local,
                };
                let fid = ctx.func_indices.get(name).copied().unwrap_or(0);
                ctx.functions[fid] = chunk;
            }
            crate::parser::ast::StmtKind::FiberDef { name, params, body, return_type } => {
                let mut fc = FunctionCompiler::new(false, Some(self.convert_to_flat_locals()));
                fc.has_return_type = return_type.is_some();
                for (i, (_, pname)) in params.iter().enumerate() {
                    fc.define_local(*pname, i);
                }
                fc.next_local = params.len();
                for s in body { fc.compile_stmt(s, ctx); }
                
                let chunk = FunctionChunk {
                    bytecode: fc.bytecode,
                    is_fiber: true,
                    max_locals: fc.next_local,
                };
                let fid = ctx.func_indices.get(name).copied().unwrap_or(0);
                ctx.functions[fid] = chunk;
            }
            crate::parser::ast::StmtKind::FiberDecl { name, fiber_name, args, .. } => {
                for arg in args { self.compile_expr(arg, ctx); }
                let f_idx = ctx.func_indices.get(fiber_name).copied().unwrap_or(0);
                self.bytecode.push(OpCode::FiberCreate(f_idx, args.len()));
                let slot = if let Some(s) = self.lookup_local(name) { s } else {
                    let s = self.next_local;
                    self.define_local(*name, s);
                    self.next_local += 1;
                    s
                };
                self.bytecode.push(OpCode::SetLocal(slot));
            }
            crate::parser::ast::StmtKind::JsonBind { json, path, target } => {
                self.compile_expr(json, ctx);
                self.compile_expr(path, ctx);
                if let Some(local_idx) = self.lookup_local(target) {
                    self.bytecode.push(OpCode::JsonBindLocal(local_idx));
                } else {
                    let idx = ctx.globals.get(target).copied().unwrap_or(0);
                    self.bytecode.push(OpCode::JsonBind(idx));
                }
            }
            crate::parser::ast::StmtKind::JsonInject { json, mapping, table } => {
                self.compile_expr(json, ctx);
                self.compile_expr(mapping, ctx);
                if let Some(local_idx) = self.lookup_local(table) {
                    self.bytecode.push(OpCode::JsonInjectLocal(local_idx));
                } else {
                    let idx = ctx.globals.get(table).copied().unwrap_or(0);
                    self.bytecode.push(OpCode::JsonInject(idx));
                }
            }
            crate::parser::ast::StmtKind::Yield(expr) => {
                self.compile_expr(expr, ctx);
                self.bytecode.push(OpCode::Yield);
            }
            crate::parser::ast::StmtKind::YieldFrom(expr) => {
                self.compile_expr(expr, ctx);
                let fiber_slot = self.next_local;
                self.next_local += 1;
                self.bytecode.push(OpCode::SetLocal(fiber_slot));

                let start_label = self.bytecode.len();

                self.bytecode.push(OpCode::GetLocal(fiber_slot));
                self.bytecode.push(OpCode::FiberIsDone);
                let exit_jmp = self.bytecode.len();
                self.bytecode.push(OpCode::JumpIfTrue(0));

                self.bytecode.push(OpCode::GetLocal(fiber_slot));
                self.bytecode.push(OpCode::FiberNext);
                self.bytecode.push(OpCode::Yield);

                self.bytecode.push(OpCode::Jump(start_label));

                let end_label = self.bytecode.len();
                self.bytecode[exit_jmp] = OpCode::JumpIfTrue(end_label);
            }
            crate::parser::ast::StmtKind::YieldVoid => {
                self.bytecode.push(OpCode::YieldVoid);
            }
            crate::parser::ast::StmtKind::Wait(expr) => {
                self.compile_expr(expr, ctx);
                self.bytecode.push(OpCode::Wait);
            }
            crate::parser::ast::StmtKind::NetRequestStmt { method, url, headers, body, timeout, target } => {
                // Build a map for HttpRequest
                // { "method": method, "url": url, "headers": headers, "body": body, "timeout": timeout }
                let mut elements = Vec::new();
                elements.push((crate::parser::ast::Expr { kind: crate::parser::ast::ExprKind::StringLiteral(ctx.interner.intern("method")), span: crate::lexer::token::Span { line: 0, col: 0, len: 0 } }, *method.clone()));
                elements.push((crate::parser::ast::Expr { kind: crate::parser::ast::ExprKind::StringLiteral(ctx.interner.intern("url")), span: crate::lexer::token::Span { line: 0, col: 0, len: 0 } }, *url.clone()));
                if let Some(h) = headers {
                    elements.push((crate::parser::ast::Expr { kind: crate::parser::ast::ExprKind::StringLiteral(ctx.interner.intern("headers")), span: crate::lexer::token::Span { line: 0, col: 0, len: 0 } }, *h.clone()));
                }
                if let Some(b) = body {
                    elements.push((crate::parser::ast::Expr { kind: crate::parser::ast::ExprKind::StringLiteral(ctx.interner.intern("body")), span: crate::lexer::token::Span { line: 0, col: 0, len: 0 } }, *b.clone()));
                }
                if let Some(t) = timeout {
                    elements.push((crate::parser::ast::Expr { kind: crate::parser::ast::ExprKind::StringLiteral(ctx.interner.intern("timeout")), span: crate::lexer::token::Span { line: 0, col: 0, len: 0 } }, *t.clone()));
                }
                
                let map_expr = crate::parser::ast::Expr {
                    kind: crate::parser::ast::ExprKind::MapLiteral { 
                        key_type: crate::parser::ast::Type::String,
                        value_type: crate::parser::ast::Type::Json,
                        elements 
                    },
                    span: crate::lexer::token::Span { line: 0, col: 0, len: 0 },
                };
                self.compile_expr(&map_expr, ctx);
                self.bytecode.push(OpCode::HttpRequest);
                
                // Set to target
                if let Some(slot) = self.lookup_local(target) {
                    self.bytecode.push(OpCode::SetLocal(slot));
                } else if let Some(&idx) = ctx.globals.get(target) {
                    self.bytecode.push(OpCode::SetVar(idx));
                } else {
                    // Implicit local if it's "as target"
                    let slot = self.next_local;
                    self.define_local(*target, slot);
                    self.next_local += 1;
                    self.bytecode.push(OpCode::SetLocal(slot));
                }
            }
            crate::parser::ast::StmtKind::Serve { name, port, host, workers, routes } => {
                self.compile_expr(port, ctx);
                if let Some(h) = host { self.compile_expr(h, ctx); }
                else { let f = ctx.add_constant(Value::Bool(false)); self.bytecode.push(OpCode::Constant(f)); }
                
                if let Some(w) = workers { self.compile_expr(w, ctx); }
                else { let f = ctx.add_constant(Value::Bool(false)); self.bytecode.push(OpCode::Constant(f)); }
                
                self.compile_expr(routes, ctx);
                
                let s_name = ctx.interner.lookup(*name).to_string();
                let ni = ctx.add_constant(Value::String(s_name));
                self.bytecode.push(OpCode::HttpServe(ni));
            }
            crate::parser::ast::StmtKind::Include { .. } => {
                // Handled in the pre-processor/expander pass
            }
        }
    }
}

// ── first pass helper ────────────────────────────────────────────────────────

fn register_globals_recursive(
    stmts: &[Stmt],
    globals: &mut std::collections::HashMap<StringId, usize>,
    func_indices: &mut std::collections::HashMap<StringId, usize>,
    functions: &mut Vec<FunctionChunk>,
    is_main_script: bool,
) {
    for stmt in stmts {
        match &stmt.kind {
            crate::parser::ast::StmtKind::FunctionDef { name, body, .. } => {
                let idx = functions.len();
                func_indices.insert(*name, idx);
                functions.push(FunctionChunk {
                    bytecode: Vec::new(), is_fiber: false, max_locals: 0,
                });
                // Recurse into function body to find nested functions/fibers
                // but mark as NOT main script so nested VarDecls become locals.
                register_globals_recursive(body, globals, func_indices, functions, false);
            }
            crate::parser::ast::StmtKind::FiberDef { name, body, .. } => {
                let idx = functions.len();
                func_indices.insert(*name, idx);
                functions.push(FunctionChunk {
                    bytecode: Vec::new(), is_fiber: true, max_locals: 0,
                });
                register_globals_recursive(body, globals, func_indices, functions, false);
            }
            crate::parser::ast::StmtKind::VarDecl { name, .. } if is_main_script => {
                if !globals.contains_key(name) {
                    let idx = globals.len();
                    globals.insert(*name, idx);
                }
            }
            crate::parser::ast::StmtKind::FiberDecl { name, .. } if is_main_script => {
                if !globals.contains_key(name) {
                    let idx = globals.len();
                    globals.insert(*name, idx);
                }
            }
            crate::parser::ast::StmtKind::If { then_branch, else_ifs, else_branch, .. } => {
                register_globals_recursive(then_branch, globals, func_indices, functions, false);
                for (_, elif_branch) in else_ifs {
                    register_globals_recursive(elif_branch, globals, func_indices, functions, false);
                }
                if let Some(eb) = else_branch {
                    register_globals_recursive(eb, globals, func_indices, functions, false);
                }
            }
            crate::parser::ast::StmtKind::While { body, .. } => {
                register_globals_recursive(body, globals, func_indices, functions, false);
            }
            crate::parser::ast::StmtKind::For { body, .. } => {
                register_globals_recursive(body, globals, func_indices, functions, false);
            }
            _ => {}
        }
    }
}

impl Compiler {
    #[allow(dead_code)]
    pub fn get_global_idx(&self, name: StringId) -> usize {
        *self.globals.get(&name).expect("Global not found")
    }

    pub fn new() -> Self {
        Self {
            globals: HashMap::new(),
            func_indices: HashMap::new(),
            functions: Vec::new(),
            constants: Vec::new(),
        }
    }

    pub fn compile(
        &mut self,
        program: &Program,
        interner: &mut Interner,
    ) -> (Vec<OpCode>, Vec<Value>, Vec<FunctionChunk>) {
        // ── First pass: register built-in and user symbols ────────────────────
        let built_ins = ["json", "date", "random", "store"];
        for (i, name) in built_ins.iter().enumerate() {
            let id = interner.intern(name);
            self.globals.insert(id, i);
        }

        register_globals_recursive(&program.stmts, &mut self.globals, &mut self.func_indices, &mut self.functions, true);

        // ── Build compile context ─────────────────────────────────────────────
        let mut ctx = CompileContext {
            constants:    &mut self.constants,
            functions:    &mut self.functions,
            func_indices: &self.func_indices,
            globals:      &self.globals,
            interner,
        };

        let mut main_compiler = FunctionCompiler::new(true, None);

        // Init built-in global slots (0:json, 1:date, 2:random, 3:store)
        for (i, name) in ["json", "date", "random", "store"].iter().enumerate() {
            let val = ctx.add_constant(Value::String(name.to_string()));
            main_compiler.bytecode.push(OpCode::Constant(val));
            main_compiler.bytecode.push(OpCode::SetVar(i));
        }

        // ── Second pass: compile all stmts ────────────────────────────────────
        for stmt in &program.stmts {
            match &stmt.kind {
                crate::parser::ast::StmtKind::FunctionDef { name, params, return_type, body } => {
                    let fid = *self.func_indices.get(name).unwrap();
                    let chunk = compile_function_helper(
                        params, body, return_type.is_some(), false, &mut ctx,
                    );
                    ctx.functions[fid] = chunk;
                }
                crate::parser::ast::StmtKind::FiberDef { name, params, body, .. } => {
                    let fid = *self.func_indices.get(name).unwrap();
                    let chunk = compile_function_helper(
                        params, body, true, true, &mut ctx,
                    );
                    ctx.functions[fid] = chunk;
                }
                _ => main_compiler.compile_stmt(stmt, &mut ctx),
            }
        }

        main_compiler.bytecode.push(OpCode::Halt);
        (main_compiler.bytecode, self.constants.clone(), self.functions.clone())
    }
}

// ── function / fiber helper ───────────────────────────────────────────────────

fn compile_function_helper(
    params: &[(crate::parser::ast::Type, StringId)],
    body: &[Stmt],
    has_return_type: bool,
    is_fiber: bool,
    ctx: &mut CompileContext,
) -> FunctionChunk {
    let mut compiler = FunctionCompiler::new(false, None);
    compiler.has_return_type = has_return_type;

    for (i, (_, param_name)) in params.iter().enumerate() {
        compiler.define_local(*param_name, i);
    }
    compiler.next_local = params.len();

    for s in body {
        compiler.compile_stmt(s, ctx);
    }

    // Ensure a terminal return opcode exists
    if !compiler.bytecode.last().map_or(false, |op| {
        matches!(op, OpCode::Return | OpCode::ReturnVoid)
    }) {
        compiler.bytecode.push(OpCode::ReturnVoid);
    }

    FunctionChunk {
        bytecode: compiler.bytecode,
        is_fiber,
        max_locals: compiler.next_local,
    }
}