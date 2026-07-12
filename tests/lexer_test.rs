use tinycerilte::lexer::{Lexer, Token};

/// 全トークンを Vec に収集するヘルパー
fn tokenize(input: &str) -> Vec<Token> {
    let mut lex = Lexer::new(input);
    let mut tokens = Vec::new();
    loop {
        let t = lex.next_token();
        let is_eof = matches!(t, Token::Eof);
        tokens.push(t);
        if is_eof {
            break;
        }
    }
    tokens
}

#[test]
fn empty_input_produces_only_eof() {
    let tokens = tokenize("");
    assert_eq!(tokens, vec![Token::Eof]);
}

#[test]
fn keywords_var_and_bit() {
    let tokens = tokenize("var bit");
    assert_eq!(tokens, vec![Token::Var, Token::Bit, Token::Eof]);
}

#[test]
fn all_delimiters() {
    let tokens = tokenize("{}[];:");
    assert_eq!(
        tokens,
        vec![
            Token::LBrace,
            Token::RBrace,
            Token::LBracket,
            Token::RBracket,
            Token::Semicolon,
            Token::Colon,
            Token::Eof,
        ]
    );
}

#[test]
fn operators_assign_nonblock_caret() {
    let tokens = tokenize("=<=^");
    assert_eq!(
        tokens,
        vec![
            Token::Assign,
            Token::NonBlockAssign,
            Token::Caret,
            Token::Eof,
        ]
    );
}

#[test]
fn identifier_and_number() {
    let tokens = tokenize("a 42");
    assert_eq!(
        tokens,
        vec![Token::Ident("a".into()), Token::Number(42), Token::Eof]
    );
}

#[test]
fn unknown_character_reports_error() {
    let tokens = tokenize("@");
    let has_error = tokens.iter().any(|t| matches!(t, Token::Error(_)));
    assert!(has_error);
}