/*
TODO:
1. Reorganize the mod file. Project is way bigger than I thought and I need to move components to different files.
2. Collapse whitespace into a single Plain enum variant. - Done
3. Add per file type config - Done
4. Configurable syntax highlighting - In progress
5. Rebindable keys

*/

mod highlight;
mod editor;

use unescape::unescape;
use std::borrow::Cow;
use crate::editor::highlight::Token;
use std::path::Path;
use std::io::BufReader;
use std::io::BufRead;
use std::fs::File;
use std::error::Error;
use unicode_width::UnicodeWidthChar;
use unicode_width::UnicodeWidthStr;
use std::collections::HashMap;
use crossterm::style::Color;
use std::io::Read;

use std::io::Write;

pub fn kelp_version() -> String {
    {
        #[cfg(debug_assertions)]
        format!("{} - Debug",option_env!("CARGO_PKG_VERSION").unwrap_or("Unknown"))
    }
    #[cfg(not(debug_assertions))]
    format!("{}",option_env!("CARGO_PKG_VERSION").unwrap_or("Unknown"))
}

#[derive(Default)]
pub struct Editor {
    pub docs: Vec<Document>,

    pub config: EditorConfig,

    pub open_doc: Option<usize>,

    width: usize,
    height: usize,

    docs_mouse_cache: Vec<(usize,usize)>
}

#[derive(Default)]
pub struct Document {
    pub filename: String,

    pub cursor_row: usize,
    pub cursor_col: usize,
    pub line_start: usize,

    pub rows: Vec<Row>
}

#[derive(Serialize, Deserialize,Debug)]
pub struct FileConfig {
    #[serde(default)]
    pub tab_str: String,
    #[serde(default)]
    pub line_ending: String,

    #[serde(default)]
    pub line_comment_start: String,
    #[serde(default)]
    pub multi_line_comment: (String,String),

    #[serde(default)]
    pub keywords: Vec<String>,

    #[serde(default)]
    pub syntax_colors: HashMap<String, (u8,u8,u8)>,

    #[serde(default)]
    pub syntax_highlighting_disabled: bool,
}

#[derive(Default, Serialize, Deserialize,Debug)]
pub struct EditorConfig {
    pub languages: HashMap<String, FileConfig>
}

#[allow(dead_code)]
pub struct Row {
    buf: String,

    pub tokens: Vec<Token>,

    indices: Option<Vec<usize>>, // Allocate this only if there are utf 8 chars in the row. Shamelessly stolen from kiro-editor by rhysd    
}
//==========================================================================================

// =========================================================================================
impl Default for FileConfig {
    fn default() -> Self {
        let mut syntax_colors = HashMap::new();
        syntax_colors.insert("identifier".to_owned(), (128,128,128));
        syntax_colors.insert("keyword".to_owned(), (0,148,255));
        syntax_colors.insert("comment".to_owned(), (0,127,14));
        syntax_colors.insert("string".to_owned(), (255,240,24));

        Self {
            tab_str: String::from("    "),
            line_ending: String::from("\\r\\n"),
            syntax_colors, line_comment_start: "//".to_owned(),
            keywords: vec![ ],
            syntax_highlighting_disabled: false,
            multi_line_comment: ("/*".to_owned(),"*/".to_owned())
        }
    }
}


impl Row {
    pub fn empty() -> Self {
        Self {
            buf: String::new(),
            indices: None,
            tokens: Vec::new()
        }
    }

