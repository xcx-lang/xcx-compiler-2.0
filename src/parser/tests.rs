#[cfg(test)]
mod tests {
    use crate::parser::pratt::Parser;
    use crate::parser::ast::Type;
    use crate::lexer::token::TokenKind;

    #[test]
    fn test_var_decl() {
        let source = "i: x = 10;";
        let mut parser = Parser::new(source);
        let program = parser.parse_program();

        assert_eq!(program.stmts.len(), 1);
        match &program.stmts[0].kind {
            crate::parser::ast::StmtKind::VarDecl { ty, name: _name, value: _value, .. } => {
                assert_eq!(ty, &Type::Int);
            }
            _ => panic!("Expected VarDecl"),
        }
    }

    #[test]
    fn test_expression_precedence() {
        let source = "1 + 2 * 3 ^ 4 ^ 5;";
        let mut parser = Parser::new(source);
        let program = parser.parse_program();

        assert_eq!(program.stmts.len(), 1);
        if let crate::parser::ast::StmtKind::ExprStmt(expr) = &program.stmts[0].kind {
            if let crate::parser::ast::ExprKind::Binary { op, .. } = &expr.kind {
                assert_eq!(op, &TokenKind::Plus);
            } else {
                panic!("Expected Binary Plus at top level");
            }
        } else {
            panic!("Expected ExprStmt");
        }
    }

    #[test]
    fn test_comparisons() {
        let source = "i: a = b == c;";
        let mut parser = Parser::new(source);
        let program = parser.parse_program();

        assert_eq!(program.stmts.len(), 1);
        if let crate::parser::ast::StmtKind::VarDecl { value: Some(expr), .. } = &program.stmts[0].kind {
            if let crate::parser::ast::ExprKind::Binary { op, .. } = &expr.kind {
                assert_eq!(op, &TokenKind::EqualEqual);
            } else {
                panic!("Expected Binary EqualEqual");
            }
        } else {
            panic!("Expected VarDecl with Some value");
        }
    }

    #[test]
    fn test_func_decl_with_return_in_params() {
        let source = "func add(i: a, i: b -> i) { return a + b; };";
        let mut parser = Parser::new(source);
        let program = parser.parse_program();

        assert_eq!(program.stmts.len(), 1);
        if let crate::parser::ast::StmtKind::FunctionDef { name, params, return_type, .. } = &program.stmts[0].kind {
            let mut interner = parser.into_interner();
            let add_id = interner.intern("add");
            assert_eq!(*name, add_id);
            assert_eq!(params.len(), 2);
            assert_eq!(return_type, &Some(Type::Int));
        } else {
            panic!("Expected FunctionDef");
        }
    }
}
