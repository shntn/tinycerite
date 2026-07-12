use std::fmt;

/// トークン種別
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // キーワード
    Var,
    Bit,

    // デリミタ
    LBrace,    // {
    RBrace,    // }
    LBracket,  // [
    RBracket,  // ]
    LAngle,    // <
    RAngle,    // >
    Semicolon, // ;
    Colon,     // :

    // 演算子
    Assign,         // =
    NonBlockAssign, // <=
    Caret,          // ^

    // リテラル
    Number(u64),

    // 識別子
    Ident(String),

    // 特殊
    Eof,
    Error(String),
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::Var => write!(f, "var"),
            Token::Bit => write!(f, "bit"),
            Token::LBrace => write!(f, "{{"),
            Token::RBrace => write!(f, "}}"),
            Token::LBracket => write!(f, "["),
            Token::RBracket => write!(f, "]"),
            Token::LAngle => write!(f, "<"),
            Token::RAngle => write!(f, ">"),
            Token::Semicolon => write!(f, ";"),
            Token::Colon => write!(f, ":"),
            Token::Assign => write!(f, "="),
            Token::NonBlockAssign => write!(f, "<="),
            Token::Caret => write!(f, "^"),
            Token::Number(n) => write!(f, "{n}"),
            Token::Ident(s) => write!(f, "{s}"),
            Token::Eof => write!(f, "<EOF>"),
            Token::Error(e) => write!(f, "<ERROR: {e}>"),
        }
    }
}

/// 字句解析器
pub struct Lexer {
    chars: Vec<char>,
    pos: usize,
}

impl Lexer {
    pub fn new(input: &str) -> Self {
        Self {
            chars: input.chars().collect(),
            pos: 0,
        }
    }

    /// 次のトークンを1つ読む
    pub fn next_token(&mut self) -> Token {
        self.skip_whitespace();

        if self.pos >= self.chars.len() {
            return Token::Eof;
        }

        let c = self.chars[self.pos];

        // 識別子・キーワード
        if c.is_ascii_alphabetic() || c == '_' {
            return self.read_ident_or_keyword();
        }

        // 数字
        if c.is_ascii_digit() {
            return self.read_number();
        }

        // 記号
        self.pos += 1;
        match c {
            '{' => Token::LBrace,
            '}' => Token::RBrace,
            '[' => Token::LBracket,
            ']' => Token::RBracket,
            ';' => Token::Semicolon,
            ':' => Token::Colon,
            '^' => Token::Caret,
            '=' => Token::Assign,
            '<' => {
                // <= の可能性
                if self.pos < self.chars.len() && self.chars[self.pos] == '=' {
                    self.pos += 1;
                    Token::NonBlockAssign
                } else {
                    Token::LAngle
                }
            }
            '>' => Token::RAngle,
            _ => Token::Error(format!("予期しない文字 '{c}' (位置 {})", self.pos)),
        }
    }

    fn read_ident_or_keyword(&mut self) -> Token {
        let start = self.pos;
        while self.pos < self.chars.len() && (self.chars[self.pos].is_ascii_alphanumeric() || self.chars[self.pos] == '_') {
            self.pos += 1;
        }
        let word: String = self.chars[start..self.pos].iter().collect();
        match word.as_str() {
            "var" => Token::Var,
            "bit" => Token::Bit,
            _ => Token::Ident(word),
        }
    }

    fn read_number(&mut self) -> Token {
        let start = self.pos;
        while self.pos < self.chars.len() && self.chars[self.pos].is_ascii_digit() {
            self.pos += 1;
        }
        let s: String = self.chars[start..self.pos].iter().collect();
        match s.parse::<u64>() {
            Ok(n) => Token::Number(n),
            Err(_) => Token::Error(format!("数値パース失敗: {s}")),
        }
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.chars.len() && self.chars[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }
}