    pub fn from_string(line: String) -> Self {
        let indices;

        if line.is_ascii() {
            indices = None;
        }
        else {
            indices = Some(line.char_indices().map(|index| index.0).collect());
        }

        Self {
            buf: line,
            indices,
            tokens: Vec::new()
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        if self.indices.is_none() {
            self.buf.len()
        }
        else {
            self.indices.as_ref().unwrap().len()
        }
    }

    #[inline]
    pub fn line_width(&self) -> usize {
        if self.indices.is_none() {
            self.buf.len()
        }
        else {
            self.buf.width()
        }
    }

    #[inline]
    pub fn display_buf(&mut self, config: &FileConfig) -> Cow<String> {
        if self.tokens.len() == 0 {
            Cow::Borrowed(&self.buf)
        }
        else {

            let mut res = String::new();

            let ident = config.syntax_colors.get(&"identifier".to_owned()).unwrap_or(&(255,255,255));
            let keyword = config.syntax_colors.get(&"keyword".to_owned()).unwrap_or(&(255,255,255));
            let string = config.syntax_colors.get(&"string".to_owned()).unwrap_or(&(255,255,255));
            let comment = config.syntax_colors.get(&"comment".to_owned()).unwrap_or(&(255,255,255));
            let fncall = config.syntax_colors.get(&"fncall".to_owned()).unwrap_or(&(255,255,255));
            let macro_ = config.syntax_colors.get(&"macro".to_owned()).unwrap_or(&(255,255,255));
            let number = config.syntax_colors.get(&"number".to_owned()).unwrap_or(&(255,255,255));

            for token in &self.tokens {
                match token {
                    Token::Identifier(range) => {
                        let tmp = format!("{}{}\x1B[0m",crossterm::style::SetForegroundColor(Color::from(*ident)),&self.buf[range.start..range.end]);
                        res.push_str(&tmp);
                    },
                    Token::Keyword(range) => {
                        let tmp = format!("{}{}\x1B[0m",crossterm::style::SetForegroundColor(Color::from(*keyword)),&self.buf[range.start..range.end]);
                        res.push_str(&tmp);
                    },
                    Token::String(range) => {
                        let tmp = format!("{}{}\x1B[0m",crossterm::style::SetForegroundColor(Color::from(*string)),&self.buf[range.start..range.end]);
                        res.push_str(&tmp);
                    },
                    Token::Plain(range) => {
                        res.push_str(&self.buf[range.start..range.end]);
                    },
                    Token::Comment(range) => {
                        let tmp = format!("{}{}\x1B[0m",crossterm::style::SetForegroundColor(Color::from(*comment)),&self.buf[range.start..range.end]);
                        res.push_str(&tmp);
                    },
                    Token::FnCall(range) => {
                        let tmp = format!("{}{}\x1B[0m",crossterm::style::SetForegroundColor(Color::from(*fncall)),&self.buf[range.start..range.end]);
                        res.push_str(&tmp);
                    },
                    Token::Macro(range) => {
                        let tmp = format!("{}{}\x1B[0m",crossterm::style::SetForegroundColor(Color::from(*macro_)),&self.buf[range.start..range.end]);
                        res.push_str(&tmp);
                    },
                    Token::Number(range) => {
                        let tmp = format!("{}{}\x1B[0m",crossterm::style::SetForegroundColor(Color::from(*number)),&self.buf[range.start..range.end]);
                        res.push_str(&tmp);
                    },
                }
            }

            Cow::Owned(res)
        }
    }

    #[inline]
    pub fn insert_char(&mut self,idx: usize,chr: char) {
        if chr.is_ascii() {
            if self.indices.is_none() {
                if !self.buf.is_char_boundary(idx) {
                    panic!("{} {}",idx,self.buf.len());
                }
                self.buf.insert(idx, chr);
            }
            else {
                
                if idx < self.indices.as_ref().unwrap().len() {
                    self.buf.insert(self.indices.as_ref().unwrap()[idx], chr);
                }
                else if idx == self.indices.as_ref().unwrap().len() {
                    self.buf.push(chr);
                }
                self.refresh_cache();
            }
        }
        else {
            if self.indices.is_none() {
                self.buf.insert(idx, chr);
                self.refresh_cache();
            }
            else {
                if idx < self.indices.as_ref().unwrap().len() {
                    self.buf.insert(self.indices.as_ref().unwrap()[idx], chr);
                }
                else if idx == self.indices.as_ref().unwrap().len() {
                    self.buf.push(chr);
                }
                self.refresh_cache();
            }
        }
    }

    #[inline]
    pub fn remove_at(&mut self,idx: usize) {
        let mut loc_idx = if self.indices.is_none() {
            idx
        }
        else {
            self.indices.as_ref().unwrap()[idx]
        };
        while !self.buf.is_char_boundary(loc_idx) {
            loc_idx -= 1;
        }
        self.buf.remove(loc_idx);
        if self.indices.is_some() {
            self.refresh_cache();
        }
    }

    #[inline]
    pub fn split_at(&mut self,idx: usize) -> (String,String) {
        if self.indices.is_some() {
            let (left,right) = self.buf.split_at(self.indices.as_ref().unwrap()[idx]);
            (left.to_owned(),right.to_owned())
        }
        else {
            let (left,right) = self.buf.split_at(idx);
            (left.to_owned(),right.to_owned())
        }
    }

    #[inline]
    fn refresh_cache(&mut self) {
        self.indices = Some(self.buf.char_indices().map(|index| index.0).collect());
    }
}

impl Document {
    pub fn new(filename: String) -> Self {
        Self {
            filename,
            rows: Vec::new(),
            ..Document::default()
        }
    }

