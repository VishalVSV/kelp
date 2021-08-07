use crate::editor::editor::debug_file;
use crate::editor::editor::is_debug;
use crate::editor::prelude::*;
use core::ops::Range;
use crossterm::style::Color;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum Token {
    Identifier(Range<usize>),
    Keyword(Range<usize>),
    Comment(Range<usize>),
    String(Range<usize>),
    Plain(Range<usize>),
    FnCall(Range<usize>),
    Macro(Range<usize>),
    Number(Range<usize>),
    Selection(Range<usize>),

    CustomStyle(Range<usize>, String),
}

enum TokenizerAction {
    ParseString(char),
    ParseComment,
}

impl Token {
    pub fn tokenize(
        rows: &mut Vec<Row>,
        mut info: HighlightingInfo,
        from: usize,
        num_lines: usize,
        config: &FileConfig,
    ) {
        let selection = if info.selection.is_none() {
            None
        } else {
            let s = info.selection.as_mut().unwrap();
            if s.start_row > s.end_row {
                std::mem::swap(&mut s.start_row, &mut s.end_row);
                std::mem::swap(&mut s.start_col, &mut s.end_col);
            } else if s.start_row == s.end_row {
                if s.start_col > s.end_col {
                    std::mem::swap(&mut s.start_col, &mut s.end_col);
                }
            }
            Some(s)
        };

        let mut parser = None;

        for (row_index, row) in rows.iter_mut().enumerate().skip(from).take(num_lines) {
            let mut res = Vec::new();

            let src = &row.buf;

            let char_indices: Vec<(usize, char)> = src.char_indices().collect();

            let mut odd_token = String::new();

            let mut i = 0;
            macro_rules! parse_string {
                ($c: expr) => {
                    if !odd_token.is_empty() {
                        if odd_token.ends_with(&config.line_comment_start) {
                            if odd_token.len() > config.line_comment_start.len() {
                                res.push(Token::Plain(
                                    from_char!(i) - odd_token.len()
                                        ..from_char!(i) - config.line_comment_start.len(),
                                ));
                            }
                            res.push(Token::Comment(
                                from_char!(i) - config.line_comment_start.len()..src.len(),
                            ));
                            odd_token.clear();
                            break;
                        } else {
                            res.push(Token::Plain(from_char!(i) - odd_token.len()..from_char!(i)));
                        }

                        odd_token.clear();
                    }

                    let mut len = 1;
                    let mut is_escaped = false;
                    i += 1;
                    while i < char_indices.len() && (char_indices[i].1 != $c || is_escaped) {
                        if i < char_indices.len() && char_indices[i].1 == '\\' {
                            if i + 2 <= char_indices.len() {
                                res.push(Token::CustomStyle(
                                    from_char!(i)..from_char!(i + 2),
                                    format!(
                                        "{}{}",
                                        Token::String(0..0).get_style(config),
                                        crossterm::style::Attribute::Dim
                                    ),
                                ));
                            }
                            is_escaped = true;
                        } else {
                            is_escaped = false;
                        }

                        len += 1;
                        i += 1;
                    }

                    if i < char_indices.len() {
                        len += 1;
                        i += 1;
                    } else {
                        parser = Some(TokenizerAction::ParseString($c));
                    }

                    res.push(Token::String(from_char!(i - len)..from_char!(i)));

                    i -= 1;
                };
            }

            macro_rules! string_match {
                ($str: expr) => {{
                    let mut offset = 0;
                    let mut res = true;
                    for c in $str.chars() {
                        if i + offset < char_indices.len() {
                            if c != char_indices[i + offset].1 {
                                res = false;
                                break;
                            }
                        } else {
                            res = false;
                            break;
                        }
                        offset += 1;
                    }
                    res
                }};
            }

            macro_rules! handle_odd_token {
                () => {
                    if !odd_token.is_empty() {
                        let li = if i >= char_indices.len() {
                            src.len()
                        } else {
                            char_indices[i].0
                        };

                        res.push(Token::Plain(li - odd_token.len()..li));
                        odd_token.clear();
                    }
                };
            }

            macro_rules! is_inside_sel {
                () => {{
                    if let Some(selection) = &selection {
                        row_index >= selection.start_row
                            && row_index <= selection.end_row
                            && if selection.start_row != selection.end_row {
                                if row_index == selection.start_row {
                                    i >= selection.start_col
                                } else if row_index == selection.end_row {
                                    i < selection.end_col
                                } else {
                                    true
                                }
                            } else {
                                i >= selection.start_col && i < selection.end_col
                            }
                    } else {
                        false
                    }
                }};
            }

            macro_rules! from_char {
                ($i: expr) => {
                    if $i < char_indices.len() {
                        char_indices[$i].0
                    } else if $i == char_indices.len() {
                        src.len()
                    } else {
                        panic!("{} out of bounds...", $i)
                    }
                };
            }

            if let Some(selection) = &selection {
                if row_index >= selection.start_row && row_index <= selection.end_row {
                    if selection.start_row != selection.end_row {
                        if row_index == selection.start_row {
                            res.push(Token::Selection(from_char!(selection.start_col)..src.len()));
                        } else if row_index == selection.end_row {
                            res.push(Token::Selection(0..from_char!(selection.end_col)));
                        } else {
                            res.push(Token::Selection(0..src.len()));
                        }
                    } else {
                        res.push(Token::Selection(
                            from_char!(selection.start_col)..from_char!(selection.end_col),
                        ));
                    }
                }
            }

            if config.syntax_highlighting_disabled {
                res.push(Token::Plain(0..src.len()));
                Token::normalize(&mut res, src.len(), config, src.to_string());
                row.tokens = res;
                continue;
            }

            if let Some(p) = &parser {
                match p {
                    TokenizerAction::ParseString(c) => {
                        let mut len = 0;
                        let mut is_escaped = false;
                        while i < char_indices.len() && (char_indices[i].1 != *c || is_escaped) {
                            if i < char_indices.len() && char_indices[i].1 == '\\' {
                                if i + 2 <= char_indices.len() {
                                    res.push(Token::CustomStyle(
                                        from_char!(i)..from_char!(i + 2),
                                        format!(
                                            "{}{}",
                                            Token::String(0..0).get_style(config),
                                            crossterm::style::Attribute::Dim
                                        ),
                                    ));
                                }
                                is_escaped = true;
                            } else {
                                is_escaped = false;
                            }

                            i += 1;
                            len += 1;
                        }
                        if i != char_indices.len() {
                            len += 1;
                            i += 1;
                            res.push(Token::String(from_char!(i - len)..from_char!(i)));
                            parser = None;
                        } else {
                            res.push(Token::String(0..src.len()));
                        }
                    }
                    TokenizerAction::ParseComment => {
                        let mut len = 0;
                        while i < char_indices.len() && !string_match!(config.multi_line_comment.1)
                        {
                            i += 1;
                            len += 1;
                        }
                        if i < char_indices.len() {
                            len += config.multi_line_comment.1.chars().count();
                            i += config.multi_line_comment.1.chars().count();

                            res.push(Token::Comment(from_char!(i - len)..from_char!(i)));
                            parser = None;
                        } else {
                            res.push(Token::Comment(0..src.len()));
                        }
                    }
                }
            }

            while i < char_indices.len() {
                if char_indices[i].1.is_alphabetic() || char_indices[i].1 == '_' {
                    handle_odd_token!();

                    let mut len = 0;
                    while i < char_indices.len()
                        && (char_indices[i].1.is_alphanumeric() || char_indices[i].1 == '_')
                    {
                        len += 1;
                        i += 1;
                    }

                    if len != 0 {
                        if i < char_indices.len() && char_indices[i].1 == '(' {
                            res.push(Token::FnCall(from_char!(i - len)..from_char!(i)));
                        } else if i < char_indices.len() && char_indices[i].1 == '!' {
                            res.push(Token::Macro(from_char!(i - len)..from_char!(i)));
                        } else if config
                            .keywords
                            .contains(&src[from_char!(i - len)..from_char!(i)].to_owned())
                        {
                            res.push(Token::Keyword(from_char!(i - len)..from_char!(i)));
                        } else {
                            res.push(Token::Identifier(from_char!(i - len)..from_char!(i)));
                        }

                        if i != char_indices.len() && !is_inside_sel!() {
                            i -= 1;
                        }
                    }

                    // else if i != 0 {
                    //     panic!("{} at {} {:?} {}",char_indices[i].1, i, res, status);
                    // }
                } else if char_indices[i].1.is_numeric() {
                    handle_odd_token!();

                    let mut len = 0;
                    while i < char_indices.len() && char_indices[i].1.is_numeric() {
                        len += 1;
                        i += 1;
                    }

                    res.push(Token::Number(from_char!(i - len)..from_char!(i)));

                    if i != char_indices.len() {
                        i -= 1;
                    }
                } else if string_match!(config.line_comment_start) {
                    handle_odd_token!();

                    res.push(Token::Comment(from_char!(i)..src.len()));
                    i = src.len();
                } else if string_match!(config.multi_line_comment.0) {
                    handle_odd_token!();
                    let mut len = config.multi_line_comment.0.len();
                    i += config.multi_line_comment.0.len();
                    while i < char_indices.len() && !string_match!(config.multi_line_comment.1) {
                        i += 1;
                        len += 1;
                    }

                    if i < char_indices.len() {
                        let line_ender_len = config.multi_line_comment.1.chars().count();
                        i += line_ender_len;
                        len += line_ender_len;

                        res.push(Token::Comment(from_char!(i - len)..from_char!(i)));

                        if i < char_indices.len() {
                            i -= 1;
                        }
                    } else {
                        res.push(Token::Comment(from_char!(i - len)..from_char!(i)));
                        parser = Some(TokenizerAction::ParseComment);
                    }
                } else if char_indices[i].1 == '"' {
                    handle_odd_token!();

                    parse_string!('"');
                } else if char_indices[i].1 == '\'' {
                    handle_odd_token!();

                    parse_string!('\'');
                } else if char_indices[i].1 == '`' {
                    handle_odd_token!();

                    parse_string!('`');
                } else {
                    odd_token.push(char_indices[i].1);
                }

                i += 1;
            }
            handle_odd_token!();

            Token::normalize(&mut res, src.len(), config, src.to_string());
            row.tokens = res;
        }
    }

