#[cfg(test)]
mod tests {
    use crate::parser::pratt::Parser;
    use crate::sema::checker::{Checker, TypeErrorKind};
    use crate::sema::symbol_table::SymbolTable;
    use crate::parser::ast::Type;

    #[test]
    fn test_type_mismatch() {
        let source = "i: x = \"hello\";";
        let mut parser = Parser::new(source);
        let mut program = parser.parse_program();
        let interner = parser.into_interner();

        let mut checker = Checker::new(&interner);
        let mut symbols = SymbolTable::new();
        let errors = checker.check(&mut program, &mut symbols);

        assert_eq!(errors.len(), 1);
        match &errors[0].kind {
            TypeErrorKind::TypeMismatch { expected, actual } => {
                assert_eq!(expected, &Type::Int);
                assert_eq!(actual, &Type::String);
            }
            _ => panic!("Expected TypeMismatch"),
        }
    }

    #[test]
    fn test_undefined_variable() {
        let source = ">! x;";
        let mut parser = Parser::new(source);
        let mut program = parser.parse_program();
        let interner = parser.into_interner();

        let mut checker = Checker::new(&interner);
        let mut symbols = SymbolTable::new();
        let errors = checker.check(&mut program, &mut symbols);

        assert_eq!(errors.len(), 1);
        match &errors[0].kind {
            TypeErrorKind::UndefinedVariable(name) => {
                assert_eq!(name, "x");
            }
            _ => panic!("Expected UndefinedVariable"),
        }
    }

    #[test]
    fn test_correct_types() {
        let source = "i: x = 10; f: y = 20.5; >! x + 5; >! y * 2.0;";
        let mut parser = Parser::new(source);
        let mut program = parser.parse_program();
        let interner = parser.into_interner();

        let mut checker = Checker::new(&interner);
        let mut symbols = SymbolTable::new();
        let errors = checker.check(&mut program, &mut symbols);

        assert_eq!(errors.len(), 0);
    }
}
