pub mod vm;
pub mod repl;

#[cfg(test)]
mod tests;

use crate::parser::ast::{Stmt, Expr};
use crate::backend::vm::{OpCode, Value, FunctionChunk, MethodKind};
use crate::lexer::token::TokenKind;
use crate::sema::interner::{Interner, StringId};
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;


pub struct Compiler {
    pub globals: HashMap<StringId, usize>,
    pub func_indices: HashMap<StringId, usize>,
    pub functions: Vec<FunctionChunk>,
    pub constants: Vec<Value>,
    pub string_constants: HashMap<String, usize>,
}

pub struct CompileContext<'a> {
    pub constants: &'a mut Vec<Value>,
    pub string_constants: &'a mut HashMap<String, usize>,
    pub functions: &'a mut Vec<FunctionChunk>,
    pub func_indices: &'a HashMap<StringId, usize>,
    pub globals: &'a HashMap<StringId, usize>,
    pub interner: &'a mut Interner,
}

impl<'a> CompileContext<'a> {
    pub fn add_string_constant(&mut self, s: &'static str) -> usize {
        if let Some(&idx) = self.string_constants.get(s) {
            return idx;
        }
        let idx = self.constants.len();
        self.string_constants.insert(s.to_string(), idx);
        self.constants.push(Value::String(s.to_string()));
        idx
    }

    pub fn add_constant(&mut self, val: Value) -> usize {
        if let Value::String(ref s) = val {
            if let Some(&idx) = self.string_constants.get(s) {
                return idx;
            }
            let idx = self.constants.len();
            self.string_constants.insert(s.clone(), idx);
            self.constants.push(val);
            return idx;
        }
        self.constants.push(val);
        self.constants.len() - 1
    }
}

pub struct FunctionCompiler {
    pub bytecode: Vec<OpCode>,
    pub spans: Vec<crate::lexer::token::Span>,
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
            spans: Vec::new(),
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