    pub fn normalize(tokens: &mut Vec<Token>, len: usize, _config: &FileConfig, src: String) {
        if tokens.len() == 0 || len == 0 {
            return;
        }

        let mut normalizing_line = vec![-1; len];

        for (token_id, token) in tokens.iter().enumerate() {
            for i in token.start()..token.end() {
                if i < normalizing_line.len() {
                    if normalizing_line[i] != -1 {
                        if tokens[normalizing_line[i] as usize].priority() < token.priority() {
                            normalizing_line[i] = token_id as i32;
                        }
                    } else {
                        normalizing_line[i] = token_id as i32;
                    }
                }
            }
        }

        let mut res = Vec::with_capacity(tokens.len());

        let mut start = 0;
        let mut i_outer = 0;
        let mut token_id_outer = 0;
        let mut current_token = -1;

        for (i, token_id) in normalizing_line.iter().enumerate() {
            i_outer = i;
            token_id_outer = *token_id;

            if current_token == -1 {
                current_token = *token_id;
                start = i;
            } else {
                if *token_id != current_token {
                    res.push(tokens[current_token as usize].clone());
                    let l = res.len();
                    *res[l - 1].get_range_mut() = start..i;

                    start = i;

                    current_token = *token_id;
                }
            }
        }

        if token_id_outer != -1 {
            res.push(tokens[token_id_outer as usize].clone());
            let l = res.len();
            *res[l - 1].get_range_mut() = start..i_outer + 1;
        } else {
            panic!("{:?} {:?} {:?}", tokens, res, normalizing_line);
        }

        if is_debug() {
            let mut d = String::new();

            for t in &res {
                d.push_str(&format!(
                    "{}|",
                    &src[t.get_range().start..t.get_range().end]
                ));
            }

            std::fs::write(
                debug_file(),
                format!("{}\n{:?}\n{:?}", d, res, tokens).as_bytes(),
            )
            .unwrap();
        }

        *tokens = res;
    }

