/// Integration tests for the XCX compiler backend.
///
/// Each test runs a complete pipeline: source → lexer → parser → checker → compiler → VM.
/// Tests are grouped by the bug they cover so regressions are immediately identifiable.

#[cfg(test)]
mod tests {
    use crate::parser::pratt::Parser;
    use crate::sema::checker::Checker;
    use crate::sema::symbol_table::SymbolTable;
    use crate::backend::Compiler as XCXCompiler;
    use crate::backend::vm::{VM, Value, VMContext};

    // -------------------------------------------------------------------------
    // Helper: run a source string through the full pipeline and return the VM.
    // -------------------------------------------------------------------------
    fn run(source: &str) -> VM {
        let mut parser = Parser::new(source);
        let mut program = parser.parse_program();
        let mut interner = parser.into_interner();

        let mut checker = Checker::new(&interner);
        let mut symbols = SymbolTable::new();
        let errors = checker.check(&mut program, &mut symbols);
        assert!(
            errors.is_empty(),
            "Type-check errors in test source:\n{:?}",
            errors
        );

        let mut compiler = XCXCompiler::new();
        let (bytecode, constants, functions) = compiler.compile(&program, &mut interner);

        let mut vm = VM::new();
        let ctx = VMContext { constants: &constants, functions: &functions };
        vm.run(&bytecode, &ctx);
        vm
    }


    // -------------------------------------------------------------------------
    // Sanity test — basic arithmetic (original smoke test, kept for reference).
    // -------------------------------------------------------------------------
    #[test]
    fn test_basic_arithmetic() {
        // Declare two ints, print their sum. Checks that the whole pipeline works.
        run("i: x = 10; i: y = 20; >! x + y;");
    }

    // -------------------------------------------------------------------------
    // REPL — Parser::new_with_interner was called without `source`.
    //
    // Regression: confirm that Parser::new (which internally calls
    // new_with_interner with the source) compiles without panic and that a
    // program parsed this way produces no errors.
    // -------------------------------------------------------------------------
    #[test]
    fn test_repl_parser_new_accepts_source() {
        // Parser::new(source) must accept a string slice and produce a working
        // program — if the function signature were broken this would not compile.
        let source = "i: a = 42;";
        let mut parser = Parser::new(source);
        let mut program = parser.parse_program();
        let interner = parser.into_interner();

        let mut checker = Checker::new(&interner);
        let mut symbols = SymbolTable::new();
        let errors = checker.check(&mut program, &mut symbols);
        assert!(errors.is_empty(), "Unexpected type errors: {:?}", errors);
    }

    // -------------------------------------------------------------------------
    // Regression: Ensure no debug prints in production.
    //
    // Compiling a method call that took the "fallback" branch in
    // compile_func_expr used to emit a println!("DEBUG_COMPILE: ...").
    // We verify the output goes to stdout correctly (i.e. the program outputs
    // something via >! not the debug message). Since we can't easily capture
    // stdout in a unit test, we at minimum confirm the pipeline runs without
    // panicking and the bytecode is generated.
    // -------------------------------------------------------------------------
    #[test]
    fn test_no_debug_print_on_method_call() {
        // `count` on a table triggers the fallback MethodCall path.  
        // The test just ensures it doesn't panic/crash — the debug line has
        // been removed so nothing leaks to stdout.
        let source = r#"
            table: t = table {
                columns: [id :: i @auto, name :: s]
                rows: [("Alice"), ("Bob")]
            };
            i: n = t.count();
            >! n;
        "#;
        run(source); // must not panic
    }

    // -------------------------------------------------------------------------
    // Regression: Unary negation of a float literal crashed the VM.
    //
    // `-3.14` was compiled as `Int(0) - Float(3.14)` which hit the "Sub" arm
    // with mismatched types.  After the fix it emits `Float(0.0) - Float(3.14)`.
    // -------------------------------------------------------------------------
    #[test]
    fn test_unary_negation_float_does_not_crash() {
        // -3.14 assignment: f: x = -3.14;
        let source = "f: x = -3.14;";
        run(source); // must not panic with a runtime type error
    }