    pub fn load(filename: String) -> Result<Self, String> {
        let file = 
            if let Ok(file) = File::open(&filename) {
                file
            }
            else {
                return Err(filename);
            };
        let reader = BufReader::new(file);

        let mut rows = Vec::new();

        for line in reader.lines() {
            if let Ok(mut line) = line {
                if line.len() >= 3 && line.as_bytes()[0] == 0xEF && line.as_bytes()[1] == 0xBB && line.as_bytes()[2] == 0xBF {
                    line = String::from_utf8(line.as_bytes().iter().skip(3).map(|b| *b).collect()).unwrap();
                }
                rows.push(Row::from_string(line));
            }
            else {
                
            }
        }

        Ok(
            Self {
                rows,
                filename,
                ..Document::default()
            }
        )
    }

    pub fn save(&self, config: &FileConfig) -> Result<(),Box<dyn Error>> {
        let mut file = File::create(&self.filename)?;
        
        let mut row_index = 0;
        for row in &self.rows {
            file.write(
                if row_index + 1 != self.rows.len() {
                    format!("{}{}",row.buf,unescape(&config.line_ending).unwrap_or("\r\n".to_owned()))
                }
                else {
                    format!("{}",row.buf)
                }.as_bytes())?;

            row_index += 1;
        }

        Ok(())
    }

    #[inline]
    pub fn display_name(&self) -> String {
        Path::new(&self.filename).file_name().unwrap_or_default().to_str().unwrap_or_default().to_owned()
    }

    #[inline]
    pub fn extension(&self) -> String {
        Path::new(&self.filename).extension().unwrap_or_default().to_str().unwrap_or_default().to_owned()
    }

    pub fn visual_rows_to(&self, width: usize, row_index: usize) -> usize {
        let mut rows = 0;

        let mut i = 0;
        for row in &self.rows {
            if row_index == i {
                break;
            }
            rows += row.line_width() / width + 1;
            i += 1;
        }

        rows
    }

    pub fn tokenize(&mut self, start: usize, end: usize, config: &FileConfig) {
        Token::tokenize(&mut self.rows,start, end - start, config);
    }
}

impl Editor {
    pub fn new() -> Self {
        let mut config = EditorConfig { languages: HashMap::new() };

        config.languages.insert("*".to_owned() , FileConfig::default());

        let mut path = std::env::current_exe().unwrap_or_default();
        path.pop();
        path.push("config.json");
        let config_file = File::open(path.clone());

        if let Ok(mut config_file) = config_file {
            let mut config_file_contents = String::new();
            if let Ok(_) = config_file.read_to_string(&mut config_file_contents) {
                if let Ok(new_config) = serde_json::from_str(&config_file_contents) {
                    config = new_config;
                }
            }
        }
        else {
            match File::create(path.clone()) {
               Ok(mut config_file) => {
                   let _ = config_file.write_all(&serde_json::to_string_pretty(&config).unwrap_or_default().as_bytes());
                },
                _ => {}
            }
        }

        Self {
            docs: Vec::new(),
            open_doc: None,
            width: crossterm::terminal::size().unwrap_or((100,100)).0 as usize,
            height: crossterm::terminal::size().unwrap_or((100,100)).1 as usize,
            config,
            ..Editor::default()
        }
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        self.width = width;
        self.height = height;
    }

    pub fn add_doc(&mut self, doc: Document) {
        self.docs.push(doc);
        self.refresh_mouse_cache();
    }

    pub fn refresh_mouse_cache(&mut self) {
        self.docs_mouse_cache.clear();

        let mut i = 0;
        for doc in &self.docs {
            self.docs_mouse_cache.push((i, i + doc.display_name().width() + 2));

            i += doc.display_name().width() + 3;
        }
    }
}
//==========================================================================================

// ==========================================================================================

impl Editor {
    fn position_cursor(cursor_row: usize,cursor_col: usize,rows: &Vec<Row>, width: usize, first_row: usize) {
        let mut y = 1 + cursor_col / width;
        let mut x = 0;

        if cursor_row < first_row {
            return; // Should never happen...
        }

        for row in rows.iter().skip(first_row).take(cursor_row - first_row) {
            y += row.line_width() / width + 1;
        }

        let mut i = 0;
        for c in rows[cursor_row].buf.chars().skip((cursor_col / width) * width) {
            if i == cursor_col % width {
                break;
            }

            if let Some(char_width) = c.width() {
                x += char_width;
            }
            i += 1;
        }

        if cursor_col % width == 0 && cursor_col != 0 {
            x = width;
            y -= 1;
        }

        print!("{}{}",crossterm::cursor::Show,crossterm::cursor::MoveTo(x as u16, y as u16));
        std::io::stdout().flush().unwrap();
    }

    #[inline]
    fn width(&self) -> usize {
        self.width
    }

    #[inline]
    fn height(&self) -> usize {
        self.height
    }
}