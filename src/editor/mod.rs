use std::path::Path;
use std::io::BufReader;
use std::io::BufRead;
use std::fs::File;
use crossterm::event::{MouseEventKind,MouseButton};
use crossterm::event::Event::Resize;
use crossterm::event::{Event::{Key,Mouse},KeyModifiers,KeyCode};
use crossterm::event::read;
use std::error::Error;
use unicode_width::UnicodeWidthChar;
use unicode_width::UnicodeWidthStr;

use crossterm::ExecutableCommand;

use std::io::Write;

pub fn kelp_version() -> String {
    {
        #[cfg(debug_assertions)]
        format!("{} - Debug",option_env!("CARGO_PKG_VERSION").unwrap_or("Unknown"))
    }
    #[cfg(not(debug_assertions))]
    format!("{}",option_env!("CARGO_PKG_VERSION").unwrap_or("Unknown"))
}

// ================================= TYPE DECL =============================================
#[derive(Default)]
pub struct Editor {
    pub docs: Vec<Document>,

    pub config: EditorConfig,

    pub open_doc: Option<usize>,

    width: usize,
    height: usize,

    line_start: usize,
    docs_mouse_cache: Vec<(usize,usize)>
}

#[derive(Default)]
pub struct Document {
    pub filename: String,

    pub cursor_row: usize,
    pub cursor_col: usize,

    pub rows: Vec<Row>
}

#[derive(Default)]
pub struct EditorConfig {
    pub tab_str: String,
    pub line_ending: String
}

#[allow(dead_code)]
pub struct Row {
    buf: String,

    indices: Option<Vec<usize>>, // Allocate this only if there are utf 8 chars in the row. Shamelessly stolen from kiro-editor by rhysd    
}
//==========================================================================================


// =========================================================================================
impl Row {
    pub fn empty() -> Self {
        Self {
            buf: String::new(),
            indices: None
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
            indices
        }
    }

    pub fn len(&self) -> usize {
        if self.indices.is_none() {
            self.buf.len()
        }
        else {
            self.indices.as_ref().unwrap().len()
        }
    }

    pub fn line_width(&self) -> usize {
        if self.indices.is_none() {
            self.buf.len()
        }
        else {
            self.buf.width()
        }
    }

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
                // self.buf.insert(self.indices.as_ref().unwrap()[idx], chr);
                // self.refresh_cache();
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

    pub fn save(&self, config: &EditorConfig) -> Result<(),Box<dyn Error>> {
        let mut file = File::create(&self.filename)?;
        
        for row in &self.rows {
            file.write(format!("{}{}",row.buf,config.line_ending).as_bytes())?;
        }

        Ok(())
    }