    fn emit(&mut self, op: OpCode, span: &crate::lexer::token::Span) {
        self.bytecode.push(op);
        self.spans.push(span.clone());
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

    fn map_method_kind(&self, name: &str) -> Option<MethodKind> {
        match name {
            "push" => Some(MethodKind::Push),
            "pop" => Some(MethodKind::Pop),
            "len" => Some(MethodKind::Len),
            "count" => Some(MethodKind::Count),
            "size" => Some(MethodKind::Size),
            "clear" => Some(MethodKind::Clear),
            "contains" => Some(MethodKind::Contains),
            "isEmpty" => Some(MethodKind::IsEmpty),
            "get" => Some(MethodKind::Get),
            "insert" => Some(MethodKind::Insert),
            "update" => Some(MethodKind::Update),
            "delete" => Some(MethodKind::Delete),
            "find" => Some(MethodKind::Find),
            "join" => Some(MethodKind::Join),
            "show" => Some(MethodKind::Show),
            "sort" => Some(MethodKind::Sort),
            "reverse" => Some(MethodKind::Reverse),
            "add" => Some(MethodKind::Add),
            "remove" => Some(MethodKind::Remove),
            "has" => Some(MethodKind::Has),
            "length" => Some(MethodKind::Length),
            "upper" => Some(MethodKind::Upper),
            "lower" => Some(MethodKind::Lower),
            "trim" => Some(MethodKind::Trim),
            "indexOf" => Some(MethodKind::IndexOf),
            "lastIndexOf" => Some(MethodKind::LastIndexOf),
            "replace" => Some(MethodKind::Replace),
            "slice" => Some(MethodKind::Slice),
            "split" => Some(MethodKind::Split),
            "startsWith" | "starts_with" => Some(MethodKind::StartsWith),
            "endsWith" | "ends_with" => Some(MethodKind::EndsWith),
            "toInt" | "to_int" => Some(MethodKind::ToInt),
            "toFloat" | "to_float" => Some(MethodKind::ToFloat),
            "set" => Some(MethodKind::Set),
            "keys" => Some(MethodKind::Keys),
            "values" => Some(MethodKind::Values),
            "where" => Some(MethodKind::Where),
            "year" => Some(MethodKind::Year),
            "month" => Some(MethodKind::Month),
            "day" => Some(MethodKind::Day),
            "hour" => Some(MethodKind::Hour),
            "minute" => Some(MethodKind::Minute),
            "second" => Some(MethodKind::Second),
            "format" => Some(MethodKind::Format),
            "exists" => Some(MethodKind::Exists),
            "append" => Some(MethodKind::Append),
            "inject" => Some(MethodKind::Inject),
            "to_str" | "to_string" | "toString" => Some(MethodKind::ToStr),
            "next" => Some(MethodKind::Next),
            "run" => Some(MethodKind::Run),
            "isDone" => Some(MethodKind::IsDone),
            "close" => Some(MethodKind::Close),
            _ => None,
        }
    }

    pub fn compile_expr(&mut self, expr: &Expr, ctx: &mut CompileContext) {
        match &expr.kind {
            crate::parser::ast::ExprKind::IntLiteral(v) => {
                let i = ctx.add_constant(Value::Int(*v));
                self.emit(OpCode::Constant(i), &expr.span);
            }
            crate::parser::ast::ExprKind::FloatLiteral(v) => {
                let i = ctx.add_constant(Value::Float(*v));
                self.emit(OpCode::Constant(i), &expr.span);
            }
            crate::parser::ast::ExprKind::StringLiteral(id) => {
                let s = ctx.interner.lookup(*id).to_string();
                let i = ctx.add_constant(Value::String(s));
                self.emit(OpCode::Constant(i), &expr.span);
            }
            crate::parser::ast::ExprKind::BoolLiteral(v) => {
                let i = ctx.add_constant(Value::Bool(*v));
                self.emit(OpCode::Constant(i), &expr.span);
            }
            crate::parser::ast::ExprKind::Identifier(id) => {
                if let Some(slot) = self.lookup_local(id) {
                    self.emit(OpCode::GetLocal(slot), &expr.span);
                } else if let Some(&idx) = ctx.globals.get(id) {
                    self.emit(OpCode::GetVar(idx), &expr.span);
                } else if let Some(&fid) = ctx.func_indices.get(id) {
                    let i = ctx.add_constant(Value::Function(fid));
                    self.emit(OpCode::Constant(i), &expr.span);
                } else if let Some(&_parent_slot) = self.parent_locals.as_ref().and_then(|p| p.get(id)) {
                    let slot = self.next_local;
                    self.define_local(*id, slot);
                    self.next_local += 1;
                    self.captures.push(*id);
                    self.emit(OpCode::GetLocal(slot), &expr.span);
                } else if self.is_table_lambda {
                    self.emit(OpCode::GetLocal(0), &expr.span);
                    let mi = ctx.add_constant(Value::String(ctx.interner.lookup(*id).to_string()));
                    self.emit(OpCode::MethodCallCustom(mi, 0), &expr.span);
                } else {
                    let name = ctx.interner.lookup(*id).to_string();
                    let i = ctx.add_constant(Value::String(name));
                    self.emit(OpCode::Constant(i), &expr.span);
                }
            }
            crate::parser::ast::ExprKind::FunctionCall { name, args } => {
                let n = ctx.interner.lookup(*name);
                if n == "json.parse" && args.len() == 1 {
                    self.compile_expr(&args[0], ctx);
                    self.emit(OpCode::JsonParse, &expr.span);
                } else if n == "terminal.input" {
                    self.emit(OpCode::Input, &expr.span);
                } else if n == "i" && args.len() == 1 {
                    self.compile_expr(&args[0], ctx);
                    self.emit(OpCode::CastInt, &expr.span);
                } else if n == "f" && args.len() == 1 {
                    self.compile_expr(&args[0], ctx);
                    self.emit(OpCode::CastFloat, &expr.span);
                } else if n == "s" && args.len() == 1 {
                    self.compile_expr(&args[0], ctx);
                    self.emit(OpCode::CastString, &expr.span);
                } else if n == "b" && args.len() == 1 {
                    self.compile_expr(&args[0], ctx);
                    self.emit(OpCode::CastBool, &expr.span);
                } else {
                    for arg in args {
                        self.compile_expr(arg, ctx);
                    }
                    if let Some(&fid) = ctx.func_indices.get(name) {
                        if ctx.functions[fid].is_fiber {
                            self.emit(OpCode::FiberCreate(fid, args.len()), &expr.span);
                        } else {
                            self.emit(OpCode::Call(fid, args.len()), &expr.span);
                        }
                    }
                }
            }
            crate::parser::ast::ExprKind::ArrayLiteral { elements } => {
                for e in elements {
                    self.compile_expr(e, ctx);
                }
                self.emit(OpCode::ArrayInit(elements.len()), &expr.span);
            }
            crate::parser::ast::ExprKind::SetLiteral { elements, range, .. } => {
                if let Some(r) = range {
                    self.compile_expr(&r.start, ctx);
                    self.compile_expr(&r.end, ctx);
                    if let Some(s) = &r.step {
                        self.compile_expr(s, ctx);
                        let t = ctx.add_constant(Value::Bool(true));
                        self.emit(OpCode::Constant(t), &expr.span);
                    } else {
                        let f = ctx.add_constant(Value::Bool(false));
                        self.emit(OpCode::Constant(f), &expr.span);
                    }
                    self.emit(OpCode::SetRange, &expr.span);
                } else {
                    for e in elements {
                        self.compile_expr(e, ctx);
                    }
                    self.emit(OpCode::SetInit(elements.len()), &expr.span);
                }
            }
            crate::parser::ast::ExprKind::MapLiteral { elements, .. } => {
                for (k, v) in elements {
                    self.compile_expr(k, ctx);
                    self.compile_expr(v, ctx);
                }
                self.emit(OpCode::MapInit(elements.len()), &expr.span);
            }
            crate::parser::ast::ExprKind::RandomChoice { set } => {
                self.compile_expr(set, ctx);
                self.emit(OpCode::RandomChoice, &expr.span);
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
                self.emit(OpCode::Constant(i), &expr.span);
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
                        "write"  => self.emit(OpCode::StoreWrite, &expr.span),
                        "read"   => self.emit(OpCode::StoreRead, &expr.span),
                        "append" => self.emit(OpCode::StoreAppend, &expr.span),
                        "exists" => self.emit(OpCode::StoreExists, &expr.span),
                        "delete" => self.emit(OpCode::StoreDelete, &expr.span),
                        _ => {}
                    }
                    return;
                }
                // ── date.now() ─────────────────────────────────────────────────
                if is_date && method_name == "now" {
                    self.emit(OpCode::DateNow, &expr.span);
                    return;
                }
                // ── json.parse() ───────────────────────────────────────────────
                if is_json && method_name == "parse" {
                    self.compile_expr(&args[0], ctx);
                    self.emit(OpCode::JsonParse, &expr.span);
                    return;
                }
                // ── env.get("VAR") / env.args() ────────────────────────────────
                if is_env {
                    if method_name == "get" {
                        if let Some(arg) = args.first() {
                            self.compile_expr(arg, ctx);
                        }
                        self.emit(OpCode::EnvGet, &expr.span);
                    } else if method_name == "args" {
                        self.emit(OpCode::EnvArgs, &expr.span);
                    }
                    return;
                }

                // ── crypto.* ───────────────────────────────────────────────────
                if is_crypto {
                    for arg in args {
                        self.compile_expr(arg, ctx);
                    }
                    match method_name.as_str() {
                        "hash"   => self.emit(OpCode::CryptoHash, &expr.span),
                        "verify" => self.emit(OpCode::CryptoVerify, &expr.span),
                        "token"  => self.emit(OpCode::CryptoToken, &expr.span),
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
                            bytecode: Arc::new(lambda_fc.bytecode),
                            spans: Arc::new(lambda_fc.spans),
                            is_fiber: false,
                            max_locals: lambda_fc.next_local,
                        };
                        let fid = ctx.functions.len();
                        ctx.functions.push(chunk);

                        let ci = ctx.add_constant(Value::Function(fid));

                        // Reify captures
                        for captured_id in &lambda_fc.captures {
                            if let Some(outer_slot) = self.lookup_local(captured_id) {
                                self.emit(OpCode::GetLocal(outer_slot), &expr.span);
                            }
                        }

                        self.emit(OpCode::MethodCall(MethodKind::Join, 2 + lambda_fc.captures.len()), &expr.span);
                    } else {
                        // Key-based join: second arg is left-col string, third is right-col string
                        self.compile_expr(second, ctx);
                        if args.len() >= 3 {
                            self.compile_expr(&args[2], ctx);
                            self.emit(OpCode::MethodCall(MethodKind::Join, 3), &expr.span);
                        } else {
                            // Only 2 args — treat second as a function value expression
                            self.emit(OpCode::MethodCall(MethodKind::Join, 2), &expr.span);
                        }
                    }
                    return;
                }

                // ── general method call ────────────────────────────────────────
                self.compile_expr(receiver, ctx);
                for arg in args {
                    self.compile_expr(arg, ctx);
                }

                if let Some(kind) = self.map_method_kind(&method_name) {
                    self.emit(OpCode::MethodCall(kind, args.len()), &expr.span);
                } else {
                    let mi = ctx.add_constant(Value::String(method_name));
                    self.emit(OpCode::MethodCallCustom(mi, args.len()), &expr.span);
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
                        self.emit(OpCode::Not, &expr.span);
                    }
                    TokenKind::Minus => {
                        let zero = if matches!(right.kind, crate::parser::ast::ExprKind::FloatLiteral(_)) {
                            ctx.add_constant(Value::Float(0.0))
                        } else {
                            ctx.add_constant(Value::Int(0))
                        };
                        self.emit(OpCode::Constant(zero), &expr.span);
                        self.compile_expr(right, ctx);
                        self.emit(OpCode::Sub, &expr.span);
                    }
                    _ => {}
                }
            }
            crate::parser::ast::ExprKind::Index { receiver, index } => {
                self.compile_expr(receiver, ctx);
                self.compile_expr(index, ctx);
                self.emit(OpCode::MethodCall(MethodKind::Get, 1), &expr.span);
            }
            crate::parser::ast::ExprKind::Lambda { .. } => {
                // Lambdas are compiled when they are passed as arguments to methods like .where() or .join()
                // or when they are assigned to variables.
                // If a lambda appears as a standalone expression, it evaluates to a function value.
                let f = ctx.add_constant(Value::Bool(false)); // Placeholder for now, actual function value will be created by FunctionCompiler
                self.emit(OpCode::Constant(f), &expr.span);
            }
            crate::parser::ast::ExprKind::Tuple(exprs) => {
                for e in exprs { self.compile_expr(e, ctx); }
                self.emit(OpCode::ArrayInit(exprs.len()), &expr.span);
            }
            crate::parser::ast::ExprKind::ArrayOrSetLiteral { elements } => {
                for e in elements { self.compile_expr(e, ctx); }
                self.emit(OpCode::ArrayInit(elements.len()), &expr.span);
            }
            crate::parser::ast::ExprKind::TerminalCommand(cmd_id, arg) => {
                let cmd = ctx.interner.lookup(*cmd_id);
                if cmd == "exit"       { self.bytecode.push(OpCode::TerminalExit); }
                else if cmd == "clear" { self.bytecode.push(OpCode::TerminalClear); }
                else if cmd == "run"   {
                    if let Some(a) = arg {
                        self.compile_expr(a, ctx);
                        self.emit(OpCode::TerminalRun, &expr.span);
                    }
                }
            }
            crate::parser::ast::ExprKind::MemberAccess { receiver, member } => {
                self.compile_expr(receiver, ctx);
                let member_name = ctx.interner.lookup(*member).to_string();
                if let Some(kind) = self.map_method_kind(&member_name) {
                    self.emit(OpCode::MethodCall(kind, 0), &expr.span);
                } else {
                    let mi = ctx.add_constant(Value::String(member_name));
                    self.emit(OpCode::MethodCallCustom(mi, 0), &expr.span);
                }
            }
            crate::parser::ast::ExprKind::TableLiteral { columns, rows } => {
                for row in rows { for val in row { self.compile_expr(val, ctx); } }
                let vm_cols = columns.iter().map(|c| crate::backend::vm::VMColumn {
                    name: ctx.interner.lookup(c.name).to_string(),
                    ty: c.ty.clone(),
                    is_auto: c.is_auto,
                }).collect();
                let skeleton = Value::Table(Arc::new(RwLock::new(
                    crate::backend::vm::TableData { columns: vm_cols, rows: Vec::new() }
                )));
                let ci = ctx.add_constant(skeleton);
                self.emit(OpCode::TableInit(ci, rows.len()), &expr.span);
            }
            crate::parser::ast::ExprKind::RawBlock(id) => {
                let s = ctx.interner.lookup(*id).to_string();
                let i = ctx.add_constant(Value::String(s));
                self.emit(OpCode::Constant(i), &expr.span);
            }
            crate::parser::ast::ExprKind::NetCall { method, url, body } => {
                self.compile_expr(url, ctx);
                if let Some(b) = body {
                    self.compile_expr(b, ctx);
                } else {
                    let f = ctx.add_constant(Value::Bool(false));
                    self.emit(OpCode::Constant(f), &expr.span);
                }
                let mi = ctx.add_constant(Value::String(ctx.interner.lookup(*method).to_string()));
                self.emit(OpCode::HttpCall(mi), &expr.span);
            }
            crate::parser::ast::ExprKind::NetRespond { status, body, headers } => {
                self.compile_expr(status, ctx);
                self.compile_expr(body, ctx);
                if let Some(h) = headers {
                    self.compile_expr(h, ctx);
                } else {
                    let f = ctx.add_constant(Value::Bool(false));
                    self.emit(OpCode::Constant(f), &expr.span);
                }
                self.emit(OpCode::HttpRespond, &expr.span);
            }
        }
    }

    /// Helper: compile a single-argument lambda/expr predicate into an anonymous
    /// function, then emit MethodCall(method_name, 1).
    /// Used by `.where()` and can be reused for similar single-pred methods.
    fn compile_lambda_method(
        &mut self,
        _method_name: String,
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
            bytecode: Arc::new(lambda_fc.bytecode),
            spans: Arc::new(lambda_fc.spans),
            is_fiber: false,
            max_locals: lambda_fc.next_local,
        };
        let fid = ctx.functions.len();
        ctx.functions.push(chunk);

        let ci = ctx.add_constant(Value::Function(fid));
        self.emit(OpCode::Constant(ci), &pred_expr.span);

        // Reify captures from outer scope
        for captured_id in &lambda_fc.captures {
            if let Some(outer_slot) = self.lookup_local(captured_id) {
                self.emit(OpCode::GetLocal(outer_slot), &pred_expr.span);
            }
        }

        self.emit(OpCode::MethodCall(MethodKind::Where, 1 + lambda_fc.captures.len()), &pred_expr.span);
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
                            Arc::new(RwLock::new(Vec::new()))
                        ),
                        crate::parser::ast::Type::Set(_) => Value::Set(
                            Arc::new(RwLock::new(std::collections::BTreeSet::new()))
                        ),
                        crate::parser::ast::Type::Map(_, _) => Value::Map(
                            Arc::new(RwLock::new(Vec::new()))
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
                            Value::Table(Arc::new(RwLock::new(
                                crate::backend::vm::TableData { columns: vm_cols, rows: Vec::new() }
                            )))
                        }
                        crate::parser::ast::Type::Json => Value::Json(
                            Arc::new(RwLock::new(serde_json::Value::Null))
                        ),
                        crate::parser::ast::Type::Builtin(_) => Value::String("builtin".to_string()),
                        crate::parser::ast::Type::Unknown    => Value::Int(0),
                        crate::parser::ast::Type::Fiber(_)   => Value::Bool(false),
                    };
                    let idx = ctx.add_constant(default_val);
                    self.emit(OpCode::Constant(idx), &stmt.span);
                }
                if self.is_main && self.scopes.len() == 1 {
                    let idx = *ctx.globals.get(name).expect("Global not registered in 1st pass");
                    self.emit(OpCode::SetVar(idx), &stmt.span);
                } else {
                    let slot = self.next_local;
                    self.define_local(*name, slot);
                    self.next_local += 1;
                    self.emit(OpCode::SetLocal(slot), &stmt.span);
                }
            }
            crate::parser::ast::StmtKind::Print(expr) => {
                self.compile_expr(expr, ctx);
                self.emit(OpCode::Print, &stmt.span);
            }
            crate::parser::ast::StmtKind::FunctionCallStmt { name, args } => {
                for arg in args {
                    self.compile_expr(arg, ctx);
                }
                if let Some(&func_id) = ctx.func_indices.get(name) {
                    self.emit(OpCode::Call(func_id, args.len()), &stmt.span);
                }
            }
            crate::parser::ast::StmtKind::Input(name) => {
                self.emit(OpCode::Input, &stmt.span);
                if let Some(slot) = self.lookup_local(name) {
                    self.emit(OpCode::SetLocal(slot), &stmt.span);
                } else if let Some(&global_idx) = ctx.globals.get(name) {
                    self.emit(OpCode::SetVar(global_idx), &stmt.span);
                }
            }
            crate::parser::ast::StmtKind::Assign { name, value } => {
                self.compile_expr(value, ctx);
                if let Some(slot) = self.lookup_local(name) {
                    self.emit(OpCode::SetLocal(slot), &stmt.span);
                } else if let Some(&global_idx) = ctx.globals.get(name) {
                    self.emit(OpCode::SetVar(global_idx), &stmt.span);
                }
            }
            crate::parser::ast::StmtKind::If { condition, then_branch, else_ifs, else_branch } => {
                let mut end_jumps = Vec::new();
                self.compile_expr(condition, ctx);
                let jmp_idx = self.bytecode.len();
                self.emit(OpCode::JumpIfFalse(0), &stmt.span);
                
                self.enter_scope();
                for s in then_branch {
                    self.compile_stmt(s, ctx);
                }
                self.exit_scope();

                if !else_ifs.is_empty() || else_branch.is_some() {
                    end_jumps.push(self.bytecode.len());
                    self.emit(OpCode::Jump(0), &stmt.span);
                }
                self.bytecode[jmp_idx] = OpCode::JumpIfFalse(self.bytecode.len());
                for (elif_cond, elif_branch) in else_ifs {
                    self.compile_expr(elif_cond, ctx);
                    let elif_jmp = self.bytecode.len();
                    self.emit(OpCode::JumpIfFalse(0), &stmt.span);
                    self.enter_scope();
                    for s in elif_branch {
                        self.compile_stmt(s, ctx);
                    }
                    self.exit_scope();
                    end_jumps.push(self.bytecode.len());
                    self.emit(OpCode::Jump(0), &stmt.span);
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
                self.emit(OpCode::JumpIfFalse(0), &stmt.span);
                self.enter_scope();
                for s in body {
                    self.compile_stmt(s, ctx);
                }
                self.exit_scope();
                self.emit(OpCode::Jump(start_p), &stmt.span);
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
                        self.emit(OpCode::SetLocal(array_slot), &stmt.span);

                        // Index counter starts at 0
                        let zero_idx = ctx.add_constant(Value::Int(0));
                        self.emit(OpCode::Constant(zero_idx), &stmt.span);
                        let index_slot = self.next_local;
                        self.next_local += 1;
                        self.emit(OpCode::SetLocal(index_slot), &stmt.span);

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
                        self.spans.push(stmt.span.clone());
                        self.emit(OpCode::MethodCall(MethodKind::Size, 0), &stmt.span);
                        self.emit(OpCode::GetLocal(index_slot), &stmt.span);
                        self.emit(OpCode::Greater, &stmt.span);
                        let exit_jmp = self.bytecode.len();
                        self.emit(OpCode::JumpIfFalse(0), &stmt.span);

                        // Load element: collection.get(index)
                        self.emit(OpCode::GetLocal(array_slot), &stmt.span);
                        self.emit(OpCode::GetLocal(index_slot), &stmt.span);
                        self.emit(OpCode::MethodCall(MethodKind::Get, 1), &stmt.span);
                        self.emit(OpCode::SetLocal(loop_var_slot), &stmt.span);

                        for s in body { self.compile_stmt(s, ctx); }

                        // Increment index
                        let cont_label = self.bytecode.len();
                        self.emit(OpCode::GetLocal(index_slot), &stmt.span);
                        if let Some(s) = step {
                            self.compile_expr(s, ctx);
                        } else {
                            let one_idx = ctx.add_constant(Value::Int(1));
                            self.bytecode.push(OpCode::Constant(one_idx));
                        }
                        self.bytecode.push(OpCode::Add);
                        self.emit(OpCode::SetLocal(index_slot), &stmt.span);
                        self.emit(OpCode::Jump(start_label), &stmt.span);

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
                        self.emit(OpCode::SetLocal(loop_var_slot), &stmt.span);

                        let start_p = self.bytecode.len();
                        self.enter_scope();
                        self.loop_stack.push((start_p, Vec::new(), Vec::new(), None));

                        // Condition: end >= var (i.e. var <= end)
                        self.compile_expr(end, ctx);
                        self.emit(OpCode::GetLocal(loop_var_slot), &stmt.span);
                        self.emit(OpCode::GreaterEqual, &stmt.span);
                        let exit_jmp = self.bytecode.len();
                        self.emit(OpCode::JumpIfFalse(0), &stmt.span);

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
                        self.emit(OpCode::Add, &stmt.span);
                        self.emit(OpCode::SetLocal(loop_var_slot), &stmt.span);
                        self.emit(OpCode::Jump(start_p), &stmt.span);

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
                        self.emit(OpCode::SetLocal(fiber_slot), &stmt.span);

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
                        self.emit(OpCode::GetLocal(fiber_slot), &stmt.span);
                        self.emit(OpCode::MethodCall(MethodKind::IsDone, 0), &stmt.span);
                        let exit_jmp = self.bytecode.len();
                        self.emit(OpCode::JumpIfTrue(0), &stmt.span);

                        // Get next value (it's already been pre-loaded by FiberIsDone but wait, we need to call Next explicitly now)
                        // Actually in the new VM, MethodKind::IsDone is just a check, it doesn't preload.
                        // So we need to call Next.
                        self.emit(OpCode::GetLocal(fiber_slot), &stmt.span);
                        self.emit(OpCode::MethodCall(MethodKind::Next, 0), &stmt.span);
                        self.emit(OpCode::SetLocal(loop_var_slot), &stmt.span);

                        for s in body { self.compile_stmt(s, ctx); }

                        let cont_label = self.bytecode.len();
                        self.emit(OpCode::Jump(start_label), &stmt.span);

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
                    self.emit(OpCode::GetLocal(fiber_slot), &stmt.span);
                    self.emit(OpCode::MethodCall(MethodKind::Close, 0), &stmt.span);
                }
                let jmp = self.bytecode.len();
                self.emit(OpCode::Jump(0), &stmt.span);
                if let Some(l) = self.loop_stack.last_mut() { l.1.push(jmp); }
            }
            crate::parser::ast::StmtKind::Continue => {
                let jmp = self.bytecode.len();
                self.emit(OpCode::Jump(0), &stmt.span);
                if let Some(l) = self.loop_stack.last_mut() { l.2.push(jmp); }
            }
            crate::parser::ast::StmtKind::ExprStmt(expr) => {
                self.compile_expr(expr, ctx);
                self.emit(OpCode::Pop, &stmt.span);
            }
            crate::parser::ast::StmtKind::Halt { level, message } => {
                self.compile_expr(message, ctx);
                match level {
                    crate::parser::ast::HaltLevel::Alert => self.emit(OpCode::HaltAlert, &stmt.span),
                    crate::parser::ast::HaltLevel::Error => self.emit(OpCode::HaltError, &stmt.span),
                    crate::parser::ast::HaltLevel::Fatal => self.emit(OpCode::HaltFatal, &stmt.span),
                }
            }
            crate::parser::ast::StmtKind::Return(expr) => {
                if let Some(e) = expr {
                    self.compile_expr(e, ctx);
                    self.emit(OpCode::Return, &stmt.span);
                } else if self.has_return_type {
                    self.emit(OpCode::Return, &stmt.span);
                } else {
                    self.emit(OpCode::ReturnVoid, &stmt.span);
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
                    bytecode: Arc::new(fc.bytecode),
                    spans: Arc::new(fc.spans),
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
                    bytecode: Arc::new(fc.bytecode),
                    spans: Arc::new(fc.spans),
                    is_fiber: true,
                    max_locals: fc.next_local,
                };
                let fid = ctx.func_indices.get(name).copied().unwrap_or(0);
                ctx.functions[fid] = chunk;
            }
            crate::parser::ast::StmtKind::FiberDecl { name, fiber_name, args, .. } => {
                for arg in args { self.compile_expr(arg, ctx); }
                let f_idx = ctx.func_indices.get(fiber_name).copied().unwrap_or(0);
                self.emit(OpCode::FiberCreate(f_idx, args.len()), &stmt.span);
                let slot = if let Some(s) = self.lookup_local(name) { s } else {
                    let s = self.next_local;
                    self.define_local(*name, s);
                    self.next_local += 1;
                    s
                };
                self.emit(OpCode::SetLocal(slot), &stmt.span);
            }
            crate::parser::ast::StmtKind::JsonBind { json, path, target } => {
                self.compile_expr(json, ctx);
                self.compile_expr(path, ctx);
                if let Some(local_idx) = self.lookup_local(target) {
                    self.emit(OpCode::JsonBindLocal(local_idx), &stmt.span);
                } else {
                    let idx = ctx.globals.get(target).copied().unwrap_or(0);
                    self.emit(OpCode::JsonBind(idx), &stmt.span);
                }
            }
            crate::parser::ast::StmtKind::JsonInject { json, mapping, table } => {
                self.compile_expr(json, ctx);
                self.compile_expr(mapping, ctx);
                if let Some(local_idx) = self.lookup_local(table) {
                    self.emit(OpCode::JsonInjectLocal(local_idx), &stmt.span);
                } else {
                    let idx = ctx.globals.get(table).copied().unwrap_or(0);
                    self.emit(OpCode::JsonInject(idx), &stmt.span);
                }
            }
            crate::parser::ast::StmtKind::Yield(expr) => {
                self.compile_expr(expr, ctx);
                self.emit(OpCode::Yield, &stmt.span);
            }
            crate::parser::ast::StmtKind::YieldFrom(expr) => {
                self.compile_expr(expr, ctx);
                let fiber_slot = self.next_local;
                self.next_local += 1;
                self.emit(OpCode::SetLocal(fiber_slot), &stmt.span);

                let start_label = self.bytecode.len();

                self.emit(OpCode::GetLocal(fiber_slot), &stmt.span);
                self.emit(OpCode::MethodCall(MethodKind::IsDone, 0), &stmt.span);
                let exit_jmp = self.bytecode.len();
                self.emit(OpCode::JumpIfTrue(0), &stmt.span);

                self.emit(OpCode::GetLocal(fiber_slot), &stmt.span);
                self.emit(OpCode::MethodCall(MethodKind::Next, 0), &stmt.span);
                self.emit(OpCode::Yield, &stmt.span);

                self.emit(OpCode::Jump(start_label), &stmt.span);

                let end_label = self.bytecode.len();
                self.bytecode[exit_jmp] = OpCode::JumpIfTrue(end_label);
            }
            crate::parser::ast::StmtKind::YieldVoid => {
                self.emit(OpCode::YieldVoid, &stmt.span);
            }
            crate::parser::ast::StmtKind::Wait(expr) => {
                self.compile_expr(expr, ctx);
                self.emit(OpCode::Wait, &stmt.span);
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
                self.emit(OpCode::HttpRequest, &stmt.span);
                
                // Set to target
                if let Some(slot) = self.lookup_local(target) {
                    self.emit(OpCode::SetLocal(slot), &stmt.span);
                } else if let Some(&idx) = ctx.globals.get(target) {
                    self.emit(OpCode::SetVar(idx), &stmt.span);
                } else {
                    // Implicit local if it's "as target"
                    let slot = self.next_local;
                    self.define_local(*target, slot);
                    self.next_local += 1;
                    self.emit(OpCode::SetLocal(slot), &stmt.span);
                }
            }
            crate::parser::ast::StmtKind::Serve { name, port, host, workers, routes } => {
                self.compile_expr(port, ctx);
                if let Some(h) = host { self.compile_expr(h, ctx); }
                else { let f = ctx.add_constant(Value::Bool(false)); self.emit(OpCode::Constant(f), &stmt.span); }
                
                if let Some(w) = workers { self.compile_expr(w, ctx); }
                else { let f = ctx.add_constant(Value::Bool(false)); self.emit(OpCode::Constant(f), &stmt.span); }
                
                self.compile_expr(routes, ctx);
                
                let s_name = ctx.interner.lookup(*name).to_string();
                let ni = ctx.add_constant(Value::String(s_name));
                self.emit(OpCode::HttpServe(ni), &stmt.span);
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
                    bytecode: Arc::new(Vec::new()),
                    spans: Arc::new(Vec::new()),
                    is_fiber: false,
                    max_locals: 0,
                });
                // Recurse into function body to find nested functions/fibers
                // but mark as NOT main script so nested VarDecls become locals.
                register_globals_recursive(body, globals, func_indices, functions, false);
            }
            crate::parser::ast::StmtKind::FiberDef { name, body, .. } => {
                let idx = functions.len();
                func_indices.insert(*name, idx);
                functions.push(FunctionChunk {
                    bytecode: Arc::new(Vec::new()),
                    spans: Arc::new(Vec::new()),
                    is_fiber: true,
                    max_locals: 0,
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
            string_constants: HashMap::new(),
        }
    }

    pub fn compile(
        &mut self,
        program: &crate::parser::ast::Program,
        interner: &mut Interner,
    ) -> (FunctionChunk, Arc<Vec<Value>>, Arc<Vec<FunctionChunk>>) {
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
            string_constants: &mut self.string_constants,
            functions:    &mut self.functions,
            func_indices: &self.func_indices,
            globals:      &self.globals,
            interner,
        };

        let mut main_compiler = FunctionCompiler::new(true, None);

        // Init built-in global slots (0:json, 1:date, 2:random, 3:store)
        let dummy_span = crate::lexer::token::Span { line: 0, col: 0, len: 0 };
        for (i, name) in ["json", "date", "random", "store"].iter().enumerate() {
            let val = ctx.add_constant(Value::String(name.to_string()));
            main_compiler.emit(OpCode::Constant(val), &dummy_span);
            main_compiler.emit(OpCode::SetVar(i), &dummy_span);
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

        main_compiler.emit(OpCode::Halt, &dummy_span);
        let main_chunk = FunctionChunk {
            bytecode: Arc::new(main_compiler.bytecode),
            spans: Arc::new(main_compiler.spans),
            is_fiber: false,
            max_locals: main_compiler.next_local,
        };
        (main_chunk, Arc::new(std::mem::take(&mut self.constants)), Arc::new(std::mem::take(&mut self.functions)))
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
        let dummy_span = crate::lexer::token::Span { line: 0, col: 0, len: 0 };
        compiler.emit(OpCode::ReturnVoid, &dummy_span);
    }

    FunctionChunk {
        bytecode: Arc::new(compiler.bytecode),
        spans: Arc::new(compiler.spans),
        is_fiber,
        max_locals: compiler.next_local,
    }
}