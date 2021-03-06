use crate::Editor;
use crate::editor::*;
use crate::editor::highlight::Token;
use std::path::Path;
use crossterm::event::{MouseEventKind,MouseButton};
use crossterm::event::Event::Resize;
use crossterm::event::{Event::{Key,Mouse},KeyModifiers,KeyCode};
use crossterm::event::read;
use std::error::Error;

use crossterm::ExecutableCommand;

use std::io::Write;

impl Editor {
    pub fn start(mut self) -> Result<(), Box<dyn Error>> {
        std::io::stdout().execute(crossterm::terminal::EnterAlternateScreen)?;

        let args: Vec<String> = std::env::args().skip(1).collect();

        crossterm::terminal::enable_raw_mode()?;

        std::io::stdout().execute(crossterm::event::EnableMouseCapture)?;

        let mut is_conhost = false;

        let mut redraw;
        if args.len() > 0 {
            self.open_doc = Some(0);
            for filename in args {
                if filename == "--powershell" {
                    is_conhost = true;
                }

                match Document::load(filename) {
                    Ok(doc) => self.add_doc(doc),
                    Err(filename) => self.add_doc(Document::new(filename))
                }
            }
            let config = 
                if self.config.languages.contains_key(&self.docs[0].extension()) {
                    &self.config.languages[&self.docs[0].extension()]
                }
                else {
                    &self.config.languages[&"*".to_owned()]
                };

            self.docs[0].tokenize(0, self.height, config);
            
            if !is_conhost {
                print!("\x1B[?1000;1006;1015h"); // Enable for windows terminal cause the cfg based system switches to winapi calls
                std::io::stdout().flush().unwrap();
            }
        }
        else {
            if !is_conhost {
                print!("\x1B[?1000;1006;1015h"); // Enable for windows terminal cause the cfg based system switches to winapi calls
                std::io::stdout().flush().unwrap();
            }

            self.main_screen()?;
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

                let config = 
                    if self.config.languages.contains_key(&self.docs[doc_index].extension()) {
                        &self.config.languages[&self.docs[doc_index].extension()]
                    }
                    else {
                        &self.config.languages[&"*".to_owned()]
                    };

                let lines;
                {
                    let doc = &mut self.docs[doc_index];
                    lines = doc.rows.len();

                    if doc.rows.len() == 0 {
                        doc.rows.push(Row::empty());
                    }
        
                    if redraw {
                        Token::tokenize(&mut doc.rows,doc.line_start,height - 3 ,config);

                        let mut drawing_row = 0;
                        let mut processing_row = 0;

                        while drawing_row < height - 2 {
                            if drawing_row == 0 {
                                println!();
                            }
                            else if processing_row + doc.line_start - 1 < doc.rows.len() {
                                if doc.rows[processing_row - 1 + doc.line_start].line_width() > width {
                                    let n = doc.rows[processing_row - 1 + doc.line_start].line_width() / width;
                                    let padding = width * (n + 1) - doc.rows[processing_row - 1 + doc.line_start].line_width();
                                    println!("{}{}",doc.rows[processing_row - 1 + doc.line_start].display_buf(config)," ".repeat(padding));
                                    drawing_row += n;
                                }
                                else {
                                    let padding = width - doc.rows[processing_row - 1 + doc.line_start].line_width();
                                    println!("{}{}",doc.rows[processing_row - 1 + doc.line_start].display_buf(config)," ".repeat(padding));
                                }
                            }
                            else {
                                println!("~{}"," ".repeat(width - 1));
                            }
                            drawing_row += 1;
                            processing_row += 1;
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
                    {
                        let row = self.docs[doc_index].cursor_row;
                        self.write_status_bar(Some(format!("Debug:[col:{} tokens:{}] Line {} of {} {}",self.docs[doc_index].cursor_col,self.docs[doc_index].rows[row].tokens.len(),self.docs[doc_index].cursor_row + 1,lines,status_msg)));
                    }
                    #[cfg(not(debug_assertions))]
                    {
                        self.write_status_bar(Some(format!("Line {} of {} {}",self.docs[doc_index].cursor_row + 1,lines,status_msg)));
                    }
                }
                
                if !status_msg.is_empty() {
                    status_msg.clear();
                }

                {
                    let doc = &mut self.docs[doc_index];
                    
                    if !mouse_event {
                        Editor::position_cursor(doc.cursor_row,doc.cursor_col,&doc.rows,width,doc.line_start);
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
                                            doc.save(config)?;
                                            status_msg = format!("Saved file as {} in ",doc.filename);
                                        }
                                        else if c == 'n' {
                                            if let Ok(filename) = self.read_new_filename(None) {
                                                if !Path::new(&filename).exists() {
                                                    self.open_doc = Some(self.docs.len());
                                                    let mut doc = Document::new(filename);
                                                    doc.tokenize(0, height, config);
                                                    self.add_doc(doc);
                                                }
                                                else {
                                                    status_msg = format!("File {} already exists!",filename);
                                                }
                                                continue 'editor;
                                            }
                                        }
                                        else if c == 'o' {
                                            if let Ok(filename) = self.read_new_filename(None) {    
                                                match Document::load(filename) {
                                                    Ok(mut doc) => {
                                                        self.open_doc = Some(self.docs.len());
                                                        doc.tokenize(0, height, config);
                                                        self.add_doc(doc);

                                                        continue 'editor;
                                                    },
                                                    Err(filename) => self.write_status_bar(Some(format!("File {} not found!",filename)))
                                                }
                                            }
                                        }
                                        else if c == 'w' {
                                            self.docs.remove(doc_index);
                                            if doc_index != 0 && doc_index - 1 < self.docs.len() {
                                                self.open_doc = Some(doc_index - 1);
                                            }
                                            else {
                                                self.open_doc = None;
                                                self.main_screen()?;
                                            }
                                            continue 'editor;
                                        }
                                        else if c == 'g' {
                                            if let Ok(mut command) = self.read_new_filename(Some("j".to_owned())) {
                                                if command.starts_with('j') {
                                                    command.drain(..1);
                                                    if let Ok(line) = command.parse::<usize>() {
                                                        if line < self.docs[doc_index].rows.len() {
                                                            self.docs[doc_index].line_start = line;
                                                            self.docs[doc_index].cursor_row = line;
                                                            self.docs[doc_index].cursor_col = 0;
                                                        }
                                                    }
                                                }
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
                                    for c in unescape(&config.tab_str).unwrap().chars() {
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
                                        if doc.cursor_row + 1 <= doc.rows.len() {
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
                                    if doc.rows[doc.cursor_row].line_width() > width && doc.cursor_col > width {
                                        doc.cursor_col -= width;
                                    }
                                    else {
                                        if doc.cursor_row != 0 {
                                            doc.cursor_row -= 1;
                                            if doc.cursor_col > doc.rows[doc.cursor_row].len() {
                                                doc.cursor_col = doc.rows[doc.cursor_row].len();
                                            }
                                        }
                                    }
                                    redraw = false;
                                },
                                KeyCode::Down => {
                                    if doc.rows[doc.cursor_row].line_width() > width {
                                        doc.cursor_col += width;
                                        if doc.cursor_col > doc.rows[doc.cursor_row].len() {
                                            doc.cursor_col = doc.cursor_col % width;
                                            doc.cursor_row += 1;
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
                                    if k.modifiers.contains(KeyModifiers::CONTROL) && k.modifiers.contains(KeyModifiers::SHIFT) {
                                        if self.open_doc.unwrap() != 0 {
                                            self.open_doc = Some(self.open_doc.unwrap() - 1);
                                        }
                                        else {
                                            self.open_doc = Some(num_docs - 1);
                                        }
                                    }
                                    else if k.modifiers.contains(KeyModifiers::CONTROL) {
                                        if doc.rows[doc.cursor_row].tokens.len() != 0 && doc.cursor_col != 0 {
                                            let mut last_end = 0;

                                            for token in &doc.rows[doc.cursor_row].tokens {
                                                if token.end() > doc.cursor_col - 1 {
                                                    doc.cursor_col = last_end;
                                                    break;
                                                }
                                                else {
                                                    last_end = token.end();
                                                }
                                            }
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
                                    if k.modifiers.contains(KeyModifiers::CONTROL) && k.modifiers.contains(KeyModifiers::SHIFT) {
                                        if self.open_doc.unwrap() + 1 < num_docs {
                                            self.open_doc = Some(self.open_doc.unwrap() + 1);
                                        }
                                        else {
                                            self.open_doc = Some(0);
                                        }
                                    }
                                    else if k.modifiers.contains(KeyModifiers::CONTROL) {
                                        let mut found_next_token = false;

                                        if doc.rows[doc.cursor_row].tokens.len() != 0 {
                                            for token in &doc.rows[doc.cursor_row].tokens {
                                                if token.start() > doc.cursor_col + 1 {
                                                    doc.cursor_col = token.start();
                                                    found_next_token = true;
                                                    break;
                                                }
                                            }
                                        }

                                        if !found_next_token {
                                            doc.cursor_col = doc.rows[doc.cursor_row].len();
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
                                KeyCode::Home => {
                                    doc.line_start = 0;
                                    doc.cursor_col = 0;
                                    doc.cursor_row = 0;
                                },
                                KeyCode::End => {
                                    doc.line_start = doc.rows.len() - height + 3;
                                    doc.cursor_col = 0;
                                    doc.cursor_row = doc.rows.len() - 1;
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
                                    Editor::position_cursor(doc.cursor_row,doc.cursor_col,&doc.rows,width,doc.line_start);
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
                let diff = self.docs[doc_index].visual_rows_to(width, self.docs[doc_index].cursor_row) as i32 - self.docs[doc_index].visual_rows_to(width, self.docs[doc_index].line_start) as i32;
                if diff >= actual_rows {
                    self.docs[doc_index].line_start += 1;
                    redraw = true;
                }
                else if diff < 0 && self.docs[doc_index].line_start != 0 {
                    self.docs[doc_index].line_start -= 1;
                    redraw = true;
                }
            }
            else {

            }
        }

        
        std::io::stdout().execute(crossterm::event::DisableMouseCapture)?.execute(crossterm::terminal::Clear(crossterm::terminal::ClearType::All))?.execute(crossterm::cursor::MoveTo(0,0))?;
        print!("\x1B[?1000;1006;1015l");
        std::io::stdout().flush().unwrap();
        crossterm::terminal::disable_raw_mode()?;

        std::io::stdout().execute(crossterm::terminal::LeaveAlternateScreen)?;

        Ok(())
    }

    pub fn main_screen(&mut self) -> Result<(),Box<dyn Error>> {
        let mut redraw = true;

        loop {
            if redraw {
                self.show_start_splash()?;
                redraw = false;
            }

            let event = read()?;
            if let Key(k) = event {
                if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('n') {
                    if let Ok(filename) = self.read_new_filename(None) {
                        if !Path::new(&filename).exists() {
                            self.open_doc = Some(self.docs.len());
                            self.add_doc(Document::new(filename));
                            return Ok(());
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
                    if let Ok(filename) = self.read_new_filename(None) {
                        
                        match Document::load(filename) {
                            Ok(mut doc) => {
                                let config = 
                                    if self.config.languages.contains_key(&doc.extension()) {
                                        &self.config.languages[&doc.extension()]
                                    }
                                    else {
                                        &self.config.languages[&"*".to_owned()]
                                    };

                                self.open_doc = Some(self.docs.len());
                                doc.tokenize(0, self.height, config);

                                self.add_doc(doc);
                                return Ok(());
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

                    return Err("Close".into());
                }
            }
            else if let Resize(w,h) = event {
                self.resize(w as usize, h as usize);
                redraw = true;
            }
        }
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

    fn read_new_filename(&self, filename_def: Option<String>) -> Result<String, Box<dyn Error>> {
        let (width, height) = (self.width(),self.height());

        let mut filename = 
            if filename_def.is_none() {
                String::new()
            }
            else {
                filename_def.unwrap()
            };
        let index = self.docs.len();

        loop {
            let mut status_str = format!("[{}] - Doc {} of {}",filename,index,self.docs.len());

            let dir = std::env::current_dir().unwrap_or_default();
            let dir = format!("in [/{}]",dir.iter().last().unwrap().to_os_string().into_string().unwrap());

            if width < status_str.len() + dir.len() + 10 {
                status_str.drain(..status_str.char_indices().nth(status_str.len() + dir.len() + 14 - width).unwrap().0);
                status_str.insert_str(0,"[...");
            }
    
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
}