    #[test]
    fn test_unary_negation_int_still_works() {
        // Ensure the existing int-negation path is unbroken.
        let source = "i: x = -7;";
        run(source);
    }

    #[test]
    fn test_unary_negation_float_value_is_correct() {
        // Confirm the computed value is actually negative.
        // We store into a variable and retrieve it via get_global.
        let source = "f: result = -2.5;";

        let mut parser = Parser::new(source);
        let mut program = parser.parse_program();
        let mut interner = parser.into_interner();

        let mut checker = Checker::new(&interner);
        let mut symbols = SymbolTable::new();
        let errors = checker.check(&mut program, &mut symbols);
        assert!(errors.is_empty(), "{:?}", errors);

        let name_id = interner.intern("result");
        let mut compiler = XCXCompiler::new();
        let (bytecode, constants, functions) = compiler.compile(&program, &mut interner);
        let global_idx = compiler.get_global_idx(name_id);

        let mut vm = VM::new();
        let ctx = VMContext { constants: &constants, functions: &functions };
        vm.run(&bytecode, &ctx);

        match vm.get_global(global_idx) {
            Some(Value::Float(v)) => {
                assert!(
                    (v - (-2.5_f64)).abs() < 1e-9,
                    "Expected -2.5, got {}",
                    v
                );
            }
            other => panic!("Expected Float(-2.5), got {:?}", other),
        }
    }

    // -------------------------------------------------------------------------
    // Regression: halt.error did not stop the current frame.
    //
    // After halt.error executes, no further statements in the same frame
    // must be executed. We verify this by placing a variable assignment AFTER
    // halt.error — that assignment must NOT run, leaving the variable at its
    // default value.
    // -------------------------------------------------------------------------
    #[test]
    fn test_halt_error_stops_current_frame() {
        // `sentinel` starts at 0. After halt.error >! "..." the `i: sentinel = 99`
        // assignment must NOT execute. If halt.error is broken, sentinel becomes 99.
        //
        // Per the XCX spec / parse_halt_stmt, the syntax is:
        //   halt.error >! "message";
        let source = "i: sentinel = 0;\nhalt.error >! \"stopping here\";\ni: sentinel = 99;";

        let mut parser = Parser::new(source);
        let mut program = parser.parse_program();
        let mut interner = parser.into_interner();

        let mut checker = Checker::new(&interner);
        let mut symbols = SymbolTable::new();
        let _ = checker.check(&mut program, &mut symbols);

        let name_id = interner.intern("sentinel");
        let mut compiler = XCXCompiler::new();
        let (bytecode, constants, functions) = compiler.compile(&program, &mut interner);
        let global_idx = compiler.get_global_idx(name_id);

        let mut vm = VM::new();
        let ctx = VMContext { constants: &constants, functions: &functions };
        vm.run(&bytecode, &ctx);

        match vm.get_global(global_idx) {
            Some(Value::Int(v)) => assert_eq!(
                v, 0,
                "halt.error failed to stop the frame — sentinel was mutated to {}",
                v
            ),
            Some(Value::Bool(false)) => {} // slot never reached — halt.error stopped execution
            other => panic!("Unexpected value for sentinel: {:?}", other),
        }
    }

    // -------------------------------------------------------------------------
    // Regression: Globals Vec was fixed at 1024 entries.
    //
    // Declare more than 1024 distinct global variables. Before the fix this
    // would either panic (OOB) or silently corrupt memory. After the fix
    // the Vec grows automatically.
    // -------------------------------------------------------------------------
    #[test]
    fn test_globals_exceed_1024() {
        // Generate a program that declares 1030 distinct global int variables.
        let mut source = String::new();
        for i in 0..1030 {
            source.push_str(&format!("i: var{i} = {i};\n"));
        }
        // Print the last one so the compiler keeps it.
        source.push_str(">! var1029;");

        run(&source); // must not panic or index-out-of-bounds
    }

