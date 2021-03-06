use crate::editor::Row;
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

enum TokenizerAction {
    ParseString(char),
    ParseComment
}

impl Token {
    pub fn tokenize(rows: &mut Vec<Row>,from: usize,num_lines: usize, config: &FileConfig) {
        if config.syntax_highlighting_disabled {
            return;
        }

        let mut parser = None;

        for row in rows.iter_mut().skip(from).take(num_lines) {
            let mut res = Vec::new();

            let src = &row.buf;

            let char_indices: Vec<(usize,char)> = src.char_indices().collect();
    
            let mut odd_token = String::new();
    
            let mut i = 0;
            macro_rules! parse_string {
                ($c: expr) => {
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
                    while i < char_indices.len() && char_indices[i].1 != $c {
                        len += 1;
                        i += 1;
                    }
    
                    if i < char_indices.len() {
                        len += 1;
                        i += 1;
                    }
                    else {
                        parser = Some(TokenizerAction::ParseString($c));
                    }
    
                    res.push(Token::String(i - len..i));
                    
                    i -= 1;
                            
                };
            }

            macro_rules! string_match {
                ($str: expr) => {
                    {
                        let mut offset = 0;
                        let mut res = true;
                        for c in $str.chars() {
                            if i + offset < char_indices.len() {
                                if c != char_indices[i + offset].1 {
                                    res = false;
                                    break;
                                }
                            }
                            else {
                                res = false;
                                break;
                            }
                            offset += 1;
                        }
                        res
                    }
                };
            }

            macro_rules! handle_odd_token {
                () => {
                    if !odd_token.is_empty() {
                        res.push(Token::Plain(i - odd_token.len()..i));
                        odd_token.clear();
                    }
                };
            }
            
            if let Some(p) = &parser {
                match p {
                    TokenizerAction::ParseString(c) => {
                        let mut len = 0;
                        while i < char_indices.len() && char_indices[i].1 != *c {
                            i += 1;
                            len += 1;
                        }
                        if i != char_indices.len() {
                            len += 1;
                            i += 1;
                            res.push(Token::String(i - len..i));
                            parser = None;
                        }
                        else {
                            res.push(Token::String(0..src.len()));
                        }
                    },
                    TokenizerAction::ParseComment => {
                        let mut len = 0;
                        while i < char_indices.len() && !string_match!(config.multi_line_comment.1) {
                            i += 1;
                            len += 1;
                        }
                        if i < char_indices.len() {
                            len += config.multi_line_comment.1.chars().count();
                            i += config.multi_line_comment.1.chars().count();

                            res.push(Token::Comment(i - len..i));
                            parser = None;
                        }
                        else {
                            res.push(Token::Comment(0..src.len()));
                            row.tokens = res;
                            continue;
                        }
                    }
                }
            }
            while i < char_indices.len() {    
                if char_indices[i].1.is_alphabetic() || char_indices[i].1 == '_' {
                    handle_odd_token!();

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
                    handle_odd_token!();

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
                    handle_odd_token!();
                    
                    parse_string!('"');
                }
                else if char_indices[i].1 == '\'' {
                    handle_odd_token!();
                    
                    parse_string!('\'');
                }
                else if char_indices[i].1 == '`' {    
                    handle_odd_token!();
                                    
                    parse_string!('`');
                }
                else if string_match!(config.line_comment_start) {
                    handle_odd_token!();
                    
                    res.push(Token::Comment(i..src.len()));
                    i = src.len();
                }
                else if string_match!(config.multi_line_comment.0) {
                    handle_odd_token!();
                    
                    let mut len = 0;
                    while i < char_indices.len() && !string_match!(config.multi_line_comment.1) {
                        i += 1;
                        len += 1;
                    }

                    if i < char_indices.len() {
                        let line_ender_len = config.multi_line_comment.1.chars().count();
                        i += line_ender_len;
                        len += line_ender_len;

                        res.push(Token::Comment(i - len..i));

                        i -= 1;
                    }
                    else {
                        res.push(Token::Comment(i - len..i));
                        parser = Some(TokenizerAction::ParseComment);
                        break;
                    }
                }
                else {
                    odd_token.push(char_indices[i].1);
                }
    
                i += 1;
            }
            handle_odd_token!();
    
            row.tokens = res;
        }
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