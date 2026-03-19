use crate::lexer::token::{Token, TokenKind};
use crate::sema::interner::Interner;

pub struct Scanner {
    chars: Vec<char>,
    pos: usize,
    line: usize,
    col: usize,
}

impl Scanner {
    pub fn new(source: &str) -> Self {
        Self {
            chars: source.chars().collect(),
            pos: 0,
            line: 1,
            col: 1,
        }
    }

    pub fn next_token(&mut self, interner: &mut Interner) -> Token {
        self.skip_whitespace_and_comments();

        let line = self.line;
        let col = self.col;

        if self.is_at_end() {
             return Token::new(TokenKind::EOF, line, col, 0);
        }

        let c = self.advance();

        if c.is_ascii_digit() {
            return self.number(c, line, col);
        }

        if c.is_alphabetic() || c == '_' {
            return self.identifier(c, line, col, interner);
        }

        match c {
            '(' => Token::new(TokenKind::LeftParen, line, col, 1),
            ')' => Token::new(TokenKind::RightParen, line, col, 1),
            '{' => Token::new(TokenKind::LeftBrace, line, col, 1),
            '}' => Token::new(TokenKind::RightBrace, line, col, 1),
            '[' => Token::new(TokenKind::LeftBracket, line, col, 1),
            ']' => Token::new(TokenKind::RightBracket, line, col, 1),
            ',' => {
                if self.peek() == ',' {
                    self.advance();
                    Token::new(TokenKind::DoubleComma, line, col, 2)
                } else {
                    Token::new(TokenKind::Comma, line, col, 1)
                }
            }
            '.' => {
                if self.peek() == '.' {
                    self.advance();
                    Token::new(TokenKind::To, line, col, 2)
                } else {
                    Token::new(TokenKind::Dot, line, col, 1)
                }
            }
            ';' => Token::new(TokenKind::Semicolon, line, col, 1),
            ':' => {
                if self.peek() == ':' {
                    self.advance();
                    Token::new(TokenKind::DoubleColon, line, col, 2)
                } else {
                    Token::new(TokenKind::Colon, line, col, 1)
                }
            }
            '+' => {
                if self.peek() == '+' {
                    self.advance();
                    Token::new(TokenKind::PlusPlus, line, col, 2)
                } else {
                    Token::new(TokenKind::Plus, line, col, 1)
                }
            }
            '-' => {
                if self.peek() == '>' {
                    self.advance();
                    Token::new(TokenKind::Arrow, line, col, 2)
                } else {
                    Token::new(TokenKind::Minus, line, col, 1)
                }
            }
            '*' => Token::new(TokenKind::Star, line, col, 1),
            '/' => Token::new(TokenKind::Slash, line, col, 1),
            '%' => Token::new(TokenKind::Percent, line, col, 1),
            '^' => Token::new(TokenKind::Caret, line, col, 1),
            '!' => {
                if self.peek() == '=' {
                    self.advance();
                    Token::new(TokenKind::BangEqual, line, col, 2)
                } else if self.peek() == '!' {
                    self.advance();
                    Token::new(TokenKind::Not, line, col, 2)
                } else {
                    Token::new(TokenKind::Bang, line, col, 1)
                }
            }
            '=' => {
                if self.peek() == '=' {
                    self.advance();
                    Token::new(TokenKind::EqualEqual, line, col, 2)
                } else {
                    Token::new(TokenKind::Equal, line, col, 1)
                }
            }
            '<' => {
                if self.peek() == '=' {
                    self.advance();
                    Token::new(TokenKind::LessEqual, line, col, 2)
                } else if self.peek() == '-' && self.peek_next() == '>' {
                    self.advance();
                    self.advance();
                    Token::new(TokenKind::Bridge, line, col, 3)
                } else if self.peek() == '<' && self.peek_next() == '<' {
                    self.advance();
                    self.advance();
                    return self.raw_block(line, col, interner);
                } else {
                    Token::new(TokenKind::Less, line, col, 1)
                }
            }
            '>' => {
                if self.peek() == '=' {
                    self.advance();
                    Token::new(TokenKind::GreaterEqual, line, col, 2)
                } else if self.peek() == '!' {
                    self.advance();
                    Token::new(TokenKind::GreaterBang, line, col, 2)
                } else if self.peek() == '?' {
                    self.advance();
                    Token::new(TokenKind::GreaterQuestion, line, col, 2)
                } else {
                    Token::new(TokenKind::Greater, line, col, 1)
                }
            }
            '@' => {
                let start_col = col;
                let mut name = String::new();
                while self.peek().is_alphabetic() {
                    name.push(self.advance());
                }
                match name.as_str() {
                    "step" => Token::new(TokenKind::AtStep, line, start_col, name.len() + 1),
                    "auto" => Token::new(TokenKind::AtAuto, line, start_col, name.len() + 1),
                    "wait" => Token::new(TokenKind::AtWait, line, start_col, name.len() + 1),
                    _ => Token::new(TokenKind::Unknown('@'), line, start_col, 1),
                }
            }
            '&' => {
                if self.peek() == '&' {
                    self.advance();
                    Token::new(TokenKind::And, line, col, 2)
                } else {
                    Token::new(TokenKind::Unknown('&'), line, col, 1)
                }
            }
            '|' => {
                if self.peek() == '|' {
                    self.advance();
                    Token::new(TokenKind::Or, line, col, 2)
                } else {
                    Token::new(TokenKind::Unknown('|'), line, col, 1)
                }
            }
            '\"' => {
                self.string(line, col, interner)
            }
            '∪' => Token::new(TokenKind::Union, line, col, 1),
            '∩' => Token::new(TokenKind::Intersection, line, col, 1),
            '\\' => Token::new(TokenKind::Difference, line, col, 1),
            '⊕' => Token::new(TokenKind::SymDifference, line, col, 1),
            _ => Token::new(TokenKind::Unknown(c), line, col, 1),
        }
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            let c = self.peek();
            if c.is_whitespace() {
                self.advance();
            } else if c == '-' && self.peek_next() == '-' && self.peek_at(2) == '-' {
                self.advance(); // -
                self.advance(); // -
                self.advance(); // -
                
                let mut is_multi = true;
                let mut temp_pos = self.pos;
                while temp_pos < self.chars.len() && self.chars[temp_pos] != '\n' {
                    if !self.chars[temp_pos].is_whitespace() {
                        is_multi = false;
                        break;
                    }
                    temp_pos += 1;
                }
                
                if !is_multi {
                    while self.peek() != '\n' && self.peek() != '\0' {
                        self.advance();
                    }
                } else {
                    while self.peek() != '\0' {
                        if self.peek() == '*' && self.peek_at(1) == '-' && self.peek_at(2) == '-' && self.peek_at(3) == '-' {
                            self.advance(); // *
                            self.advance(); // -
                            self.advance(); // -
                            self.advance(); // -
                            break;
                        }
                        self.advance();
                    }
                }
            } else {
                break;
            }
        }
    }

    fn peek(&self) -> char {
        self.peek_at(0)
    }

    fn peek_next(&self) -> char {
        self.peek_at(1)
    }

    fn peek_at(&self, offset: usize) -> char {
        if self.pos + offset >= self.chars.len() {
            '\0'
        } else {
            self.chars[self.pos + offset]
        }
    }

    fn advance(&mut self) -> char {
        let c = self.chars[self.pos];
        self.pos += 1;
        if c == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        c
    }

    fn is_at_end(&self) -> bool {
        self.pos >= self.chars.len()
    }

    fn identifier(&mut self, _first: char, line: usize, col: usize, interner: &mut Interner) -> Token {
        let start_pos = self.pos - 1;
        while self.peek().is_alphanumeric() || self.peek() == '_' {
            self.advance();
        }
        let text: String = self.chars[start_pos..self.pos].iter().collect();
        let lower_text = text.to_lowercase();

        let kind = match text.as_str() {
            "UNION" => TokenKind::Union,
            "INTERSECTION" => TokenKind::Intersection,
            "DIFFERENCE" => TokenKind::Difference,
            "SYMMETRIC_DIFFERENCE" => TokenKind::SymDifference,
            "HAS" => TokenKind::Has,
            "AND" => TokenKind::And,
            "OR" => TokenKind::Or,
            "NOT" => TokenKind::Not,
            "N" => TokenKind::TypeSetN,
            "Q" => TokenKind::TypeSetQ,
            "Z" => TokenKind::TypeSetZ,
            "S" => TokenKind::TypeSetS,
            "B" => TokenKind::TypeSetB,
            "C" => TokenKind::TypeSetC,
            _ => match lower_text.as_str() {
                "const" => TokenKind::Const,
                "i" | "int" => TokenKind::TypeI,
                "f" | "float" => TokenKind::TypeF,
                "s" | "string" => TokenKind::TypeS,
                "b" | "bool" | "boolean" => TokenKind::TypeB,
                "true" => TokenKind::True,
                "false" => TokenKind::False,
                "if" => TokenKind::If,
                "then" => TokenKind::Then,
                "elseif" | "elif" | "elf" => TokenKind::ElseIf,
                "else" | "els" => {
                    self.skip_whitespace_and_comments();
                    let after_ws_pos = self.pos;
                    let after_ws_line = self.line;
                    let after_ws_col = self.col;
                    let next_start = self.pos;
                    while self.peek().is_alphanumeric() || self.peek() == '_' {
                        self.advance();
                    }
                    let next_text: String = self.chars[next_start..self.pos].iter().collect();
                    if next_text == "if" {
                        TokenKind::ElseIf
                    } else {
                        self.pos = after_ws_pos;
                        self.line = after_ws_line;
                        self.col = after_ws_col;
                        TokenKind::Else
                    }
                }
                "end" => TokenKind::End,
                "for" => TokenKind::For,
                "in" => TokenKind::In,
                "to" => TokenKind::To,
                "while" => TokenKind::While,
                "do" => TokenKind::Do,
                "break" => TokenKind::Break,
                "continue" => TokenKind::Continue,
                "and" => TokenKind::And,
                "or" => TokenKind::Or,
                "not" => TokenKind::Not,
                "has" => TokenKind::Has,
                "halt" => TokenKind::Halt,
                "alert" => TokenKind::Alert,
                "error" => TokenKind::Error,
                "fatal" => TokenKind::Fatal,
                "terminal" => TokenKind::Terminal,
                "func" | "function" | "fun" => TokenKind::Func,
                "return" => TokenKind::Return,
                "array" => TokenKind::Array,
                "set" => TokenKind::Set,
                "map" => TokenKind::Map,
                "date" => TokenKind::Date,
                "store" => TokenKind::Store,
                "schema" => TokenKind::Schema,
                "data" => TokenKind::Data,
                "table" => TokenKind::Table,
                "columns" => TokenKind::Columns,
                "rows" => TokenKind::Rows,
                "json" => TokenKind::Json,
                "net" => TokenKind::Net,
                "serve" => TokenKind::Serve,
                "fiber" => TokenKind::Fiber,
                "yield" => TokenKind::Yield,
                "empty" => TokenKind::Empty,
                "include" => TokenKind::Include,
                "as" => TokenKind::As,
                "random" => TokenKind::Random,
                "choice" => TokenKind::Choice,
                "from" => TokenKind::From,
                _ => TokenKind::Identifier(interner.intern(&text)),
            }
        };
        Token::new(kind, line, col, text.len())
    }

    fn number(&mut self, first: char, line: usize, col: usize) -> Token {
        let mut num = String::from(first);
        let mut is_float = false;
        while self.peek().is_ascii_digit() {
            num.push(self.advance());
        }
        if self.peek() == '.' && self.peek_next().is_ascii_digit() {
            is_float = true;
            num.push(self.advance());
            while self.peek().is_ascii_digit() {
                num.push(self.advance());
            }
        }
        if is_float {
            Token::new(TokenKind::FloatLiteral(num.parse().unwrap_or(0.0)), line, col, num.len())
        } else {
            Token::new(TokenKind::IntLiteral(num.parse().unwrap_or(0)), line, col, num.len())
        }
    }

    fn string(&mut self, line: usize, col: usize, interner: &mut Interner) -> Token {
        let start_pos_full = self.pos - 1;
        let mut s = String::new();
        while self.peek() != '"' && self.peek() != '\0' {
            let c = self.advance();
            if c == '\\' {
                match self.peek() {
                    'n' => { self.advance(); s.push('\n'); }
                    't' => { self.advance(); s.push('\t'); }
                    'r' => { self.advance(); s.push('\r'); }
                    '0'..='7' => {
                        let mut octal = String::new();
                        for _ in 0..3 {
                            if self.peek().is_ascii_digit() && self.peek() <= '7' {
                                octal.push(self.advance());
                            } else {
                                break;
                            }
                        }
                        if let Ok(val) = u32::from_str_radix(&octal, 8) {
                            if let Some(ch) = std::char::from_u32(val) {
                                s.push(ch);
                            }
                        }
                    }
                    'x' => {
                        self.advance();
                        let mut hex = String::new();
                        for _ in 0..2 {
                            if self.peek().is_ascii_hexdigit() {
                                hex.push(self.advance());
                            } else {
                                break;
                            }
                        }
                        if let Ok(val) = u32::from_str_radix(&hex, 16) {
                            if let Some(ch) = std::char::from_u32(val) {
                                s.push(ch);
                            }
                        }
                    }
                    '"' => { self.advance(); s.push('"'); }
                    '\\' => { self.advance(); s.push('\\'); }
                    _ => { s.push('\\'); }
                }
            } else {
                s.push(c);
            }
        }
        if self.peek() == '"' {
            self.advance();
            Token::new(TokenKind::StringLiteral(interner.intern(&s)), line, col, self.pos - start_pos_full)
        } else {
            Token::new(TokenKind::Unknown('"'), line, col, 1)
        }
    }

    fn raw_block(&mut self, line: usize, col: usize, interner: &mut Interner) -> Token {
        let start_pos_full = self.pos - 3;
        let mut s = String::new();
        while self.peek() != '\0' {
            if self.peek() == '>' && self.peek_at(1) == '>' && self.peek_at(2) == '>' {
                self.advance();
                self.advance();
                self.advance();
                return Token::new(TokenKind::RawBlock(interner.intern(&s)), line, col, self.pos - start_pos_full);
            }
            s.push(self.advance());
        }
        Token::new(TokenKind::Unknown('<'), line, col, 3)
    }
}
