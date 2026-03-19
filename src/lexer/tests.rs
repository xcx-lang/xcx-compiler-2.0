#[cfg(test)]
mod tests {
    use crate::lexer::scanner::Scanner;
    use crate::lexer::token::TokenKind;

    #[test]
    fn test_basic_tokens() {
        let source = "i: x = 10; >! x;";
        let mut scanner = Scanner::new(source);
        let mut interner = crate::sema::interner::Interner::new();

        let tokens = vec![
            TokenKind::TypeI,
            TokenKind::Colon,
            TokenKind::Identifier(interner.intern("x")),
            TokenKind::Equal,
            TokenKind::IntLiteral(10),
            TokenKind::Semicolon,
            TokenKind::GreaterBang,
            TokenKind::Identifier(interner.intern("x")),
            TokenKind::Semicolon,
            TokenKind::EOF,
        ];

        for kind in tokens {
            assert_eq!(scanner.next_token(&mut interner).kind, kind);
        }
    }

    #[test]
    fn test_comments() {
        let source = "--- comment\ni: x = 10; ---\nmulti\nline\n*--- >! x;";
        let mut scanner = Scanner::new(source);
        let mut interner = crate::sema::interner::Interner::new();

        let tokens = vec![
            TokenKind::TypeI,
            TokenKind::Colon,
            TokenKind::Identifier(interner.intern("x")),
            TokenKind::Equal,
            TokenKind::IntLiteral(10),
            TokenKind::Semicolon,
            TokenKind::GreaterBang,
            TokenKind::Identifier(interner.intern("x")),
            TokenKind::Semicolon,
            TokenKind::EOF,
        ];

        for kind in tokens {
            assert_eq!(scanner.next_token(&mut interner).kind, kind);
        }
    }

    #[test]
    fn test_if_keywords() {
        let source = "if then elseif elf elif else els end";
        let mut scanner = Scanner::new(source);
        let mut interner = crate::sema::interner::Interner::new();
        assert_eq!(scanner.next_token(&mut interner).kind, TokenKind::If);
        assert_eq!(scanner.next_token(&mut interner).kind, TokenKind::Then);
        assert_eq!(scanner.next_token(&mut interner).kind, TokenKind::ElseIf);
        assert_eq!(scanner.next_token(&mut interner).kind, TokenKind::ElseIf);
        assert_eq!(scanner.next_token(&mut interner).kind, TokenKind::ElseIf);
        assert_eq!(scanner.next_token(&mut interner).kind, TokenKind::Else);
        assert_eq!(scanner.next_token(&mut interner).kind, TokenKind::Else);
        assert_eq!(scanner.next_token(&mut interner).kind, TokenKind::End);
    }
}
