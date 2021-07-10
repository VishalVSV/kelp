use crossterm::event::KeyCode;
use crate::editor::editor::char_width;
use crate::editor::editor::line_ending;
use crate::editor::history::EditDiff;
use unescape::unescape;
use crate::editor::highlight::Token;
use std::path::Path;
use std::io::BufReader;
use std::io::BufRead;
use std::fs::File;
use std::error::Error;
use unicode_width::UnicodeWidthStr;
use std::collections::HashMap;
use std::io::Read;

use std::io::Write;

#[derive(Default)]
pub struct Editor {
    pub docs: Vec<Document>,

    pub config: EditorConfig,

    pub open_doc: Option<usize>,

    width: usize,
    height: usize,

    pub docs_mouse_cache: Vec<(usize,usize)>
}

#[derive(Debug, Copy, Clone)]
pub struct Selection {
    pub start_row: usize,
    pub start_col: usize,
    pub end_row: usize,
    pub end_col: usize
}

pub struct HighlightingInfo {
    pub selection: Option<Selection>
}

#[derive(Default)]
pub struct Document {
    pub filename: String,

    pub cursor_row: usize,
    pub cursor_col: usize,
    pub line_start: usize,

    pub rows: Vec<Row>,
    pub selection: Option<Selection>,

    pub dirty: usize,
    pub show_close: bool,

    pub history: Vec<EditDiff>,
    pub history_index: Option<usize>,

    pub to_auto_close: bool
}

#[derive(Serialize, Deserialize,Debug, Clone)]
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

    #[serde(default)]
    pub auto_close: HashMap<char, char>,
}

#[derive(Default, Serialize, Deserialize,Debug)]
pub struct EditorConfig {
    #[serde(default)]
    pub languages: HashMap<String, FileConfig>,

    #[serde(default)]
    pub theme: Theme,

    #[serde(default)]
    pub keybinds: HashMap<String, KelpKeyEvent>,
}

#[derive(Serialize, Deserialize,Debug)]
pub struct Theme {
    pub background_color: (u8, u8, u8),
    pub foreground_color: (u8, u8, u8)
}

#[allow(dead_code)]
pub struct Row {
    pub buf: String,

    pub tokens: Vec<Token>,

    pub indices: Option<Vec<usize>>, // Allocate this only if there are utf 8 chars in the row. Shamelessly stolen from kiro-editor by rhysd    
}

#[derive(Serialize, Deserialize,Debug)]
pub enum KelpKeyModifiers {
    Alt,
    Control,
    Shift,

    AltAndControl,
    ShiftAndControl,
    AltAndShift,

    NoModifier,
}

#[derive(Serialize, Deserialize,Debug)]
pub struct KelpKeyEvent {
    pub key: KeyCode,
    pub modifiers: KelpKeyModifiers
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
            line_ending: line_ending(),
            syntax_colors, line_comment_start: "//".to_owned(),
            keywords: vec![ ],
            syntax_highlighting_disabled: false,
            multi_line_comment: ("/*".to_owned(),"*/".to_owned()),
            auto_close: HashMap::new()
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            foreground_color: (255, 255, 255),
            background_color: (0, 0, 0)
        }
    }
}

impl Selection {
    pub fn new(start_row: usize, start_col: usize, end_row: usize, end_col: usize) -> Self {
        Self {
            start_col,
            start_row,
            end_row,
            end_col
        }
    }