    // -------------------------------------------------------------------------
    // Professional HTTP Tests: Client and SSRF
    // -------------------------------------------------------------------------

    #[test]
    fn test_http_client_local_server() {
        use std::thread;
        
        // Start a tiny_http server on a random port
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let addr_str = server.server_addr().to_string();
        let port = addr_str.split(':').last().unwrap().parse::<u16>().unwrap();
        
        // Spawn responder thread
        thread::spawn(move || {
            if let Ok(Some(request)) = server.recv_timeout(std::time::Duration::from_secs(5)) {
                let response = tiny_http::Response::from_string("{\"hello\":\"world\"}")
                    .with_status_code(200);
                let _ = request.respond(response);
            }
        });
        
        let source = format!(r#"
            i: success = 0;
            json: res = net.get("http://127.0.0.1:{}");
            if (res.ok) then;
                if (res.body.hello == "world") then;
                    success = 42;
                end;
            end;
        "#, port);
        
        let mut parser = Parser::new(&source);
        let mut program = parser.parse_program();
        let mut interner = parser.into_interner();
        let mut checker = Checker::new(&interner);
        let mut symbols = SymbolTable::new();
        let _ = checker.check(&mut program, &mut symbols);
        
        let success_id = interner.intern("success");
        let mut compiler = XCXCompiler::new();
        let (bytecode, constants, functions) = compiler.compile(&program, &mut interner);
        let success_idx = compiler.get_global_idx(success_id);
        
        let mut vm = VM::new();
        let ctx = VMContext { constants: &constants, functions: &functions };
        vm.run(&bytecode, &ctx);
        
        match vm.get_global(success_idx) {
            Some(Value::Int(42)) => {},
            other => panic!("HTTP integration test failed! Expected success=42, got {:?}", other),
        }
    }

    #[test]
    fn test_ssrf_protection_link_local() {
        let source = r#"
            json: res = net.get("http://169.254.169.254/latest/meta-data/");
            s: err = res.error;
        "#;
        let mut parser = Parser::new(source);
        let mut program = parser.parse_program();
        let mut interner = parser.into_interner();
        let mut checker = Checker::new(&interner);
        let mut symbols = SymbolTable::new();
        let _ = checker.check(&mut program, &mut symbols);
        
        let err_id = interner.intern("err");
        let mut compiler = XCXCompiler::new();
        let (bytecode, constants, functions) = compiler.compile(&program, &mut interner);
        let err_idx = compiler.get_global_idx(err_id);
        
        let mut vm = VM::new();
        let ctx = VMContext { constants: &constants, functions: &functions };
        vm.run(&bytecode, &ctx);
        
        match vm.get_global(err_idx) {
            Some(Value::String(s)) => assert!(s.contains("SSRF")),
            other => panic!("SSRF protection test failed! Expected error string, got {:?}", other),
        }
    }

    // -------------------------------------------------------------------------
    // FAZA 1 Tests — String Methods
    // -------------------------------------------------------------------------

    #[test]
    fn test_string_starts_with_true() {
        let source = r#"
            b: result = "admin@xcx.pl".startsWith("admin");
        "#;
        let mut parser = Parser::new(source);
        let mut program = parser.parse_program();
        let mut interner = parser.into_interner();
        let mut checker = Checker::new(&interner);
        let mut symbols = SymbolTable::new();
        let _ = checker.check(&mut program, &mut symbols);
        let id = interner.intern("result");
        let mut compiler = XCXCompiler::new();
        let (bc, consts, funcs) = compiler.compile(&mut program, &mut interner);
        let idx = compiler.get_global_idx(id);
        let mut vm = VM::new();
        let ctx = VMContext { constants: &consts, functions: &funcs };
        vm.run(&bc, &ctx);
        assert_eq!(vm.get_global(idx), Some(Value::Bool(true)), "startsWith(\"admin\") should be true");
    }

    #[test]
    fn test_string_starts_with_false() {
        let source = r#"
            b: result = "xcx@xcx.pl".startsWith("admin");
        "#;
        let mut parser = Parser::new(source);
        let mut program = parser.parse_program();
        let mut interner = parser.into_interner();
        let mut checker = Checker::new(&interner);
        let mut symbols = SymbolTable::new();
        let _ = checker.check(&mut program, &mut symbols);
        let id = interner.intern("result");
        let mut compiler = XCXCompiler::new();
        let (bc, consts, funcs) = compiler.compile(&mut program, &mut interner);
        let idx = compiler.get_global_idx(id);
        let mut vm = VM::new();
        let ctx = VMContext { constants: &consts, functions: &funcs };
        vm.run(&bc, &ctx);
        assert_eq!(vm.get_global(idx), Some(Value::Bool(false)), "startsWith(\"admin\") should be false");
    }

    #[test]
    fn test_string_ends_with_true() {
        let source = r#"
            b: result = "main.xcx".endsWith(".xcx");
        "#;
        let mut parser = Parser::new(source);
        let mut program = parser.parse_program();
        let mut interner = parser.into_interner();
        let mut checker = Checker::new(&interner);
        let mut symbols = SymbolTable::new();
        let _ = checker.check(&mut program, &mut symbols);
        let id = interner.intern("result");
        let mut compiler = XCXCompiler::new();
        let (bc, consts, funcs) = compiler.compile(&mut program, &mut interner);
        let idx = compiler.get_global_idx(id);
        let mut vm = VM::new();
        let ctx = VMContext { constants: &consts, functions: &funcs };
        vm.run(&bc, &ctx);
        assert_eq!(vm.get_global(idx), Some(Value::Bool(true)), "endsWith(\".xcx\") should be true");
    }

    #[test]
    fn test_string_ends_with_false() {
        let source = r#"
            b: result = "main.xcx".endsWith(".rs");
        "#;
        let mut parser = Parser::new(source);
        let mut program = parser.parse_program();
        let mut interner = parser.into_interner();
        let mut checker = Checker::new(&interner);
        let mut symbols = SymbolTable::new();
        let _ = checker.check(&mut program, &mut symbols);
        let id = interner.intern("result");
        let mut compiler = XCXCompiler::new();
        let (bc, consts, funcs) = compiler.compile(&mut program, &mut interner);
        let idx = compiler.get_global_idx(id);
        let mut vm = VM::new();
        let ctx = VMContext { constants: &consts, functions: &funcs };
        vm.run(&bc, &ctx);
        assert_eq!(vm.get_global(idx), Some(Value::Bool(false)), "endsWith(\".rs\") should be false");
    }

    #[test]
    fn test_string_to_int_valid() {
        let source = r#"
            i: result = "42".toInt();
        "#;
        let mut parser = Parser::new(source);
        let mut program = parser.parse_program();
        let mut interner = parser.into_interner();
        let mut checker = Checker::new(&interner);
        let mut symbols = SymbolTable::new();
        let _ = checker.check(&mut program, &mut symbols);
        let id = interner.intern("result");
        let mut compiler = XCXCompiler::new();
        let (bc, consts, funcs) = compiler.compile(&mut program, &mut interner);
        let idx = compiler.get_global_idx(id);
        let mut vm = VM::new();
        let ctx = VMContext { constants: &consts, functions: &funcs };
        vm.run(&bc, &ctx);
        assert_eq!(vm.get_global(idx), Some(Value::Int(42)), ".toInt() should return 42");
    }

    #[test]
    fn test_string_to_float_valid() {
        let source = r#"
            f: result = "3.14".toFloat();
        "#;
        let mut parser = Parser::new(source);
        let mut program = parser.parse_program();
        let mut interner = parser.into_interner();
        let mut checker = Checker::new(&interner);
        let mut symbols = SymbolTable::new();
        let _ = checker.check(&mut program, &mut symbols);
        let id = interner.intern("result");
        let mut compiler = XCXCompiler::new();
        let (bc, consts, funcs) = compiler.compile(&mut program, &mut interner);
        let idx = compiler.get_global_idx(id);
        let mut vm = VM::new();
        let ctx = VMContext { constants: &consts, functions: &funcs };
        vm.run(&bc, &ctx);
        match vm.get_global(idx) {
            Some(Value::Float(f)) => assert!((f - 3.14_f64).abs() < 1e-9, "Expected 3.14, got {}", f),
            other => panic!(".toFloat() expected Float(3.14), got {:?}", other),
        }
    }

    #[test]
    fn test_string_to_int_with_whitespace() {
        // .toInt() should trim before parsing
        let source = r#"
            i: result = "  99  ".toInt();
        "#;
        let mut parser = Parser::new(source);
        let mut program = parser.parse_program();
        let mut interner = parser.into_interner();
        let mut checker = Checker::new(&interner);
        let mut symbols = SymbolTable::new();
        let _ = checker.check(&mut program, &mut symbols);
        let id = interner.intern("result");
        let mut compiler = XCXCompiler::new();
        let (bc, consts, funcs) = compiler.compile(&mut program, &mut interner);
        let idx = compiler.get_global_idx(id);
        let mut vm = VM::new();
        let ctx = VMContext { constants: &consts, functions: &funcs };
        vm.run(&bc, &ctx);
        assert_eq!(vm.get_global(idx), Some(Value::Int(99)), ".toInt() should handle whitespace");
    }

    // -------------------------------------------------------------------------
    // FAZA 2 Tests — Array Methods: sort() and reverse()
    // -------------------------------------------------------------------------

    #[test]
    fn test_array_sort_integers() {
        // sort() is in-place; check first element is smallest after sort
        let source = r#"
            array:i: nums {5, 2, 8, 1, 9};
            nums.sort();
            i: first = nums.get(0);
            i: last  = nums.get(4);
        "#;
        let mut parser = Parser::new(source);
        let mut program = parser.parse_program();
        let mut interner = parser.into_interner();
        let mut checker = Checker::new(&interner);
        let mut symbols = SymbolTable::new();
        let _ = checker.check(&mut program, &mut symbols);
        let first_id = interner.intern("first");
        let last_id  = interner.intern("last");
        let mut compiler = XCXCompiler::new();
        let (bc, consts, funcs) = compiler.compile(&program, &mut interner);
        let first_idx = compiler.get_global_idx(first_id);
        let last_idx  = compiler.get_global_idx(last_id);
        let mut vm = VM::new();
        let ctx = VMContext { constants: &consts, functions: &funcs };
        vm.run(&bc, &ctx);
        assert_eq!(vm.get_global(first_idx), Some(Value::Int(1)), "After sort first element should be 1");
        assert_eq!(vm.get_global(last_idx),  Some(Value::Int(9)), "After sort last element should be 9");
    }

    #[test]
    fn test_array_sort_strings() {
        let source = r#"
            array:s: words {"banana", "apple", "cherry"};
            words.sort();
            s: first = words.get(0);
        "#;
        let mut parser = Parser::new(source);
        let mut program = parser.parse_program();
        let mut interner = parser.into_interner();
        let mut checker = Checker::new(&interner);
        let mut symbols = SymbolTable::new();
        let _ = checker.check(&mut program, &mut symbols);
        let first_id = interner.intern("first");
        let mut compiler = XCXCompiler::new();
        let (bc, consts, funcs) = compiler.compile(&program, &mut interner);
        let first_idx = compiler.get_global_idx(first_id);
        let mut vm = VM::new();
        let ctx = VMContext { constants: &consts, functions: &funcs };
        vm.run(&bc, &ctx);
        assert_eq!(
            vm.get_global(first_idx),
            Some(Value::String("apple".to_string())),
            "After sort first string should be 'apple'"
        );
    }

    #[test]
    fn test_array_reverse_integers() {
        let source = r#"
            array:i: nums {1, 2, 3, 4, 5};
            nums.reverse();
            i: first = nums.get(0);
            i: last  = nums.get(4);
        "#;
        let mut parser = Parser::new(source);
        let mut program = parser.parse_program();
        let mut interner = parser.into_interner();
        let mut checker = Checker::new(&interner);
        let mut symbols = SymbolTable::new();
        let _ = checker.check(&mut program, &mut symbols);
        let first_id = interner.intern("first");
        let last_id  = interner.intern("last");
        let mut compiler = XCXCompiler::new();
        let (bc, consts, funcs) = compiler.compile(&program, &mut interner);
        let first_idx = compiler.get_global_idx(first_id);
        let last_idx  = compiler.get_global_idx(last_id);
        let mut vm = VM::new();
        let ctx = VMContext { constants: &consts, functions: &funcs };
        vm.run(&bc, &ctx);
        assert_eq!(vm.get_global(first_idx), Some(Value::Int(5)), "After reverse first should be 5");
        assert_eq!(vm.get_global(last_idx),  Some(Value::Int(1)), "After reverse last should be 1");
    }

    #[test]
    fn test_array_sort_then_reverse() {
        // sort ascending then reverse = descending
        let source = r#"
            array:i: nums {3, 1, 4, 1, 5, 9, 2, 6};
            nums.sort();
            nums.reverse();
            i: first = nums.get(0);
        "#;
        let mut parser = Parser::new(source);
        let mut program = parser.parse_program();
        let mut interner = parser.into_interner();
        let mut checker = Checker::new(&interner);
        let mut symbols = SymbolTable::new();
        let _ = checker.check(&mut program, &mut symbols);
        let first_id = interner.intern("first");
        let mut compiler = XCXCompiler::new();
        let (bc, consts, funcs) = compiler.compile(&program, &mut interner);
        let first_idx = compiler.get_global_idx(first_id);
        let mut vm = VM::new();
        let ctx = VMContext { constants: &consts, functions: &funcs };
        vm.run(&bc, &ctx);
        assert_eq!(vm.get_global(first_idx), Some(Value::Int(9)), "After sort+reverse first should be 9");
    }

    #[test]
    fn test_wait_ms() {
        let source = r#"
            @wait(10);
            b: result = true;
        "#;
        let mut parser = Parser::new(source);
        let mut program = parser.parse_program();
        let mut interner = parser.into_interner();
        let mut checker = Checker::new(&interner);
        let mut symbols = SymbolTable::new();
        let _ = checker.check(&mut program, &mut symbols);
        let id = interner.intern("result");
        let mut compiler = XCXCompiler::new();
        let (bc, consts, funcs) = compiler.compile(&program, &mut interner);
        let idx = compiler.get_global_idx(id);
        let mut vm = VM::new();
        let ctx = VMContext { constants: &consts, functions: &funcs };
        vm.run(&bc, &ctx);
        assert_eq!(vm.get_global(idx), Some(Value::Bool(true)), "@wait(10) should execute and allow next stmt");
    }

    #[test]
    fn test_env_get() {
        unsafe { std::env::set_var("XCX_TEST_VAR", "hello_xcx"); }
        
        let source = r#"
            s: val = env.get("XCX_TEST_VAR");
        "#;
        let mut parser = Parser::new(source);
        let mut program = parser.parse_program();
        let mut interner = parser.into_interner();
        let mut checker = Checker::new(&interner);
        let mut symbols = SymbolTable::new();
        let _ = checker.check(&mut program, &mut symbols);
        let id = interner.intern("val");
        let mut compiler = XCXCompiler::new();
        let (bc, consts, funcs) = compiler.compile(&program, &mut interner);
        let idx = compiler.get_global_idx(id);
        let mut vm = VM::new();
        let ctx = VMContext { constants: &consts, functions: &funcs };
        vm.run(&bc, &ctx);
        assert_eq!(vm.get_global(idx), Some(Value::String("hello_xcx".to_string())), "env.get should retrieve XCX_TEST_VAR");
    }

    #[test]
    fn test_crypto_bcrypt() {
        let source = r#"
            s: pass = "super-secret";
            s: hashed = crypto.hash(pass, "bcrypt");
            b: ok = crypto.verify(pass, hashed, "bcrypt");
            b: fail = crypto.verify("wrong", hashed, "bcrypt");
        "#;
        let mut parser = Parser::new(source);
        let mut program = parser.parse_program();
        let mut interner = parser.into_interner();
        let mut checker = Checker::new(&interner);
        let mut symbols = SymbolTable::new();
        let _ = checker.check(&mut program, &mut symbols);
        let ok_id = interner.intern("ok");
        let fail_id = interner.intern("fail");
        let mut compiler = XCXCompiler::new();
        let (bc, consts, funcs) = compiler.compile(&program, &mut interner);
        let ok_idx = compiler.get_global_idx(ok_id);
        let fail_idx = compiler.get_global_idx(fail_id);
        let mut vm = VM::new();
        let ctx = VMContext { constants: &consts, functions: &funcs };
        vm.run(&bc, &ctx);
        assert_eq!(vm.get_global(ok_idx), Some(Value::Bool(true)), "bcrypt verify should be true for correct password");
        assert_eq!(vm.get_global(fail_idx), Some(Value::Bool(false)), "bcrypt verify should be false for wrong password");
    }

    #[test]
    fn test_crypto_argon2() {
        let source = r#"
            s: pass = "argon-secret";
            s: hashed = crypto.hash(pass, "argon2");
            b: ok = crypto.verify(pass, hashed, "argon2");
        "#;
        let mut parser = Parser::new(source);
        let mut program = parser.parse_program();
        let mut interner = parser.into_interner();
        let mut checker = Checker::new(&interner);
        let mut symbols = SymbolTable::new();
        let _ = checker.check(&mut program, &mut symbols);
        let ok_id = interner.intern("ok");
        let mut compiler = XCXCompiler::new();
        let (bc, consts, funcs) = compiler.compile(&program, &mut interner);
        let ok_idx = compiler.get_global_idx(ok_id);
        let mut vm = VM::new();
        let ctx = VMContext { constants: &consts, functions: &funcs };
        vm.run(&bc, &ctx);
        assert_eq!(vm.get_global(ok_idx), Some(Value::Bool(true)), "argon2 verify should be true for correct password");
    }

    #[test]
    fn test_crypto_token() {
        let source = r#"
            s: t1 = crypto.token(16);
            s: t2 = crypto.token(32);
            i: len1 = t1.length;
            i: len2 = t2.length;
        "#;
        let mut parser = Parser::new(source);
        let mut program = parser.parse_program();
        let mut interner = parser.into_interner();
        let mut checker = Checker::new(&interner);
        let mut symbols = SymbolTable::new();
        let _ = checker.check(&mut program, &mut symbols);
        let l1_id = interner.intern("len1");
        let l2_id = interner.intern("len2");
        let mut compiler = XCXCompiler::new();
        let (bc, consts, funcs) = compiler.compile(&program, &mut interner);
        let l1_idx = compiler.get_global_idx(l1_id);
        let l2_idx = compiler.get_global_idx(l2_id);
        let mut vm = VM::new();
        let ctx = VMContext { constants: &consts, functions: &funcs };
        vm.run(&bc, &ctx);
        assert_eq!(vm.get_global(l1_idx), Some(Value::Int(16)));
        assert_eq!(vm.get_global(l2_idx), Some(Value::Int(32)));
    }
}