    pub fn priority(&self) -> usize {
        match self {
            Token::Identifier(_) => 1,
            Token::String(_) => 5,
            Token::Plain(_) => 0,
            Token::Comment(_) => 6,
            Token::Keyword(_) => 2,
            Token::FnCall(_) => 3,
            Token::Macro(_) => 3,
            Token::Number(_) => 1,
            Token::Selection(_) => 10,
            Token::CustomStyle(_, _) => 10,
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
            Token::Selection(r) => r.start,
            Token::CustomStyle(r, _) => r.start,
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
            Token::Selection(r) => r.end,
            Token::CustomStyle(r, _) => r.end,
        }
    }

    pub fn get_range_mut(&mut self) -> &mut Range<usize> {
        match self {
            Token::Identifier(r) => r,
            Token::String(r) => r,
            Token::Plain(r) => r,
            Token::Comment(r) => r,
            Token::Keyword(r) => r,
            Token::FnCall(r) => r,
            Token::Macro(r) => r,
            Token::Number(r) => r,
            Token::Selection(r) => r,
            Token::CustomStyle(r, _) => r,
        }
    }

    pub fn get_style(&self, config: &FileConfig) -> String {
        let ident = config
            .syntax_colors
            .get(&"identifier".to_owned())
            .unwrap_or(&(255, 255, 255));
        let keyword = config
            .syntax_colors
            .get(&"keyword".to_owned())
            .unwrap_or(&(255, 255, 255));
        let string = config
            .syntax_colors
            .get(&"string".to_owned())
            .unwrap_or(&(255, 255, 255));
        let comment = config
            .syntax_colors
            .get(&"comment".to_owned())
            .unwrap_or(&(255, 255, 255));
        let fncall = config
            .syntax_colors
            .get(&"fncall".to_owned())
            .unwrap_or(&(255, 255, 255));
        let macro_ = config
            .syntax_colors
            .get(&"macro".to_owned())
            .unwrap_or(&(255, 255, 255));
        let number = config
            .syntax_colors
            .get(&"number".to_owned())
            .unwrap_or(&(255, 255, 255));
        let selection = config
            .syntax_colors
            .get(&"selection".to_owned())
            .unwrap_or(&(0, 0, 255));

        match self {
            Token::Identifier(_) => {
                format!(
                    "{}",
                    crossterm::style::SetForegroundColor(Color::from(*ident))
                )
            }
            Token::Keyword(_) => {
                format!(
                    "{}",
                    crossterm::style::SetForegroundColor(Color::from(*keyword))
                )
            }
            Token::String(_) => {
                format!(
                    "{}",
                    crossterm::style::SetForegroundColor(Color::from(*string))
                )
            }
            Token::Plain(_) => String::new(),
            Token::Comment(_) => {
                format!(
                    "{}{}",
                    crossterm::style::Attribute::Italic,
                    crossterm::style::SetForegroundColor(Color::from(*comment))
                )
            }
            Token::FnCall(_) => {
                format!(
                    "{}",
                    crossterm::style::SetForegroundColor(Color::from(*fncall))
                )
            }
            Token::Macro(_) => {
                format!(
                    "{}",
                    crossterm::style::SetForegroundColor(Color::from(*macro_))
                )
            }
            Token::Number(_) => {
                format!(
                    "{}",
                    crossterm::style::SetForegroundColor(Color::from(*number))
                )
            }
            Token::Selection(_) => {
                format!(
                    "{}",
                    crossterm::style::SetBackgroundColor(Color::from(*selection))
                )
            }
            Token::CustomStyle(_, s) => s.clone(),
        }
    }

    pub fn get_range(&self) -> &Range<usize> {
        match self {
            Token::Identifier(r) => r,
            Token::String(r) => r,
            Token::Plain(r) => r,
            Token::Comment(r) => r,
            Token::Keyword(r) => r,
            Token::FnCall(r) => r,
            Token::Macro(r) => r,
            Token::Number(r) => r,
            Token::Selection(r) => r,
            Token::CustomStyle(r, _) => r,
        }
    }
}
