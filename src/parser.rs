use crate::ast::*;
use crate::lexer::{Lexer, Token};

/// 構文解析エラー
#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "パースエラー: {}", self.message)
    }
}

impl std::error::Error for ParseError {}

type Result<T> = std::result::Result<T, ParseError>;

/// 再帰下降パーサー
pub struct Parser {
    lexer: Lexer,
    current: Token,
}

impl Parser {
    pub fn new(input: &str) -> Self {
        let mut lexer = Lexer::new(input);
        let current = lexer.next_token();
        Self { lexer, current }
    }

    /// Program := Block+
    pub fn parse_program(&mut self) -> Result<Program> {
        let mut blocks = Vec::new();
        while !self.check(&Token::Eof) {
            blocks.push(self.parse_block()?);
        }
        Ok(Program { blocks })
    }

    /// Block := "{" (Decl | Stmt)* "}"
    fn parse_block(&mut self) -> Result<Block> {
        self.expect(&Token::LBrace)?;
        let mut decls = Vec::new();
        let mut stmts = Vec::new();

        while !self.check(&Token::RBrace) && !self.check(&Token::Eof) {
            if self.check(&Token::Var) {
                decls.push(self.parse_decl()?);
            } else {
                stmts.push(self.parse_stmt()?);
            }
        }

        self.expect(&Token::RBrace)?;
        Ok(Block { decls, stmts })
    }

    /// Decl := "var" Ident ":" "bit" ("<" Number ">")? ";"
    fn parse_decl(&mut self) -> Result<Decl> {
        self.expect(&Token::Var)?;
        let name = self.expect_ident()?;
        self.expect(&Token::Colon)?;
        self.expect(&Token::Bit)?;

        let width = if self.check(&Token::LAngle) {
            self.expect(&Token::LAngle)?;
            let n = self.expect_number()?;
            self.expect(&Token::RAngle)?;
            Some(n)
        } else {
            None
        };

        self.expect(&Token::Semicolon)?;
        Ok(Decl { name, width })
    }

    /// Stmt := Ident ("=" | "<=") Expr ";"
    fn parse_stmt(&mut self) -> Result<Stmt> {
        let target = self.expect_ident()?;

        if self.check(&Token::Assign) {
            self.expect(&Token::Assign)?;
            let expr = self.parse_expr()?;
            self.expect(&Token::Semicolon)?;
            Ok(Stmt::Combinational { target, expr })
        } else if self.check(&Token::NonBlockAssign) {
            self.expect(&Token::NonBlockAssign)?;
            let expr = self.parse_expr()?;
            self.expect(&Token::Semicolon)?;
            Ok(Stmt::Sequential { target, expr })
        } else {
            Err(ParseError {
                message: format!("代入演算子 (= または <=) が必要ですが、{} が見つかりました", self.current),
            })
        }
    }

    /// Expr := Ident | Number | Ident "^" Expr | Number "^" Expr | Ident "^" Number
    fn parse_expr(&mut self) -> Result<Expr> {
        let lhs = self.parse_primary()?;

        if self.check(&Token::Caret) {
            self.expect(&Token::Caret)?;
            let rhs = self.parse_primary()?;
            Ok(Expr::BinOp {
                op: BinOp::Xor,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            })
        } else {
            Ok(lhs)
        }
    }

    fn parse_primary(&mut self) -> Result<Expr> {
        if let Token::Ident(name) = self.current.clone() {
            self.advance();
            Ok(Expr::Ident(name))
        } else if let Token::Number(n) = self.current {
            self.advance();
            Ok(Expr::Number(n))
        } else {
            Err(ParseError {
                message: format!("式の開始として識別子か数値が必要ですが、{} が見つかりました", self.current),
            })
        }
    }

    // ---- ヘルパー ----

    fn check(&self, expected: &Token) -> bool {
        std::mem::discriminant(&self.current) == std::mem::discriminant(expected)
            && match (expected, &self.current) {
                // 将来の値比較用のアーム（現在は識別子も数値もdiscriminant比較のみ）
                (Token::Ident(_), Token::Ident(_)) => true,
                (Token::Number(_), Token::Number(_)) => true,
                _ => true,
            }
    }

    fn advance(&mut self) {
        self.current = self.lexer.next_token();
    }

    fn expect(&mut self, expected: &Token) -> Result<()> {
        if std::mem::discriminant(&self.current) == std::mem::discriminant(expected) {
            self.advance();
            Ok(())
        } else {
            Err(ParseError {
                message: format!("{} が必要ですが、{} が見つかりました", expected, self.current),
            })
        }
    }

    fn expect_ident(&mut self) -> Result<String> {
        match self.current.clone() {
            Token::Ident(name) => {
                self.advance();
                Ok(name)
            }
            _ => Err(ParseError {
                message: format!("識別子が必要ですが、{} が見つかりました", self.current),
            }),
        }
    }

    fn expect_number(&mut self) -> Result<u64> {
        match self.current {
            Token::Number(n) => {
                self.advance();
                Ok(n)
            }
            _ => Err(ParseError {
                message: format!("数値が必要ですが、{} が見つかりました", self.current),
            }),
        }
    }
}