use numlang::token::{tokenize, Token};

#[test]
fn test_tokenize_math_operators() {
    let input = "+ - * / % ^ = == != < <= > >= !";
    let tokens = tokenize(input).expect("Tokenization failed");
    let token_kinds: Vec<Token> = tokens.into_iter().map(|st| st.token).collect();

    assert_eq!(
        token_kinds,
        vec![
            Token::Plus,
            Token::Minus,
            Token::Star,
            Token::Slash,
            Token::Percent,
            Token::Caret,
            Token::Assign,
            Token::Eq,
            Token::Ne,
            Token::Lt,
            Token::Le,
            Token::Gt,
            Token::Ge,
            Token::Not,
        ]
    );
}

#[test]
fn test_tokenize_bitwise_operators() {
    let input = "& | ^ << >> **";
    let tokens = tokenize(input).expect("Tokenization failed");
    let token_kinds: Vec<Token> = tokens.into_iter().map(|st| st.token).collect();

    assert_eq!(
        token_kinds,
        vec![
            Token::Ampersand,
            Token::Pipe,
            Token::Caret,
            Token::Shl,
            Token::Shr,
            Token::StarStar,
        ]
    );
}

#[test]
fn test_tokenize_keywords() {
    let input = "fn let return if else while for true false";
    let tokens = tokenize(input).expect("Tokenization failed");
    let token_kinds: Vec<Token> = tokens.into_iter().map(|st| st.token).collect();

    assert_eq!(
        token_kinds,
        vec![
            Token::Fn,
            Token::Let,
            Token::Return,
            Token::If,
            Token::Else,
            Token::While,
            Token::For,
            Token::True,
            Token::False,
        ]
    );
}

#[test]
fn test_tokenize_literals_and_identifiers() {
    let input = "let alpha = 42; let beta = 3.14159;";
    let tokens = tokenize(input).expect("Tokenization failed");

    assert_eq!(tokens[0].token, Token::Let);
    assert_eq!(tokens[1].token, Token::Ident("alpha".to_string()));
    assert_eq!(tokens[2].token, Token::Assign);
    assert_eq!(tokens[3].token, Token::IntLiteral(42));
    assert_eq!(tokens[4].token, Token::Semi);

    assert_eq!(tokens[5].token, Token::Let);
    assert_eq!(tokens[6].token, Token::Ident("beta".to_string()));
    assert_eq!(tokens[7].token, Token::Assign);
    assert_eq!(tokens[8].token, Token::FloatLiteral(3.14159));
    assert_eq!(tokens[9].token, Token::Semi);
}

#[test]
fn test_spans() {
    let input = "fn add(x: f64)";
    let tokens = tokenize(input).expect("Tokenization failed");

    // 'fn' is 0..2
    assert_eq!(tokens[0].span.start, 0);
    assert_eq!(tokens[0].span.end, 2);

    // 'add' is 3..6
    assert_eq!(tokens[1].span.start, 3);
    assert_eq!(tokens[1].span.end, 6);
}