    pub fn normalize(&mut self) {
        if self.end_row < self.start_row {
            std::mem::swap(&mut self.end_row,&mut self.start_row);
            std::mem::swap(&mut self.end_col,&mut self.start_col);
        }
        else if self.end_row == self.start_row {
            if self.end_col < self.start_col {
                std::mem::swap(&mut self.end_row,&mut self.start_row);
                std::mem::swap(&mut self.end_col,&mut self.start_col);
            }
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
    pub fn char_at(&self, index: usize) -> char {
        self.buf.chars().nth(index).unwrap()
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
    pub fn line_width(&self, file_config: &FileConfig) -> usize {
        if self.indices.is_none() {
            self.buf.len() + (file_config.tab_str.len() - 1) * self.buf.matches('\t').count()
        }
        else {
            self.buf.width() + file_config.tab_str.len() * self.buf.matches('\t').count()
        }
    }

    #[inline]
    pub fn display_buf(&mut self, file_config: &FileConfig, theme: &Theme) -> String {
        let f_color = crossterm::style::SetForegroundColor(crossterm::style::Color::from(theme.foreground_color));
        let b_color = crossterm::style::SetBackgroundColor(crossterm::style::Color::from(theme.background_color));

        if self.tokens.len() == 0 {
            let tmp = self.buf.replace('\t', &file_config.tab_str);
            tmp
        }
        else {

            let mut res = String::new();

            for token in &self.tokens {
                let tmp = format!("{}{}{}{}\x1B[0m",f_color, b_color, token.get_style(file_config),&self.buf[token.get_range().start..token.get_range().end]);
                res.push_str(&tmp);
            }

            res.replace('\t', &file_config.tab_str)
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
    pub fn remove_at(&mut self,idx: usize) -> char {
        let mut loc_idx = if self.indices.is_none() {
            idx
        }
        else {
            self.indices.as_ref().unwrap()[idx]
        };
        while !self.buf.is_char_boundary(loc_idx) {
            loc_idx -= 1;
        }
        let c = self.buf.remove(loc_idx);
        if self.indices.is_some() {
            self.refresh_cache();
        }
        c
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

    pub fn substring(&self, start: usize, end: usize) -> &str {
        if self.indices.is_some() {
            &self.buf[self.indices.as_ref().unwrap()[start]..self.indices.as_ref().unwrap()[end]]
        }
        else {
            &self.buf[start..end]
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
                    format!("{}{}",row.buf,unescape(&config.line_ending).unwrap_or("\n".to_owned()))
                }
                else {
                    format!("{}",row.buf)
                }.as_bytes())?;

            row_index += 1;
        }

        Ok(())
    }

    pub fn add_diff(&mut self, diff: EditDiff) {
        if let Some(history_index) = self.history_index {
            if history_index + 1 < self.history.len() {
                self.history.truncate(history_index + 2);

                self.history[history_index + 1] = diff;
                *self.history_index.as_mut().unwrap() += 1;
            }
            else {
                self.history.push(diff);
                *self.history_index.as_mut().unwrap() = self.history.len() - 1;
            }
        }
        else {
            self.history.clear();
            self.history.push(diff);
            self.history_index = Some(self.history.len() - 1);
        }
    }

    #[inline]
    pub fn display_name(&self) -> String {
        Path::new(&self.filename).file_name().unwrap_or_default().to_str().unwrap_or_default().to_owned()
    }

    #[inline]
    pub fn extension(&self) -> String {
        Path::new(&self.filename).extension().unwrap_or_default().to_str().unwrap_or_default().to_owned()
    }

    pub fn visual_rows_to(&self, width: usize, row_index: usize, file_config: &FileConfig) -> usize {
        let mut rows = 0;

        let mut i = 0;
        for row in &self.rows {
            if row_index == i {
                break;
            }
            rows += row.line_width(file_config) / width + 1;
            i += 1;
        }

        rows
    }

    pub fn tokenize(&mut self, start: usize, end: usize, config: &FileConfig) {
        Token::tokenize(&mut self.rows, HighlightingInfo { selection: self.selection },start, end - start, config);
    }
}

impl Editor {
    pub fn new() -> Self {
        if let Ok(mut plugin_path) = std::env::current_exe() {
            plugin_path.pop();
            plugin_path.push("plugins/");

            // TODO: Plugin loading
        }

        let default_theme = Theme { background_color: (0, 0, 0), foreground_color: (255, 255, 255) };

        let mut default_keybinds = HashMap::new();

        default_keybinds.insert("copy".to_owned(), KelpKeyEvent {
            key: KeyCode::Char('c'),
            modifiers: KelpKeyModifiers::Control
        });
        default_keybinds.insert("paste".to_owned(), KelpKeyEvent {
            key: KeyCode::Char('v'),
            modifiers: KelpKeyModifiers::Control
        });
        default_keybinds.insert("redo".to_owned(), KelpKeyEvent {
            key: KeyCode::Char('y'),
            modifiers: KelpKeyModifiers::Control
        });
        default_keybinds.insert("undo".to_owned(), KelpKeyEvent {
            key: KeyCode::Char('z'),
            modifiers: KelpKeyModifiers::Control
        });
        default_keybinds.insert("start_command".to_owned(), KelpKeyEvent {
            key: KeyCode::Char('g'),
            modifiers: KelpKeyModifiers::Control
        });
        default_keybinds.insert("close_file".to_owned(), KelpKeyEvent {
            key: KeyCode::Char('w'),
            modifiers: KelpKeyModifiers::Control
        });
        default_keybinds.insert("open_file".to_owned(), KelpKeyEvent {
            key: KeyCode::Char('o'),
            modifiers: KelpKeyModifiers::Control
        });
        default_keybinds.insert("new_file".to_owned(), KelpKeyEvent {
            key: KeyCode::Char('n'),
            modifiers: KelpKeyModifiers::Control
        });
        default_keybinds.insert("save_file".to_owned(), KelpKeyEvent {
            key: KeyCode::Char('s'),
            modifiers: KelpKeyModifiers::Control
        });

        let mut config = EditorConfig { languages: HashMap::new(), theme: default_theme, keybinds: default_keybinds };

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
            self.docs_mouse_cache.push((i, i + doc.display_name().width() + 2 /*Side bars*/ + 2 /*Close icon*/ + 
                if doc.dirty == 0 {
                    0
                }
                else {
                    2
                }));

            i += doc.display_name().width() + 2 /*Side bars*/ + 2 /*Close icon*/ + 
            if doc.dirty == 0 {
                0
            }
            else {
                2
            };
        }
    }
}

impl KelpKeyEvent {
    pub fn equals(&self, event: &crossterm::event::KeyEvent) -> bool {
        self.key == event.code && self.modifiers.to_crossterm() == event.modifiers
    }
}

impl KelpKeyModifiers {
    pub fn to_crossterm(&self) -> crossterm::event::KeyModifiers {
        match self {
            KelpKeyModifiers::Alt => crossterm::event::KeyModifiers::ALT,
            KelpKeyModifiers::Control => crossterm::event::KeyModifiers::CONTROL,
            KelpKeyModifiers::Shift => crossterm::event::KeyModifiers::SHIFT,
            KelpKeyModifiers::NoModifier => crossterm::event::KeyModifiers::NONE,

            KelpKeyModifiers::AltAndControl => crossterm::event::KeyModifiers::ALT | crossterm::event::KeyModifiers::CONTROL,
            KelpKeyModifiers::ShiftAndControl => crossterm::event::KeyModifiers::SHIFT | crossterm::event::KeyModifiers::CONTROL,
            KelpKeyModifiers::AltAndShift => crossterm::event::KeyModifiers::ALT | crossterm::event::KeyModifiers::SHIFT,
        }
    }
}
//==========================================================================================

// ==========================================================================================

impl Editor {
    pub fn position_cursor(cursor_row: usize,cursor_col: usize,rows: &Vec<Row>, width: usize, first_row: usize, file_config: &FileConfig) {
        let mut y = 1 + cursor_col / width;
        let mut x = 0;

        if cursor_row < first_row {
            return; // Should never happen...
        }

        for row in rows.iter().skip(first_row).take(cursor_row - first_row) {
            y += row.line_width(file_config) / width + 1;
        }

        let mut i = 0;
        for c in rows[cursor_row].buf.chars().skip((cursor_col / width) * width) {
            if i == cursor_col % width {
                break;
            }

            if let Some(char_width) = char_width(c, file_config) {
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
    pub fn width(&self) -> usize {
        self.width
    }

    #[inline]
    pub fn height(&self) -> usize {
        self.height
    }
}