    #[inline]
    pub fn display_name(&self) -> String {
        Path::new(&self.filename).file_name().unwrap_or_default().to_str().unwrap_or_default().to_owned()
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
}

impl Editor {
    pub fn new() -> Self {
        Self {
            docs: Vec::new(),
            open_doc: None,
            width: crossterm::terminal::size().unwrap_or((100,100)).0 as usize,
            height: crossterm::terminal::size().unwrap_or((100,100)).1 as usize,
            config: EditorConfig { tab_str: String::from("    "),line_ending: String::from("\r\n") },
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
    pub fn start(mut self) -> Result<(), Box<dyn Error>> {
        std::io::stdout().execute(crossterm::terminal::EnterAlternateScreen)?;
        println!("Made it");

        let args: Vec<String> = std::env::args().skip(1).collect();

        crossterm::terminal::enable_raw_mode()?;

        std::io::stdout().execute(crossterm::event::EnableMouseCapture)?;
        print!("\x1B[?1000;1006;1015h");
        std::io::stdout().flush().unwrap();

        

        let mut redraw = true;
        if args.len() > 0 {
            self.open_doc = Some(0);
            for filename in args {
                match Document::load(filename) {
                    Ok(doc) => self.add_doc(doc),
                    Err(filename) => self.write_status_bar(Some(format!("File {} not found!",filename)))
                }
            }
        }
        else {
            loop {
                if redraw {
                    self.show_start_splash()?;
                    redraw = false;
                }

                let event = read()?;
                if let Key(k) = event {
                    if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('n') {
                        if let Ok(filename) = self.read_new_filename() {
                            if !Path::new(&filename).exists() {
                                self.open_doc = Some(self.docs.len());
                                self.add_doc(Document::new(filename));
                                break;
                            }
                            else {
                                self.write_status_bar(Some(format!("File {} already exists!",filename)))
                            }
                        }
                        else {
                            redraw = true;
                        }
                    }
                    else if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('o') {
                        if let Ok(filename) = self.read_new_filename() {
                            
                            match Document::load(filename) {
                                Ok(doc) => {
                                    self.open_doc = Some(self.docs.len());
                                    self.add_doc(doc);
                                    break;
                                },
                                Err(filename) => self.write_status_bar(Some(format!("File {} not found!",filename)))
                            }
                        }
                        else {
                            redraw = true;
                        }
                    }
                    else if k.code == KeyCode::Esc {
                        
                        std::io::stdout().execute(crossterm::event::DisableMouseCapture)?.execute(crossterm::terminal::Clear(crossterm::terminal::ClearType::All))?.execute(crossterm::cursor::MoveTo(0,0))?;
                        print!("\x1B[?1000;1006;1015l");
                        std::io::stdout().flush().unwrap();
                        crossterm::terminal::disable_raw_mode()?;

                        std::io::stdout().execute(crossterm::terminal::LeaveAlternateScreen)?;

                        return Ok(());
                    }
                }
                else if let Resize(w,h) = event {
                    self.resize(w as usize, h as usize);
                    redraw = true;
                }
            }
        }

        // Editor loop
        let mut clear = false;
        redraw = true;

        let mut mouse_event = false;

        let mut status_msg = String::new();

        'editor: loop {
            let (width, height) = (self.width() + 1,self.height());

            if redraw {
                print!("{}",crossterm::cursor::Hide);
                print!("{}",crossterm::cursor::MoveTo(0,0));
            }
            std::io::stdout().flush().unwrap();

            let num_docs = self.docs.len();

            if let Some(doc_index) = self.open_doc {
                let lines;
                {
                    let doc = &mut self.docs[doc_index];
                    lines = doc.rows.len();

                    if doc.rows.len() == 0 {
                        doc.rows.push(Row::empty());
                    }
        
                    if redraw {
                        let mut i = 0;
                        while i < height - 2 {
                            if i == 0 {
                                println!();
                            }
                            else if i + self.line_start - 1 < doc.rows.len() {
                                // TODO: Deal with lines larger than console!
                                if doc.rows[i - 1 + self.line_start].line_width() > width {
                                    let n = doc.rows[i - 1 + self.line_start].line_width() / width;
                                    println!("{}{}",doc.rows[i - 1 + self.line_start].buf," ".repeat(width * (n + 1) - doc.rows[i - 1 + self.line_start].line_width()));
                                    i += n;
                                }
                                else {
                                    println!("{}{}",doc.rows[i - 1 + self.line_start].buf," ".repeat(width - doc.rows[i - 1 + self.line_start].line_width()));
                                }
                            }
                            else {
                                println!("~{}"," ".repeat(width - 1));
                            }
                            i += 1;
                        }

                        if clear {
                            clear = false;
                            println!("~{}"," ".repeat(width - 1));
                            println!("~{}"," ".repeat(width - 1));                        
                        }
                    }
                    else {
                        redraw = true;
                    }
                }

                if !mouse_event {
                    self.draw_tabs();
                    #[cfg(debug_assertions)]
                    self.write_status_bar(Some(format!("Debug:[col:{} w:{} diff:{}] Line {} of {} {}",self.docs[doc_index].cursor_col,width,self.docs[doc_index].visual_rows_to(width, self.docs[doc_index].cursor_row) as i32 - self.line_start as i32,self.docs[doc_index].cursor_row + 1,lines,status_msg)));
                    #[cfg(not(debug_assertions))]
                    self.write_status_bar(Some(format!("Line {} of {} {}",self.docs[doc_index].cursor_row + 1,lines,status_msg)));
                }
                
                if !status_msg.is_empty() {
                    status_msg.clear();
                }

                {
                    let doc = &mut self.docs[doc_index];
                    
                    if !mouse_event {
                        Editor::position_cursor(doc.cursor_row - self.line_start,doc.cursor_col,&doc.rows[doc.cursor_row],width);
                    }
                    else {
                        mouse_event = false;
                    }

                    match read().unwrap() {
                        Key(k) => {
                            match k.code {
                                KeyCode::Char(c) => {
                                    if k.modifiers.contains(KeyModifiers::CONTROL) {
                                        if c == 's' {
                                            doc.save(&self.config)?;
                                            status_msg = format!("Saved file as {} in ",doc.filename);
                                        }
                                        else if c == 'n' {
                                            let filename = self.read_new_filename()?;
                                            self.open_doc = Some(self.docs.len());
                                            self.add_doc(Document::new(filename));

                                            continue 'editor;
                                        }
                                        else if c == 'o' {
                                            let filename = self.read_new_filename()?;
                                            match Document::load(filename) {
                                                Ok(doc) => {
                                                    self.open_doc = Some(self.docs.len());
                                                    self.add_doc(doc);

                                                    continue 'editor;
                                                },
                                                Err(filename) => self.write_status_bar(Some(format!("File {} not found!",filename)))
                                            }
                                        }
                                    }
                                    else {
                                        doc.rows[doc.cursor_row].insert_char(doc.cursor_col, c);
                                        doc.cursor_col += 1;
                                    }
                                },
                                KeyCode::Esc => break,
                                KeyCode::Backspace => {
                                    if doc.rows[doc.cursor_row].len() != 0 {
                                        if doc.cursor_col == doc.rows[doc.cursor_row].len() {
                                            doc.rows[doc.cursor_row].buf.pop();
                                            doc.cursor_col -= 1;
                                        }
                                        else {
                                            if doc.cursor_col == 0 {
                                                if doc.cursor_row != 0 {
                                                    doc.cursor_row -= 1;
                                                    doc.cursor_col = doc.rows[doc.cursor_row].len();
                                                    if doc.cursor_col == 0 {
                                                        doc.rows.remove(doc.cursor_row);
                                                    }
                                                    else {
                                                        let line = doc.rows[doc.cursor_row + 1].buf.clone();
                                                        doc.rows.remove(doc.cursor_row + 1);
                                                        doc.rows[doc.cursor_row].buf.push_str(&line);
                                                    }
                                                }
                                            }
                                            else {
                                                doc.rows[doc.cursor_row].remove_at(doc.cursor_col - 1);
                                                doc.cursor_col -= 1;
                                            }
                                        }
                                    }
                                    else {
                                        if doc.cursor_row != 0 {
                                            doc.rows.remove(doc.cursor_row);
                                            doc.cursor_row -= 1;
                                            doc.cursor_col = doc.rows[doc.cursor_row].len();
                                        }
                                    }
                                },
                                KeyCode::Delete => {
                                    if doc.rows[doc.cursor_row].len() != 0 {
                                        if doc.cursor_col == doc.rows[doc.cursor_row].len() {
                                            if doc.cursor_row + 1 < doc.rows.len() {
                                                let next_line = doc.rows[doc.cursor_row + 1].buf.clone();
                                                doc.rows[doc.cursor_row].buf.push_str(&next_line);

                                                doc.rows.remove(doc.cursor_row + 1);
                                            }
                                        }
                                        else {
                                            doc.rows[doc.cursor_row].remove_at(doc.cursor_col);
                                        }
                                    }
                                    else {
                                        if doc.cursor_row + 1 != doc.rows.len() {
                                            doc.rows.remove(doc.cursor_row);
                                        }
                                    }
                                },
                                KeyCode::Tab => {
                                    for c in self.config.tab_str.chars() {
                                        doc.rows[doc.cursor_row].insert_char(doc.cursor_col, c);
                                        doc.cursor_col += 1;
                                    }
                                },
                                KeyCode::Enter => {
                                    if doc.cursor_col == 0 {
                                        doc.rows.insert(doc.cursor_row,Row::empty());
                                        doc.cursor_row += 1;
                                        doc.cursor_col = 0;
                                    }
                                    else if doc.cursor_col == doc.rows[doc.cursor_row].len() {
                                        doc.rows.insert(doc.cursor_row + 1,Row::empty());
                                        doc.cursor_row += 1;
                                        doc.cursor_col = 0;
                                    }
                                    else {
                                        let (left, right) = doc.rows[doc.cursor_row].split_at(doc.cursor_col);
                                        doc.rows[doc.cursor_row] = Row::from_string(left);
                                        if doc.cursor_row + 1 == doc.rows.len() {
                                            doc.rows.insert(doc.cursor_row + 1, Row::empty());
                                        }
                                        else if doc.cursor_row + 1 > doc.rows.len() {
                                            doc.rows.push(Row::empty());
                                        }
                                        doc.rows[doc.cursor_row + 1] = Row::from_string(right);

                                        doc.cursor_row += 1;
                                        doc.cursor_col = 0;
                                    }
                                },
                                KeyCode::Up => {
                                    if k.modifiers.contains(KeyModifiers::SHIFT) {
                                        self.line_start += 1;
                                    }
                                    else if doc.cursor_row != 0 {
                                        doc.cursor_row -= 1;
                                        if doc.cursor_col > doc.rows[doc.cursor_row].len() {
                                            doc.cursor_col = doc.rows[doc.cursor_row].len();
                                        }
                                    }
                                    redraw = false;
                                },
                                KeyCode::Down => {
                                    if doc.rows[doc.cursor_row].line_width() > width {
                                        doc.cursor_col += width;
                                        if doc.cursor_col > doc.rows[doc.cursor_row].len() {
                                            doc.cursor_col = doc.rows[doc.cursor_row].len();
                                        }
                                    }
                                    else {
                                        if doc.cursor_row + 1 != doc.rows.len() {
                                            doc.cursor_row += 1;
                                            if doc.cursor_col > doc.rows[doc.cursor_row].len() {
                                                doc.cursor_col = doc.rows[doc.cursor_row].len();
                                            }
                                        }
                                    }
                                    redraw = false;
                                },
                                KeyCode::Left => {
                                    if k.modifiers.contains(KeyModifiers::CONTROL) {
                                        if self.open_doc.unwrap() != 0 {
                                            self.open_doc = Some(self.open_doc.unwrap() - 1);
                                        }
                                        else {
                                            self.open_doc = Some(num_docs - 1);
                                        }
                                    }
                                    else {
                                        if doc.cursor_col != 0 {
                                            doc.cursor_col -= 1;
                                        }
                                        else {
                                            if doc.cursor_row != 0 {
                                                doc.cursor_row -= 1;
                                                doc.cursor_col = doc.rows[doc.cursor_row].len();
                                            }
                                        }
                                        redraw = false;
                                    }
                                },
                                KeyCode::Right => {
                                    if k.modifiers.contains(KeyModifiers::CONTROL) {
                                        if self.open_doc.unwrap() + 1 < num_docs {
                                            self.open_doc = Some(self.open_doc.unwrap() + 1);
                                        }
                                        else {
                                            self.open_doc = Some(0);
                                        }
                                    }
                                    else {
                                        doc.cursor_col += 1;
                                        if doc.cursor_col > doc.rows[doc.cursor_row].len() {
                                            if doc.cursor_row + 1 != doc.rows.len() {
                                                doc.cursor_col = 0;
                                                doc.cursor_row += 1;
                                            }
                                            else {
                                                doc.cursor_col = doc.rows[doc.cursor_row].len();
                                            }
                                        }
                                        redraw = false;
                                    }
                                },
                                _ => {}
                            };
                        },
                        Resize(w,h) => {
                            self.resize(w as usize, h as usize);

                            clear = true;
                        },
                        Mouse(e) => {
                            if e.kind == MouseEventKind::Down(MouseButton::Left) {
                                if e.row != 0 {
                                    if e.row - 1 < doc.rows.len() as u16 {
                                        doc.cursor_row = e.row as usize - 1;
                                        if doc.cursor_col > doc.rows[doc.cursor_row].buf.len() {
                                            doc.cursor_col = doc.rows[doc.cursor_row].buf.len();
                                        }
                                    }
                                    if e.column < doc.rows[doc.cursor_row].buf.len() as u16 {
                                        doc.cursor_col = e.column as usize;
                                        while !doc.rows[doc.cursor_row].buf.is_char_boundary(doc.cursor_col) {
                                            doc.cursor_col -= 1;
                                        }
                                    }
                                    else {
                                        doc.cursor_col = doc.rows[doc.cursor_row].len();
                                    }
                                    Editor::position_cursor(doc.cursor_row - self.line_start,doc.cursor_col,&doc.rows[doc.cursor_row],width);
                                }
                                else {
                                    for (i,doc_index) in self.docs_mouse_cache.iter().enumerate() {
                                        if e.column > doc_index.0 as u16 && e.column < doc_index.1 as u16 {
                                            self.open_doc = Some(i);
                                            continue 'editor;
                                        }
                                    }
                                }
                            }
                            redraw = false;
                            mouse_event = true;
                        }
                    }
                }

                let actual_rows = height as i32 - 3;
                let diff = self.docs[doc_index].visual_rows_to(width, self.docs[doc_index].cursor_row) as i32 - self.line_start as i32;
                if diff >= actual_rows {
                    self.line_start += 1;
                    redraw = true;
                }
                else if diff < 0 {
                    self.line_start -= 1;
                    redraw = true;
                }
            }
        }

        
        std::io::stdout().execute(crossterm::event::DisableMouseCapture)?.execute(crossterm::terminal::Clear(crossterm::terminal::ClearType::All))?.execute(crossterm::cursor::MoveTo(0,0))?;
        print!("\x1B[?1000;1006;1015l");
        std::io::stdout().flush().unwrap();
        crossterm::terminal::disable_raw_mode()?;

        std::io::stdout().execute(crossterm::terminal::LeaveAlternateScreen)?;

        Ok(())
    }

    fn position_cursor(cursor_row: usize,cursor_col: usize,row: &Row, width: usize) {
        let mut y = cursor_row + 1 + cursor_col / width;
        let mut x = 0;

        let mut i = 0;
        for c in row.buf.chars().skip((cursor_col / width) * width) {
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

    fn draw_tabs(&self) {
        let width = self.width();

        let mut doc_bar = String::new();

        let mut i = 0;
        for doc in &self.docs {
            if doc_bar.len() + doc.filename.len() + 3 < width {
                if let Some(open_doc) = self.open_doc {
                    if open_doc == i {
                        doc_bar.push_str(&format!("{}|{}|{} ",crossterm::style::Attribute::Reverse, doc.display_name() ,crossterm::style::Attribute::Reset));
                        i += 1;
                        continue;
                    }
                }
                doc_bar.push_str(&format!("|{}| ",Path::new(&doc.filename).file_name().unwrap_or_default().to_str().unwrap_or_default()));
            }
            i += 1;
        }

        print!("{}{}{}",crossterm::cursor::MoveTo(0,0),doc_bar," ".repeat(width - doc_bar.len()));
    }

    fn read_new_filename(&self) -> Result<String, Box<dyn Error>> {
        let (width, height) = (self.width(),self.height());

        let mut filename = String::new();
        let index = self.docs.len();

        loop {
            let status_str = format!("[{}] - Doc {} of {}",filename,index,self.docs.len());

            let dir = std::env::current_dir().unwrap_or_default();
            let dir = format!("in [/{}]",dir.iter().last().unwrap().to_os_string().into_string().unwrap());
    
            print!("{}{}{}{}{}{}",crossterm::cursor::MoveTo(0,height as u16 - 2),crossterm::style::Attribute::Reverse,status_str," ".repeat(width as usize - status_str.len() - dir.len()),dir,crossterm::style::Attribute::Reset);
    
            std::io::stdout().flush().unwrap();

            if let Ok(Key(k)) = read() {
                if let KeyCode::Char(c) = k.code {
                    filename.push(c);
                }
                else if let KeyCode::Enter = k.code {
                    break;
                }
                else if k.code == KeyCode::Esc {
                    return Err("Stopped".into());
                }
                else if k.code == KeyCode::Backspace && filename.len() > 0 {
                    filename.remove(filename.len() - 1);
                }
            }
        }

        Ok(filename)
    }

    pub fn show_start_splash(&self) -> Result<(), Box<dyn Error>> {
        print!("{}",crossterm::cursor::MoveTo(0,0));

        std::io::stdout().flush()?;

        let (width,height): (usize, usize) = (self.width(),self.height());

        let title_string = format!("Kelp Editor - {}", kelp_version());
        let by_string = "Written in Rust by Vertex";
        

        for y in 0..height {
            if y != 0 && y == height / 2 {
                println!("~{}{}{}"," ".repeat(width / 2 - 1 - title_string.len() / 2), title_string, " ".repeat(width - (width / 2 + 1 + title_string.len() / 2)));
            }
            else if y != 0 && y - 1 == height / 2 {
                println!("~{}{}{}"," ".repeat(width / 2 - 1 - by_string.len() / 2), by_string, " ".repeat(width - (width / 2 + 1 + by_string.len() / 2)));
            }
            else {
                if y != height - 1 {
                    println!("~{}"," ".repeat(width - 1));
                }
                else {
                    print!("~{}"," ".repeat(width - 1));
                    std::io::stdout().flush()?;
                }
            }
        }

        self.write_status_bar(None);
        
        Ok(())
    }

    fn write_status_bar(&self,mut extra_info: Option<String>) {
        if extra_info.is_none() {
            extra_info = Some("Ctrl+N: New file | Ctrl+O: Open file  ".to_owned());
        }

        let (width, height) = (self.width(),self.height());
        // Status bar
        let filename;
        let index;
        if self.open_doc.is_none() {
            filename = "No document open".to_owned();
            index = 0;
        }
        else {
            filename = self.docs[self.open_doc.unwrap()].filename.clone();
            index = self.open_doc.unwrap() + 1;
        }

        let status_str = format!("[{}] - Doc {} of {}",filename,index,self.docs.len());

        let dir = std::env::current_dir().unwrap_or_default();
        let mut dir = format!("{} [/{}]",extra_info.unwrap_or("".to_owned()),dir.iter().last().unwrap().to_os_string().into_string().unwrap());

        if width < status_str.len() + dir.len() + 8 {
            dir.drain(..dir.char_indices().nth(status_str.len() + dir.len() + 8 - width).unwrap().0);
            dir.insert_str(0,"...");
        }

        print!("{}{}{}{}{}{}",crossterm::cursor::MoveTo(0,height as u16 - 2),crossterm::style::Attribute::Reverse,status_str," ".repeat(width as usize - status_str.len() - dir.len()),dir,crossterm::style::Attribute::Reset);

        std::io::stdout().flush().unwrap();
    }
}