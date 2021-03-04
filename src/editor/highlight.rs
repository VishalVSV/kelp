use crate::editor::FileConfig;
use core::ops::Range;

#[derive(Debug)]
pub enum Token {
    Identifier(Range<usize>),
    Keyword(Range<usize>),
    Comment(Range<usize>),
    String(Range<usize>),
    Plain(Range<usize>),
    FnCall(Range<usize>),
    Macro(Range<usize>),
    Number(Range<usize>)
}

impl Token {
    pub fn tokenize(src: &String, config: &FileConfig) -> Vec<Self> {
        let mut res = Vec::new();

        let char_indices: Vec<(usize,char)> = src.char_indices().collect();

        let mut odd_token = String::new();

        let mut i = 0;
        while i < char_indices.len() {
            if odd_token.ends_with(&config.line_comment_start) {
                if odd_token.len() > config.line_comment_start.len() {
                    res.push(Token::Plain(i - odd_token.len()..i - config.line_comment_start.len()));
                }
                res.push(Token::Comment(i - config.line_comment_start.len()..src.len()));
                odd_token.clear();
                break;
            }

            if char_indices[i].1.is_alphabetic() || char_indices[i].1 == '_' {
                if !odd_token.is_empty() {
                    if odd_token.ends_with(&config.line_comment_start) {
                        if odd_token.len() > config.line_comment_start.len() {
                            res.push(Token::Plain(i - odd_token.len()..i - config.line_comment_start.len()));
                        }
                        res.push(Token::Comment(i - config.line_comment_start.len()..src.len()));
                        odd_token.clear();
                        break;
                    }
                    else {
                        res.push(Token::Plain(i - odd_token.len()..i));
                    }

                    odd_token.clear();
                }

                let mut len = 0;
                while i < char_indices.len() && (char_indices[i].1.is_alphabetic() || char_indices[i].1 == '_') {
                    len += 1;
                    i += 1;
                }

                if i < char_indices.len() && char_indices[i].1 == '(' {
                    res.push(Token::FnCall(i - len..i));
                }
                else if i < char_indices.len() && char_indices[i].1 == '!' {
                    res.push(Token::Macro(i - len..i));
                }
                else if config.keywords.contains(&src[i - len..i].to_owned()) {
                    res.push(Token::Keyword(i - len..i));
                }
                else {
                    res.push(Token::Identifier(i - len..i));
                }

                
                if i != char_indices.len() {
                    i -= 1;
                }
            }
            else if char_indices[i].1.is_numeric() {
                if !odd_token.is_empty() {
                    if odd_token.ends_with(&config.line_comment_start) {
                        if odd_token.len() > config.line_comment_start.len() {
                            res.push(Token::Plain(i - odd_token.len()..i - config.line_comment_start.len()));
                        }
                        res.push(Token::Comment(i - config.line_comment_start.len()..src.len()));
                        odd_token.clear();
                        break;
                    }
                    else {
                        res.push(Token::Plain(i - odd_token.len()..i));
                    }

                    odd_token.clear();
                }

                let mut len = 0;
                while i < char_indices.len() && char_indices[i].1.is_numeric() {
                    len += 1;
                    i += 1;
                }

                res.push(Token::Number(i - len..i));
                
                if i != char_indices.len() {
                    i -= 1;
                }
            }
            else if char_indices[i].1 == '"' {
                if !odd_token.is_empty() {
                    if odd_token.ends_with(&config.line_comment_start) {
                        if odd_token.len() > config.line_comment_start.len() {
                            res.push(Token::Plain(i - odd_token.len()..i - config.line_comment_start.len()));
                        }
                        res.push(Token::Comment(i - config.line_comment_start.len()..src.len()));
                        odd_token.clear();
                        break;
                    }
                    else {
                        res.push(Token::Plain(i - odd_token.len()..i));
                    }

                    odd_token.clear();
                }

                let mut len = 1;
                i += 1;
                while i < char_indices.len() && char_indices[i].1 != '"' {
                    len += 1;
                    i += 1;
                }

                if i < char_indices.len() {
                    len += 1;
                    i += 1;
                }

                res.push(Token::String(i - len..i));
                
                i -= 1;
            }
            else if char_indices[i].1 == '\'' {
                if !odd_token.is_empty() {
                    if odd_token.ends_with(&config.line_comment_start) {
                        if odd_token.len() > config.line_comment_start.len() {
                            res.push(Token::Plain(i - odd_token.len()..i - config.line_comment_start.len()));
                        }
                        res.push(Token::Comment(i - config.line_comment_start.len()..src.len()));
                        odd_token.clear();
                        break;
                    }
                    else {
                        res.push(Token::Plain(i - odd_token.len()..i));
                    }

                    odd_token.clear();
                }

                let mut len = 1;
                i += 1;
                while i < char_indices.len() && char_indices[i].1 != '\'' {
                    len += 1;
                    i += 1;
                }

                if i < char_indices.len() {
                    len += 1;
                    i += 1;
                }

                res.push(Token::String(i - len..i));
                
                i -= 1;
            }
            else if char_indices[i].1 == '`' {
                if !odd_token.is_empty() {
                    if odd_token.ends_with(&config.line_comment_start) {
                        if odd_token.len() > config.line_comment_start.len() {
                            res.push(Token::Plain(i - odd_token.len()..i - config.line_comment_start.len()));
                        }
                        res.push(Token::Comment(i - config.line_comment_start.len()..src.len()));
                        odd_token.clear();
                        break;
                    }
                    else {
                        res.push(Token::Plain(i - odd_token.len()..i));
                    }

                    odd_token.clear();
                }

                let mut len = 1;
                i += 1;
                while i < char_indices.len() && char_indices[i].1 != '`' {
                    len += 1;
                    i += 1;
                }

                if i < char_indices.len() {
                    len += 1;
                    i += 1;
                }

                res.push(Token::String(i - len..i));
                
                i -= 1;
            }
            else {
                odd_token.push(char_indices[i].1);
            }

            i += 1;
        }

        if !odd_token.is_empty() {
            if odd_token.ends_with(&config.line_comment_start) {
                if odd_token.len() > config.line_comment_start.len() {
                    res.push(Token::Plain(i - odd_token.len()..i - config.line_comment_start.len()));
                }
                res.push(Token::Comment(i - config.line_comment_start.len()..src.len()));
            }
            else {
                res.push(Token::Plain(i - odd_token.len()..i));
            }

            odd_token.clear();
        }

        res
    }

    pub fn start(&self) -> usize {
        match self {
            Token::Identifier(r) => r.start,
            Token::String(r) => r.start,
            Token::Plain(r) => r.start,
            Token::Comment(r) => r.start,
            Token::Keyword(r) => r.start,
            Token::FnCall(r) => r.start,
            Token::Macro(r) => r.start,
            Token::Number(r) => r.start,
        }
    }

    pub fn end(&self) -> usize {
        match self {
            Token::Identifier(r) => r.end,
            Token::String(r) => r.end,
            Token::Plain(r) => r.end,
            Token::Comment(r) => r.end,
            Token::Keyword(r) => r.end,
            Token::FnCall(r) => r.end,
            Token::Macro(r) => r.end,
            Token::Number(r) => r.end,
        }
    }
}