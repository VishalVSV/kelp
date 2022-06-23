//  _  __    _       
// | |/ /   | |      
// | ' / ___| |_ __  
// |  < / _ \ | '_ \ 
// | . \  __/ | |_) |
// |_|\_\___|_| .__/ 
//            | |    
//            |_|    
// Made by vertex

use crate::editor::highlight::Token;
use crate::editor::history::LineDeleteMode;
use crate::editor::history::UndoRedo;
use crate::editor::prelude::*;
use crate::editor::utils::pad_center;
use crate::editor::utils::pad_center_str;
use crate::editor::*;
use crate::editor::prelude::Editor;
use clipboard::ClipboardContext;
use clipboard::ClipboardProvider;
use crossterm::event::read;
use crossterm::event::Event::Resize;
use crossterm::event::{
    Event::{Key, Mouse},
    KeyCode, KeyModifiers,
};
use crossterm::event::{MouseButton, MouseEventKind};
use crossterm::style::Color;
use std::error::Error;
use std::io::stdout;
use std::path::Path;

use crossterm::ExecutableCommand;

use std::io::Write;

/*
    +==============================================================================+
    |   _   _      _                    __                  _   _                  |
    |  | | | |    | |                  / _|                | | (_)                 |
    |  | |_| | ___| |_ __   ___ _ __  | |_ _   _ _ __   ___| |_ _  ___  _ __  ___  |
    |  |  _  |/ _ \ | '_ \ / _ \ '__| |  _| | | | '_ \ / __| __| |/ _ \| '_ \/ __| |
    |  | | | |  __/ | |_) |  __/ |    | | | |_| | | | | (__| |_| | (_) | | | \__ \ |
    |  \_| |_/\___|_| .__/ \___|_|    |_|  \__,_|_| |_|\___|\__|_|\___/|_| |_|___/ |
    |               |_|                                                            |
    |               | |                                                            |
    +==============================================================================+

*/

#[cfg(debug_assertions)]
pub fn is_debug() -> bool {
    true
}

#[cfg(not(debug_assertions))]
pub fn is_debug() -> bool {
    false
}

#[allow(dead_code)]
pub fn debug_file() -> String {
    let mut path = std::env::current_exe().unwrap();
    path.pop();
    path.push("debug.txt");
    path.to_string_lossy().to_string()
}

#[cfg(not(windows))]
pub fn line_ending() -> String {
    "\n".to_owned()
}

#[cfg(windows)]
pub fn line_ending() -> String {
    "\r\n".to_owned()
}

pub fn char_width(chr: char, file_config: &FileConfig) -> Option<usize> {
    if chr != '\t' {
        chr.width()
    } else {
        Some(file_config.tab_str.len())
    }
}

/*
    +=======================================================+
    |  ___  ___      _         _____    _ _ _               |
    |  |  \/  |     (_)       |  ___|  | (_) |              |
    |  | .  . | __ _ _ _ __   | |__  __| |_| |_ ___  _ __   |
    |  | |\/| |/ _` | | '_ \  |  __|/ _` | | __/ _ \| '__|  |
    |  | |  | | (_| | | | | | | |__| (_| | | || (_) | |     |
    |  \_|  |_/\__,_|_|_| |_| \____/\__,_|_|\__\___/|_|     |
    |                                                       |
    +=======================================================+
*/

impl Editor {
    pub fn start(mut self) -> Result<(), Box<dyn Error>> {
        std::io::stdout().execute(crossterm::terminal::EnterAlternateScreen)?;

        let args: Vec<String> = std::env::args().skip(1).collect();

        crossterm::terminal::enable_raw_mode()?;

        std::io::stdout().execute(crossterm::event::EnableMouseCapture)?;

        let mut is_conhost = false;

        // If any files are included open them
        if args.len() > 0 {
            self.open_doc = Some(0);
            for filename in args {
                // Powershell supports VT100 but is considered a windows terminal by crossterm to the best of my knowledge so a few bugs pop up if this isn't used
                if filename == "--powershell" {
                    is_conhost = true;
                }

                // Open the file if exists, if not make a new file
                if filename.starts_with("bin:") {
                    let filename = &filename[4..];
                    match BinaryDocument::load(filename.to_owned()) {
                        Ok(doc) => self.add_bin_doc(doc),
                        Err(filename) => self.add_bin_doc(BinaryDocument::new(filename)),
                    }
                } else {
                    match TextDocument::load(filename) {
                        Ok(doc) => self.add_doc(doc),
                        Err(filename) => self.add_doc(TextDocument::new(filename)),
                    }
                }
            }

            if !is_conhost {
                print!("\x1B[?1000;1006;1015h"); // Enable for windows terminal cause the cfg based system switches to winapi calls
                std::io::stdout().flush().unwrap();
            }
        } else {
            if !is_conhost {
                print!("\x1B[?1000;1006;1015h"); // Enable for windows terminal cause the cfg based system switches to winapi calls
                std::io::stdout().flush().unwrap();
            }

            // Display starting screen
            self.main_screen()?;
        }

        // Editor loop
        self.redraw = true;
        let mut _last_letter: Option<std::time::SystemTime> = None;

        // Diagnostic data

        'editor: loop {
            if let Some((w, h)) = self.resize {
                self.resize(w, h);
                self.resize = None;
            }

            if self.redraw {
                print!("{}", crossterm::cursor::Hide);
                print!("{}", crossterm::cursor::MoveTo(0, 0));
                std::io::stdout().flush().unwrap();
            }

            let (width, height) = (self.width(), self.height());

            if let Some(doc_index) = self.open_doc {
                macro_rules! save_file {
                    () => {{
                        if self.docs[doc_index].is_text_doc() {
                            let config = if self
                                .config
                                .languages
                                .contains_key(&self.docs[doc_index].as_mut_text_doc().extension())
                            {
                                &self.config.languages
                                    [&self.docs[doc_index].as_mut_text_doc().extension()]
                            } else {
                                &self.config.languages[&"*".to_owned()]
                            };
                            self.docs[doc_index].as_mut_text_doc().save(config)?;
                            self.docs[doc_index].as_mut_text_doc().dirty = 0;
                            self.status_msg =
                                format!("Saved file as {} in ", self.docs[doc_index].filename());
                        } else if self.docs[doc_index].is_binary_doc() {
                            self.docs[doc_index].as_bin_doc().save()?;
                            self.docs[doc_index].as_bin_doc().dirty = 0;
                            self.status_msg =
                                format!("Saved file as {} in ", self.docs[doc_index].filename());
                        }
                    }};
                }

                macro_rules! new_file {
                    () => {{
                        if let Ok(filename) = &self.read_new_filename(None) {
                            if !Path::new(&filename).exists() {
                                if filename.starts_with("bin:") {
                                    let mut filename = filename.clone();
                                    filename.drain(..4);

                                    self.open_doc = Some(self.docs.len());
                                    let doc = BinaryDocument::new(filename.clone());
                                    self.add_bin_doc(doc);
                                } else {
                                    let config = if self.config.languages.contains_key(
                                        &self.docs[doc_index].as_mut_text_doc().extension(),
                                    ) {
                                        &self.config.languages
                                            [&self.docs[doc_index].as_mut_text_doc().extension()]
                                    } else {
                                        &self.config.languages[&"*".to_owned()]
                                    };
                                    self.open_doc = Some(self.docs.len());
                                    let mut doc = TextDocument::new(filename.clone());
                                    doc.tokenize(0, height, config);
                                    self.add_doc(doc);
                                }
                            } else {
                                self.status_msg = format!("File {} already exists!", filename);
                            }
                            continue 'editor;
                        }
                    }};
                }

                macro_rules! open_file {
                    () => {{
                        if let Ok(mut filename) = self.read_new_filename(None) {
                            if filename.starts_with("bin:") {
                                filename.drain(..4);
                                match BinaryDocument::load(filename) {
                                    Ok(doc) => {
                                        self.open_doc = Some(self.docs.len());
                                        self.add_bin_doc(doc);

                                        continue 'editor;
                                    }
                                    Err(filename) => self.write_status_bar(Some(format!(
                                        "File {} not found!",
                                        filename
                                    ))),
                                }
                            } else {
                                match TextDocument::load(filename) {
                                    Ok(mut doc) => {
                                        let config =
                                            if self.config.languages.contains_key(&doc.extension())
                                            {
                                                &self.config.languages[&doc.extension()]
                                            } else {
                                                &self.config.languages[&"*".to_owned()]
                                            };
                                        self.open_doc = Some(self.docs.len());
                                        doc.tokenize(0, height, config);
                                        self.add_doc(doc);

                                        continue 'editor;
                                    }
                                    Err(filename) => self.write_status_bar(Some(format!(
                                        "File {} not found!",
                                        filename
                                    ))),
                                }
                            }
                        }
                    }};
                }

                macro_rules! close_file {
                    () => {
                        if self.docs[doc_index].is_text_doc() {
                            if self.docs[doc_index].as_mut_text_doc().dirty == 0 {
                                self.docs.remove(doc_index);

                                if self.docs.len() == 0 {
                                    self.open_doc = None;
                                    self.main_screen()?;
                                } else {
                                    if doc_index != 0 && doc_index - 1 < self.docs.len() {
                                        self.open_doc = Some(doc_index - 1);
                                    } else {
                                        self.open_doc = Some(0);
                                    }
                                }
                                continue 'editor;
                            } else {
                                if self
                                    .show_prompt(
                                        "WARNING!".to_owned(),
                                        "You haven't saved this file!".to_owned(),
                                        "Close anways".to_owned(),
                                        "Don't close".to_owned(),
                                    )
                                    .is_ok()
                                {
                                    self.docs.remove(doc_index);

                                    if self.docs.len() == 0 {
                                        self.open_doc = None;
                                        self.main_screen()?;
                                    } else {
                                        if doc_index != 0 && doc_index - 1 < self.docs.len() {
                                            self.open_doc = Some(doc_index - 1);
                                        } else {
                                            self.open_doc = Some(0);
                                        }
                                    }
                                    continue 'editor;
                                }
                            }
                        } else if self.docs[doc_index].is_binary_doc() {
                            if self.docs[doc_index].as_bin_doc().dirty == 0 {
                                self.docs.remove(doc_index);

                                if self.docs.len() == 0 {
                                    self.open_doc = None;
                                    self.main_screen()?;
                                } else {
                                    if doc_index != 0 && doc_index - 1 < self.docs.len() {
                                        self.open_doc = Some(doc_index - 1);
                                    } else {
                                        self.open_doc = Some(0);
                                    }
                                }
                                continue 'editor;
                            } else {
                                if self
                                    .show_prompt(
                                        "WARNING!".to_owned(),
                                        "You haven't saved this file!".to_owned(),
                                        "Close anways".to_owned(),
                                        "Don't close".to_owned(),
                                    )
                                    .is_ok()
                                {
                                    self.docs.remove(doc_index);

                                    if self.docs.len() == 0 {
                                        self.open_doc = None;
                                        self.main_screen()?;
                                    } else {
                                        if doc_index != 0 && doc_index - 1 < self.docs.len() {
                                            self.open_doc = Some(doc_index - 1);
                                        } else {
                                            self.open_doc = Some(0);
                                        }
                                    }
                                    continue 'editor;
                                }
                            }
                        }
                    };
                }

                if self.docs[doc_index].is_text_doc() {
                    let num_docs = self.docs.len();

                    let config = if self
                        .config
                        .languages
                        .contains_key(&self.docs[doc_index].as_mut_text_doc().extension())
                    {
                        self.config.languages[&self.docs[doc_index].as_text_doc().extension()].clone()
                    } else {
                        self.config.languages[&"*".to_owned()].clone()
                    };

                    let lines = self.docs[doc_index].as_mut_text_doc().rows.len();

                    if self.docs[doc_index].as_mut_text_doc().rows.len() == 0 {
                        self.docs[doc_index].as_mut_text_doc().rows.push(Row::empty());
                    }

                    if !self.undergoing_selection {
                        self.docs[doc_index].as_mut_text_doc().selection = None;
                    } else {
                        self.undergoing_selection = false;
                    }

                    let selection = self.docs[doc_index].as_mut_text_doc().selection.clone();

                    if self.redraw {
                        if self.docs[doc_index].as_mut_text_doc().selection.is_some() {
                            let s = self.docs[doc_index]
                                .as_mut_text_doc()
                                .selection
                                .as_mut()
                                .unwrap();

                            if s.start_row == s.end_row && s.end_col == s.start_col {
                                self.docs[doc_index].as_mut_text_doc().selection = None;
                            }
                        }

                        let line_start = self.docs[doc_index].as_mut_text_doc().line_start;

                        Token::tokenize(
                            &mut self.docs[doc_index].as_mut_text_doc().rows,
                            HighlightingInfo {
                                selection: selection,
                            },
                            line_start,
                            height - 3,
                            &config,
                        );

                        let mut drawing_row = 0;
                        let mut processing_row = 0;

                        while drawing_row < height - 2 {
                            if drawing_row == 0 {
                                println!();
                            } else if processing_row + line_start - 1
                                < self.docs[doc_index].as_mut_text_doc().rows.len()
                            {
                                if self.docs[doc_index].as_mut_text_doc().rows
                                    [processing_row - 1 + line_start]
                                    .line_width(&config)
                                    > width
                                {
                                    let n = self.docs[doc_index].as_mut_text_doc().rows
                                        [processing_row - 1 + line_start]
                                        .line_width(&config)
                                        / width;
                                    let padding = width * (n + 1)
                                        - self.docs[doc_index].as_mut_text_doc().rows
                                            [processing_row - 1 + line_start]
                                            .line_width(&config);
                                    println!(
                                        "{}{}{}{}",
                                        self.docs[doc_index].as_mut_text_doc().rows
                                            [processing_row - 1 + line_start]
                                            .display_buf(&config, &self.config.theme),
                                        crossterm::style::SetBackgroundColor(Color::from(
                                            self.config.theme.background_color
                                        )),
                                        crossterm::style::SetForegroundColor(Color::from(
                                            self.config.theme.foreground_color
                                        )),
                                        " ".repeat(padding)
                                    );
                                    drawing_row += n;
                                } else {
                                    let padding = width
                                        - self.docs[doc_index].as_mut_text_doc().rows
                                            [processing_row - 1 + line_start]
                                            .line_width(&config);
                                    println!(
                                        "{}{}{}{}",
                                        self.docs[doc_index].as_mut_text_doc().rows
                                            [processing_row - 1 + line_start]
                                            .display_buf(&config, &self.config.theme),
                                        crossterm::style::SetBackgroundColor(Color::from(
                                            self.config.theme.background_color
                                        )),
                                        crossterm::style::SetForegroundColor(Color::from(
                                            self.config.theme.foreground_color
                                        )),
                                        " ".repeat(padding)
                                    );
                                }
                            } else {
                                println!("~{}", " ".repeat(width - 1));
                            }
                            drawing_row += 1;
                            processing_row += 1;
                        }

                        if self.clear {
                            self.clear = false;
                            println!("~{}", " ".repeat(width - 1));
                            println!("~{}", " ".repeat(width - 1));
                        }

                        std::io::stdout().flush()?;
                    } else {
                        self.redraw = true;
                    }

                    if !self.mouse_event {
                        self.draw_tabs();
                        if is_debug() {
                            let row = self.docs[doc_index].as_mut_text_doc().cursor_row;
                            let status;
                            {
                                let doc = self.docs[doc_index].as_mut_text_doc();
                                status = format!("Debug:[col:{} tokens:{} i:{} history_count:{}] Line {} of {} {}",doc.cursor_col,doc.rows[row].tokens.len(), doc.history_index.unwrap_or(0),doc.history.len(),doc.cursor_row + 1,lines,self.status_msg);
                            }
                            self.write_status_bar(Some(status));
                        } else {
                            let status;
                            {
                                let doc = self.docs[doc_index].as_mut_text_doc();
                                status = format!(
                                    "Line {} of {} {}",
                                    doc.cursor_row + 1,
                                    lines,
                                    self.status_msg
                                );
                            }
                            self.write_status_bar(Some(status));
                        }
                    }

                    if !self.status_msg.is_empty() {
                        self.status_msg.clear();
                    }

                    macro_rules! process_command {
                        () => {
                            if let Ok(mut command) = self.read_new_filename(Some("j".to_owned())) {
                                if command.starts_with('j') {
                                    command.drain(..1);
                                    let doc = self.docs[doc_index].as_mut_text_doc();
                                    if let Ok(line) = command.parse::<usize>() {
                                        if line < doc.rows.len() {
                                            doc.line_start = line;
                                            doc.cursor_row = line;
                                            doc.cursor_col = 0;
                                        }
                                    }
                                } else if command.starts_with("cd") {
                                    command.drain(..2);
                                    let mut dir = std::env::current_dir().unwrap();
                                    if command.trim() == ".." {
                                        if dir.pop() {
                                            std::env::set_current_dir(dir)?;
                                        }
                                    } else {
                                        dir.push(command.trim());
                                        if dir.exists() {
                                            std::env::set_current_dir(dir)?;
                                        }
                                    }
                                }
                            }
                        };
                    }

                    macro_rules! undo_last {
                        () => {{
                            let doc = self.docs[doc_index].as_mut_text_doc();
                            if let Some(history_index) = doc.history_index {
                                if history_index < doc.history.len() {
                                    let action = doc.history[history_index].clone();
                                    let (x, y) = action.apply(UndoRedo::Undo, doc);

                                    if history_index == 0 {
                                        doc.history_index = None;
                                    } else {
                                        *doc.history_index.as_mut().unwrap() -= 1;
                                    }
                                    doc.cursor_col = x;
                                    doc.cursor_row = y;
                                }
                            }
                        }};
                    }

                    macro_rules! redo_last {
                        () => {{
                            let doc = self.docs[doc_index].as_mut_text_doc();
                            if let Some(history_index) = doc.history_index {
                                if history_index + 1 < doc.history.len() {
                                    let action = doc.history[history_index + 1].clone();
                                    let (x, y) = action.apply(UndoRedo::Redo, doc);
                                    *doc.history_index.as_mut().unwrap() += 1;
                                    doc.cursor_col = x;
                                    doc.cursor_row = y;
                                }
                            } else if doc.history.len() > 0 {
                                self.status_msg = format!("{:?}", doc.history[0]);

                                let action = doc.history[0].clone();
                                let (x, y) = action.apply(UndoRedo::Redo, doc);
                                doc.history_index = Some(0);
                                doc.cursor_col = x;
                                doc.cursor_row = y;
                            }
                        }};
                    }

                    macro_rules! copy_selection {
                        () => {{
                            let doc = self.docs[doc_index].as_mut_text_doc();
                            if let Some(sel) = doc.selection {
                                let mut selection_string = String::new();
                                if sel.start_row == sel.end_row {
                                    let start = std::cmp::min(sel.start_col, sel.end_col);
                                    let end = std::cmp::max(sel.start_col, sel.end_col);

                                    selection_string =
                                        doc.rows[sel.start_row].substring(start, end).to_string();
                                } else {
                                    let start = std::cmp::min(sel.start_row, sel.end_row);
                                    let end = std::cmp::max(sel.start_row, sel.end_row);
                                    let start_col = if sel.start_row == start {
                                        sel.start_col
                                    } else {
                                        sel.end_col
                                    };
                                    let end_col = if sel.end_col == end {
                                        sel.end_col
                                    } else {
                                        sel.start_col
                                    };

                                    for i in start..=end {
                                        if i == start {
                                            selection_string.push_str(&format!(
                                                "{}{}",
                                                doc.rows[i].substring(start_col, doc.rows[i].len()),
                                                line_ending()
                                            ))
                                        } else if i == end {
                                            selection_string
                                                .push_str(doc.rows[i].substring(0, end_col));
                                        } else {
                                            selection_string.push_str(&format!(
                                                "{}{}",
                                                doc.rows[i].buf,
                                                line_ending()
                                            ));
                                        }
                                    }
                                }

                                self.undergoing_selection = true;

                                let mut ctx: ClipboardContext = ClipboardProvider::new().unwrap();
                                ctx.set_contents(selection_string)?;
                            }
                        }};
                    }

                    macro_rules! paste_clip {
                        () => {{
                            let doc = self.docs[doc_index].as_mut_text_doc();
                            let mut ctx: ClipboardContext = ClipboardProvider::new().unwrap();
                            if let Ok(clipboard_contents) = ctx.get_contents() {
                                let mut i = 0;
                                let line_count = clipboard_contents.lines().count();

                                let mut paste_diff = Vec::new();

                                for line in clipboard_contents.lines() {
                                    let row = doc.cursor_row;
                                    for c in line.chars() {
                                        let col = doc.cursor_col;

                                        doc.dirty += 1;
                                        doc.rows[row].insert_char(col, c);
                                        paste_diff.push(EditDiff::InsertChar(col, row, c));

                                        doc.cursor_col += 1;
                                    }

                                    if i + 1 < line_count {
                                        doc.rows.insert(row + 1, Row::empty());
                                        paste_diff.push(EditDiff::NewLine(row + 1));
                                        doc.cursor_row += 1;
                                        doc.cursor_col = 0;
                                    }

                                    i += 1;
                                }

                                doc.add_diff(EditDiff::Compound(paste_diff));
                            }
                        }};
                    }

                    if !self.mouse_event {
                        let doc = self.docs[doc_index].as_mut_text_doc();
                        Editor::position_cursor(
                            doc.cursor_row,
                            doc.cursor_col,
                            &doc.rows,
                            width,
                            doc.line_start,
                            &config,
                        );
                    } else {
                        self.mouse_event = false;
                    }

                    let event = read().unwrap();
                    let mut process_event = true;

                    if let crossterm::event::Event::Key(k) = event {
                        for (name, keybound_event) in &self.config.keybinds {
                            if keybound_event.equals(&k) {
                                match &name[..] {
                                    "copy" => copy_selection!(),
                                    "paste" => paste_clip!(),
                                    "redo" => redo_last!(),
                                    "undo" => undo_last!(),
                                    "start_command" => process_command!(),
                                    "close_file" => close_file!(),
                                    "open_file" => open_file!(),
                                    "save_file" => save_file!(),
                                    "new_file" => new_file!(),
                                    _ => {}
                                }
                                process_event = false;
                            }
                        }
                    }

                    if process_event {
                        match event {
                            Key(k) => {
                                match k.code {
                                    KeyCode::Char(c) => {
                                        let doc = self.docs[doc_index].as_mut_text_doc();
                                        if doc.selection.is_some() {
                                            doc.selection.as_mut().unwrap().normalize();

                                            if let Some(selection) = doc.selection {
                                                doc.cursor_col = selection.start_col;
                                                doc.cursor_row = selection.start_row;

                                                if selection.start_row == selection.end_row {
                                                    let mut diffs = Vec::new();

                                                    for _ in selection.start_col..selection.end_col
                                                    {
                                                        let c = doc.rows[selection.start_row]
                                                            .remove_at(selection.start_col);
                                                        diffs.push(EditDiff::DeleteChar(
                                                            selection.start_col + 1,
                                                            selection.start_row,
                                                            c,
                                                            false,
                                                        ));
                                                    }

                                                    doc.add_diff(EditDiff::Compound(diffs));
                                                } else {
                                                    let mut ly = selection.start_row;
                                                    let mut line_index = selection.start_row;

                                                    let mut diffs = Vec::new();

                                                    while ly <= selection.end_row {
                                                        if ly == selection.start_row {
                                                            if selection.start_col == 0 {
                                                                let line =
                                                                    doc.rows.remove(line_index);
                                                                diffs.push(EditDiff::DeleteLine(
                                                                    line_index,
                                                                    line.buf,
                                                                    LineDeleteMode::WholeLine,
                                                                ));
                                                            } else {
                                                                let len =
                                                                    doc.rows[line_index].len();
                                                                for _ in selection.start_col..len {
                                                                    let c = doc.rows[line_index]
                                                                        .remove_at(
                                                                            selection.start_col,
                                                                        );
                                                                    diffs.push(
                                                                        EditDiff::DeleteChar(
                                                                            selection.start_col + 1,
                                                                            selection.start_row,
                                                                            c,
                                                                            false,
                                                                        ),
                                                                    );
                                                                }

                                                                line_index += 1;
                                                            }
                                                        } else if ly == selection.end_row {
                                                            let len = doc.rows[line_index].len();
                                                            if selection.end_col == len {
                                                                let line =
                                                                    doc.rows.remove(line_index);
                                                                diffs.push(EditDiff::DeleteLine(
                                                                    line_index,
                                                                    line.buf,
                                                                    LineDeleteMode::WholeLine,
                                                                ));
                                                            } else {
                                                                for _ in 0..selection.end_col {
                                                                    let c = doc.rows[line_index]
                                                                        .remove_at(0);
                                                                    diffs.push(
                                                                        EditDiff::DeleteChar(
                                                                            1,
                                                                            selection.end_row,
                                                                            c,
                                                                            false,
                                                                        ),
                                                                    );
                                                                }

                                                                line_index += 1;
                                                            }
                                                        } else {
                                                            let line = doc.rows.remove(line_index);
                                                            diffs.push(EditDiff::DeleteLine(
                                                                line_index,
                                                                line.buf,
                                                                LineDeleteMode::WholeLine,
                                                            ));
                                                        }

                                                        ly += 1;
                                                    }
                                                    doc.add_diff(EditDiff::Compound(diffs));
                                                }
                                            }
                                        }

                                        if doc.cursor_col != 0 {
                                            let last_chr = doc.rows[doc.cursor_row]
                                                .char_at(doc.cursor_col - 1);
                                            if config.auto_close.contains_key(&last_chr) {
                                                if c == config.auto_close[&last_chr]
                                                    && doc.to_auto_close
                                                {
                                                    doc.cursor_col += 1;
                                                    continue;
                                                }
                                            }
                                        }

                                        doc.to_auto_close = false;

                                        doc.dirty += 1;
                                        doc.rows[doc.cursor_row].insert_char(doc.cursor_col, c);

                                        doc.add_diff(EditDiff::InsertChar(
                                            doc.cursor_col,
                                            doc.cursor_row,
                                            c,
                                        ));

                                        _last_letter = Some(std::time::SystemTime::now());

                                        doc.cursor_col += 1;

                                        if config.auto_close.contains_key(&c) {
                                            doc.dirty += 1;
                                            doc.rows[doc.cursor_row]
                                                .insert_char(doc.cursor_col, config.auto_close[&c]);
                                            doc.add_diff(EditDiff::InsertChar(
                                                doc.cursor_col,
                                                doc.cursor_row,
                                                config.auto_close[&c],
                                            ));

                                            doc.to_auto_close = true;
                                        }
                                    }
                                    KeyCode::Esc => {
                                        if self.show_global_prompt().is_err() {
                                            break;
                                        } 
                                    },
                                    KeyCode::F(1) => break,
                                    KeyCode::Backspace => {
                                        let doc = self.docs[doc_index].as_mut_text_doc();
                                        if doc.selection.is_some() {
                                            doc.selection.as_mut().unwrap().normalize();

                                            if let Some(selection) = doc.selection {
                                                doc.cursor_col = selection.start_col;
                                                doc.cursor_row = selection.start_row;

                                                if selection.start_row == selection.end_row {
                                                    let mut diffs = Vec::new();

                                                    for _ in selection.start_col..selection.end_col
                                                    {
                                                        let c = doc.rows[selection.start_row]
                                                            .remove_at(selection.start_col);
                                                        diffs.push(EditDiff::DeleteChar(
                                                            selection.start_col + 1,
                                                            selection.start_row,
                                                            c,
                                                            false,
                                                        ));
                                                    }

                                                    doc.add_diff(EditDiff::Compound(diffs));
                                                } else {
                                                    let mut ly = selection.start_row;
                                                    let mut line_index = selection.start_row;

                                                    let mut diffs = Vec::new();

                                                    while ly <= selection.end_row {
                                                        if ly == selection.start_row {
                                                            if selection.start_col == 0 {
                                                                let line =
                                                                    doc.rows.remove(line_index);
                                                                diffs.push(EditDiff::DeleteLine(
                                                                    line_index,
                                                                    line.buf,
                                                                    LineDeleteMode::WholeLine,
                                                                ));
                                                            } else {
                                                                let len =
                                                                    doc.rows[line_index].len();
                                                                for _ in selection.start_col..len {
                                                                    let c = doc.rows[line_index]
                                                                        .remove_at(
                                                                            selection.start_col,
                                                                        );
                                                                    diffs.push(
                                                                        EditDiff::DeleteChar(
                                                                            selection.start_col + 1,
                                                                            selection.start_row,
                                                                            c,
                                                                            false,
                                                                        ),
                                                                    );
                                                                }

                                                                line_index += 1;
                                                            }
                                                        } else if ly == selection.end_row {
                                                            let len = doc.rows[line_index].len();
                                                            if selection.end_col == len {
                                                                let line =
                                                                    doc.rows.remove(line_index);
                                                                diffs.push(EditDiff::DeleteLine(
                                                                    line_index,
                                                                    line.buf,
                                                                    LineDeleteMode::WholeLine,
                                                                ));
                                                            } else {
                                                                for _ in 0..selection.end_col {
                                                                    let c = doc.rows[line_index]
                                                                        .remove_at(0);
                                                                    diffs.push(
                                                                        EditDiff::DeleteChar(
                                                                            1,
                                                                            selection.end_row,
                                                                            c,
                                                                            false,
                                                                        ),
                                                                    );
                                                                }

                                                                line_index += 1;
                                                            }
                                                        } else {
                                                            let line = doc.rows.remove(line_index);
                                                            diffs.push(EditDiff::DeleteLine(
                                                                line_index,
                                                                line.buf,
                                                                LineDeleteMode::WholeLine,
                                                            ));
                                                        }

                                                        ly += 1;
                                                    }
                                                    doc.add_diff(EditDiff::Compound(diffs));
                                                }
                                            }

                                            continue;
                                        }

                                        doc.dirty += 1;
                                        if doc.rows[doc.cursor_row].len() != 0 {
                                            if doc.cursor_col == doc.rows[doc.cursor_row].len() {
                                                let c = doc.rows[doc.cursor_row].buf.pop().unwrap();

                                                doc.add_diff(EditDiff::DeleteChar(
                                                    doc.cursor_col,
                                                    doc.cursor_row,
                                                    c,
                                                    true,
                                                ));

                                                doc.cursor_col -= 1;
                                            } else {
                                                if doc.cursor_col == 0 {
                                                    if doc.cursor_row != 0 {
                                                        doc.cursor_row -= 1;
                                                        doc.cursor_col =
                                                            doc.rows[doc.cursor_row].len();
                                                        if doc.cursor_col == 0 {
                                                            doc.rows.remove(doc.cursor_row);

                                                            doc.add_diff(EditDiff::DeleteLine(
                                                                doc.cursor_row,
                                                                String::new(),
                                                                LineDeleteMode::WholeLine,
                                                            ));
                                                        } else {
                                                            let line = doc.rows[doc.cursor_row + 1]
                                                                .buf
                                                                .clone();
                                                            doc.rows.remove(doc.cursor_row + 1);
                                                            doc.add_diff(EditDiff::DeleteLine(
                                                                doc.cursor_row + 1,
                                                                line.clone(),
                                                                LineDeleteMode::Joined,
                                                            ));
                                                            doc.rows[doc.cursor_row]
                                                                .buf
                                                                .push_str(&line);
                                                        }
                                                    }
                                                } else {
                                                    let c = doc.rows[doc.cursor_row]
                                                        .buf
                                                        .chars()
                                                        .nth(doc.cursor_col - 1)
                                                        .unwrap();
                                                    doc.add_diff(EditDiff::DeleteChar(
                                                        doc.cursor_col - 1,
                                                        doc.cursor_row,
                                                        c,
                                                        true,
                                                    ));

                                                    doc.rows[doc.cursor_row]
                                                        .remove_at(doc.cursor_col - 1);
                                                    doc.cursor_col -= 1;
                                                }
                                            }
                                        } else {
                                            if doc.cursor_row != 0 {
                                                doc.add_diff(EditDiff::DeleteLine(
                                                    doc.cursor_row,
                                                    String::new(),
                                                    LineDeleteMode::WholeLine,
                                                ));

                                                doc.rows.remove(doc.cursor_row);
                                                doc.cursor_row -= 1;
                                                doc.cursor_col = doc.rows[doc.cursor_row].len();
                                            }
                                        }
                                    }
                                    KeyCode::Delete => {
                                        let doc = self.docs[doc_index].as_mut_text_doc();
                                        if doc.selection.is_some() {
                                            doc.selection.as_mut().unwrap().normalize();

                                            if let Some(selection) = doc.selection {
                                                doc.cursor_col = selection.start_col;
                                                doc.cursor_row = selection.start_row;

                                                if selection.start_row == selection.end_row {
                                                    let mut diffs = Vec::new();

                                                    for _ in selection.start_col..selection.end_col
                                                    {
                                                        let c = doc.rows[selection.start_row]
                                                            .remove_at(selection.start_col);
                                                        diffs.push(EditDiff::DeleteChar(
                                                            selection.start_col + 1,
                                                            selection.start_row,
                                                            c,
                                                            false,
                                                        ));
                                                    }

                                                    doc.add_diff(EditDiff::Compound(diffs));
                                                } else {
                                                    let mut ly = selection.start_row;
                                                    let mut line_index = selection.start_row;

                                                    let mut diffs = Vec::new();

                                                    while ly <= selection.end_row {
                                                        if ly == selection.start_row {
                                                            if selection.start_col == 0 {
                                                                let line =
                                                                    doc.rows.remove(line_index);
                                                                diffs.push(EditDiff::DeleteLine(
                                                                    line_index,
                                                                    line.buf,
                                                                    LineDeleteMode::WholeLine,
                                                                ));
                                                            } else {
                                                                let len =
                                                                    doc.rows[line_index].len();
                                                                for _ in selection.start_col..len {
                                                                    let c = doc.rows[line_index]
                                                                        .remove_at(
                                                                            selection.start_col,
                                                                        );
                                                                    diffs.push(
                                                                        EditDiff::DeleteChar(
                                                                            selection.start_col + 1,
                                                                            selection.start_row,
                                                                            c,
                                                                            false,
                                                                        ),
                                                                    );
                                                                }

                                                                line_index += 1;
                                                            }
                                                        } else if ly == selection.end_row {
                                                            let len = doc.rows[line_index].len();
                                                            if selection.end_col == len {
                                                                let line =
                                                                    doc.rows.remove(line_index);
                                                                diffs.push(EditDiff::DeleteLine(
                                                                    line_index,
                                                                    line.buf,
                                                                    LineDeleteMode::WholeLine,
                                                                ));
                                                            } else {
                                                                for _ in 0..selection.end_col {
                                                                    let c = doc.rows[line_index]
                                                                        .remove_at(0);
                                                                    diffs.push(
                                                                        EditDiff::DeleteChar(
                                                                            1,
                                                                            selection.end_row,
                                                                            c,
                                                                            false,
                                                                        ),
                                                                    );
                                                                }

                                                                line_index += 1;
                                                            }
                                                        } else {
                                                            let line = doc.rows.remove(line_index);
                                                            diffs.push(EditDiff::DeleteLine(
                                                                line_index,
                                                                line.buf,
                                                                LineDeleteMode::WholeLine,
                                                            ));
                                                        }

                                                        ly += 1;
                                                    }
                                                    doc.add_diff(EditDiff::Compound(diffs));
                                                }
                                            }

                                            continue;
                                        }

                                        doc.dirty += 1;
                                        if doc.rows[doc.cursor_row].len() != 0 {
                                            if doc.cursor_col == doc.rows[doc.cursor_row].len() {
                                                if doc.cursor_row + 1 < doc.rows.len() {
                                                    let next_line =
                                                        doc.rows[doc.cursor_row + 1].buf.clone();
                                                    doc.rows[doc.cursor_row]
                                                        .buf
                                                        .push_str(&next_line);

                                                    doc.rows.remove(doc.cursor_row + 1);
                                                }
                                            } else {
                                                let c = doc.rows[doc.cursor_row]
                                                    .buf
                                                    .chars()
                                                    .nth(doc.cursor_col)
                                                    .unwrap();

                                                doc.add_diff(EditDiff::DeleteChar(
                                                    doc.cursor_col + 1,
                                                    doc.cursor_row,
                                                    c,
                                                    false,
                                                ));
                                                doc.rows[doc.cursor_row].remove_at(doc.cursor_col);
                                            }
                                        } else {
                                            if doc.cursor_row + 1 != doc.rows.len() {
                                                doc.rows.remove(doc.cursor_row);
                                                doc.add_diff(EditDiff::DeleteLine(
                                                    doc.cursor_row,
                                                    String::new(),
                                                    LineDeleteMode::WholeLine,
                                                ));
                                            }
                                        }
                                    }
                                    KeyCode::Tab => {
                                        if k.modifiers.contains(KeyModifiers::SHIFT) {
                                            if self.open_doc.unwrap() + 1 < num_docs {
                                                self.open_doc = Some(self.open_doc.unwrap() + 1);
                                            } else {
                                                self.open_doc = Some(0);
                                            }
                                        }
                                        else {
                                            let doc = self.docs[doc_index].as_mut_text_doc();
                                            doc.dirty += 1;
                                            let mut diff_vec = Vec::new();
                                            for c in unescape(&config.tab_str).unwrap().chars() {
                                                doc.rows[doc.cursor_row].insert_char(doc.cursor_col, c);
                                                diff_vec.push(EditDiff::InsertChar(
                                                    doc.cursor_col,
                                                    doc.cursor_row,
                                                    c,
                                                ));

                                                doc.cursor_col += 1;
                                            }
                                            doc.add_diff(EditDiff::Compound(diff_vec));
                                        }
                                    }
                                    KeyCode::Enter => {
                                        let doc = self.docs[doc_index].as_mut_text_doc();
                                        doc.dirty += 1;
                                        if doc.rows[doc.cursor_row].len() == 0 {
                                            doc.rows.insert(doc.cursor_row + 1, Row::empty());
                                            doc.add_diff(EditDiff::NewLine(doc.cursor_row + 1));

                                            doc.cursor_row += 1;
                                            doc.cursor_col = 0;
                                        } else if doc.cursor_col == 0 {
                                            // doc.undo.push(wrap!(Action::AddRow(doc.cursor_row)));

                                            doc.rows.insert(doc.cursor_row, Row::empty());
                                            doc.add_diff(EditDiff::NewLine(doc.cursor_row));

                                            doc.cursor_row += 1;
                                            doc.cursor_col = 0;
                                        } else if doc.cursor_col == doc.rows[doc.cursor_row].len() {
                                            // doc.undo.push(wrap!(Action::AddRow(doc.cursor_row + 1)));

                                            doc.rows.insert(doc.cursor_row + 1, Row::empty());
                                            doc.add_diff(EditDiff::NewLine(doc.cursor_row + 1));

                                            doc.cursor_row += 1;
                                            doc.cursor_col = 0;
                                        } else {
                                            let (left, right) =
                                                doc.rows[doc.cursor_row].split_at(doc.cursor_col);
                                            doc.rows[doc.cursor_row] = Row::from_string(left);
                                            if doc.cursor_row + 1 <= doc.rows.len() {
                                                // doc.undo.push(wrap!(Action::AddRow(doc.cursor_row + 1)));

                                                doc.rows.insert(doc.cursor_row + 1, Row::empty());
                                            } else if doc.cursor_row + 1 > doc.rows.len() {
                                                // doc.undo.push(wrap!(Action::AddRow(doc.rows.len())));

                                                doc.rows.push(Row::empty());
                                            }
                                            doc.add_diff(EditDiff::SplitLine(
                                                doc.cursor_col,
                                                doc.cursor_row,
                                            ));

                                            doc.rows[doc.cursor_row + 1] = Row::from_string(right);

                                            doc.cursor_row += 1;
                                            doc.cursor_col = 0;
                                        }
                                    }
                                    KeyCode::Up => {
                                        let doc = self.docs[doc_index].as_mut_text_doc();

                                        let cursor_row = doc.cursor_row;
                                        let cursor_col = doc.cursor_col;

                                        if doc.rows[doc.cursor_row].line_width(&config) > width
                                            && doc.cursor_col > width
                                        {
                                            doc.cursor_col -= width;
                                        } else {
                                            if doc.cursor_row != 0 {
                                                doc.cursor_row -= 1;
                                                if doc.cursor_col > doc.rows[doc.cursor_row].len() {
                                                    doc.cursor_col = doc.rows[doc.cursor_row].len();
                                                }
                                            }
                                        }

                                        if k.modifiers.contains(KeyModifiers::SHIFT) {
                                            if doc.selection.is_none() {
                                                doc.selection = Some(Selection::new(
                                                    cursor_row,
                                                    cursor_col,
                                                    doc.cursor_row,
                                                    doc.cursor_col,
                                                ));
                                            } else {
                                                let selection = doc.selection.as_mut().unwrap();

                                                selection.end_row = doc.cursor_row;
                                                selection.end_col = doc.cursor_col;
                                            }

                                            self.undergoing_selection = true;
                                        }
                                    }
                                    KeyCode::Down => {
                                        let doc = self.docs[doc_index].as_mut_text_doc();

                                        let cursor_row = doc.cursor_row;
                                        let cursor_col = doc.cursor_col;

                                        if doc.rows[doc.cursor_row].line_width(&config) > width {
                                            doc.cursor_col += width;
                                            if doc.cursor_col > doc.rows[doc.cursor_row].len() {
                                                doc.cursor_col = doc.cursor_col % width;
                                                doc.cursor_row += 1;
                                            }
                                        } else {
                                            if doc.cursor_row + 1 != doc.rows.len() {
                                                doc.cursor_row += 1;
                                                if doc.cursor_col > doc.rows[doc.cursor_row].len() {
                                                    doc.cursor_col = doc.rows[doc.cursor_row].len();
                                                }
                                            }
                                        }

                                        if k.modifiers.contains(KeyModifiers::SHIFT) {
                                            if doc.selection.is_none() {
                                                doc.selection = Some(Selection::new(
                                                    cursor_row,
                                                    cursor_col,
                                                    doc.cursor_row,
                                                    doc.cursor_col,
                                                ));
                                            } else {
                                                let selection = doc.selection.as_mut().unwrap();

                                                selection.end_row = doc.cursor_row;
                                                selection.end_col = doc.cursor_col;
                                            }
                                            self.undergoing_selection = true;
                                        }
                                    }
                                    KeyCode::Left => {
                                        let doc = self.docs[doc_index].as_mut_text_doc();

                                        let cursor_row = doc.cursor_row;
                                        let cursor_col = doc.cursor_col;

                                        if k.modifiers.contains(KeyModifiers::CONTROL) {
                                            if doc.rows[doc.cursor_row].tokens.len() != 0
                                                && doc.cursor_col != 0
                                            {
                                                let mut last_end = 0;

                                                for token in &doc.rows[doc.cursor_row].tokens {
                                                    if token.end() > doc.cursor_col - 1 {
                                                        doc.cursor_col = last_end;
                                                        break;
                                                    } else {
                                                        last_end = token.end();
                                                    }
                                                }
                                            }

                                            if k.modifiers.contains(KeyModifiers::SHIFT) {
                                                if doc.selection.is_none() {
                                                    doc.selection = Some(Selection::new(
                                                        cursor_row,
                                                        cursor_col,
                                                        doc.cursor_row,
                                                        doc.cursor_col,
                                                    ));
                                                } else {
                                                    let selection = doc.selection.as_mut().unwrap();
    
                                                    selection.end_row = doc.cursor_row;
                                                    selection.end_col = doc.cursor_col;
                                                }
                                                self.undergoing_selection = true;
                                            }
                                        } else {
                                            if doc.cursor_col != 0 {
                                                doc.cursor_col -= 1;
                                            } else {
                                                if doc.cursor_row != 0 {
                                                    doc.cursor_row -= 1;
                                                    doc.cursor_col = doc.rows[doc.cursor_row].len();
                                                }
                                            }
                                        }

                                        if k.modifiers.contains(KeyModifiers::SHIFT) {
                                            if doc.selection.is_none() {
                                                doc.selection = Some(Selection::new(
                                                    cursor_row,
                                                    cursor_col,
                                                    doc.cursor_row,
                                                    doc.cursor_col,
                                                ));
                                            } else {
                                                let selection = doc.selection.as_mut().unwrap();

                                                selection.end_row = doc.cursor_row;
                                                selection.end_col = doc.cursor_col;
                                            }
                                            self.undergoing_selection = true;
                                        }
                                    }
                                    KeyCode::Right => {
                                        let doc = self.docs[doc_index].as_mut_text_doc();

                                        let cursor_row = doc.cursor_row;
                                        let cursor_col = doc.cursor_col;

                                        if k.modifiers.contains(KeyModifiers::CONTROL) {
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

                                            if k.modifiers.contains(KeyModifiers::SHIFT) {
                                                if doc.selection.is_none() {
                                                    doc.selection = Some(Selection::new(
                                                        cursor_row,
                                                        cursor_col,
                                                        doc.cursor_row,
                                                        doc.cursor_col,
                                                    ));
                                                } else {
                                                    let selection = doc.selection.as_mut().unwrap();
    
                                                    selection.end_row = doc.cursor_row;
                                                    selection.end_col = doc.cursor_col;
                                                }
                                                self.undergoing_selection = true;
                                            }
                                        } else {
                                            doc.cursor_col += 1;
                                            if doc.cursor_col > doc.rows[doc.cursor_row].len() {
                                                if doc.cursor_row + 1 != doc.rows.len() {
                                                    doc.cursor_col = 0;
                                                    doc.cursor_row += 1;
                                                } else {
                                                    doc.cursor_col = doc.rows[doc.cursor_row].len();
                                                }
                                            }
                                        }

                                        if k.modifiers.contains(KeyModifiers::SHIFT) {
                                            if doc.selection.is_none() {
                                                doc.selection = Some(Selection::new(
                                                    cursor_row,
                                                    cursor_col,
                                                    doc.cursor_row,
                                                    doc.cursor_col,
                                                ));
                                            } else {
                                                let selection = doc.selection.as_mut().unwrap();

                                                selection.end_row = doc.cursor_row;
                                                selection.end_col = doc.cursor_col;
                                            }
                                            self.undergoing_selection = true;
                                        }
                                    }
                                    KeyCode::Home => {
                                        self.docs[doc_index].as_mut_text_doc().cursor_col = 0;
                                    }
                                    KeyCode::End => {
                                        let row = self.docs[doc_index].as_mut_text_doc().cursor_row;
                                        self.docs[doc_index].as_mut_text_doc().cursor_col =
                                            self.docs[doc_index].as_mut_text_doc().rows[row].len();
                                    }
                                    _ => {}
                                };
                            }
                            Resize(w, h) => {
                                self.resize = Some((w as usize, h as usize));

                                self.clear = true;
                            }
                            Mouse(e) => {
                                let doc = self.docs[doc_index].as_mut_text_doc();

                                self.redraw = false;
                                self.mouse_event = true;
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
                                            while !doc.rows[doc.cursor_row]
                                                .buf
                                                .is_char_boundary(doc.cursor_col)
                                            {
                                                doc.cursor_col -= 1;
                                            }
                                        } else {
                                            doc.cursor_col = doc.rows[doc.cursor_row].len();
                                        }
                                        Editor::position_cursor(
                                            doc.cursor_row,
                                            doc.cursor_col,
                                            &doc.rows,
                                            width,
                                            doc.line_start,
                                            &config,
                                        );
                                    } else {
                                        for (i, doc_index) in
                                            self.docs_mouse_cache.iter().enumerate()
                                        {
                                            if e.column > doc_index.0 as u16
                                                && e.column < doc_index.1 as u16
                                            {
                                                if doc_index.1 as u16 - e.column == 1 {
                                                    if self.docs.len() > 1 {
                                                        if i != 0 {
                                                            self.open_doc = Some(i - 1);
                                                        } else {
                                                            self.open_doc = Some(1);
                                                        }
                                                        self.docs.remove(i);
                                                        self.redraw = true;
                                                        self.mouse_event = false;
                                                        continue 'editor;
                                                    } else if self.docs.len() > 0 {
                                                        self.open_doc = None;
                                                        self.docs.remove(i);
                                                        self.redraw = true;
                                                        self.mouse_event = false;
                                                        continue 'editor;
                                                    } else {
                                                        // Technically unreachable I think
                                                        self.open_doc = None;
                                                        self.redraw = true;
                                                        self.mouse_event = false;
                                                        continue 'editor;
                                                    }
                                                } else {
                                                    self.open_doc = Some(i);
                                                    self.redraw = true;
                                                    self.mouse_event = false;
                                                    continue 'editor;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    let doc = self.docs[doc_index].as_mut_text_doc();
                    let actual_rows = height as i32 - 3;
                    let diff = doc.visual_rows_to(width, doc.cursor_row, &config) as i32
                        - doc.visual_rows_to(width, doc.line_start, &config) as i32;
                    if diff >= actual_rows {
                        doc.line_start += 1;
                        self.redraw = true;
                    } else if diff < 0 && doc.line_start != 0 {
                        doc.line_start -= 1;
                        self.redraw = true;
                    }
                } else if self.docs[doc_index].is_binary_doc() {
                    if self.redraw {
                        print!(
                            "{}{}",
                            crossterm::style::SetBackgroundColor(Color::from(
                                self.config.theme.background_color
                            )),
                            crossterm::style::SetForegroundColor(Color::from(
                                self.config.theme.foreground_color
                            ))
                        );

                        let mut drawing_row = 0;
                        let mut processing_row = self.docs[doc_index].as_bin_doc().line_start;
                        let col = self.docs[doc_index].as_bin_doc().cursor
                            % BinaryDocument::bytes_per_row();
                        let row = self.docs[doc_index].as_bin_doc().cursor
                            / BinaryDocument::bytes_per_row();

                        let bytes_per_row = BinaryDocument::bytes_per_row();

                        let mut cursor = self.docs[doc_index].as_bin_doc().line_start
                            * BinaryDocument::bytes_per_row();

                        while drawing_row < height - 2 {
                            if drawing_row == 0 {
                                println!();
                            } else if drawing_row == 1 {
                                let mut title_string = String::with_capacity(width);

                                title_string.push_str(" Offset   ");

                                for i in 0..bytes_per_row {
                                    title_string.push_str(&format!(
                                        "{:02X}{}",
                                        i,
                                        if i + 1 < bytes_per_row { " " } else { "" }
                                    ));
                                }

                                println!(
                                    "{}{}",
                                    title_string,
                                    " ".repeat(width - title_string.width())
                                );
                            } else if drawing_row == 2 {
                                println!("{}", " ".repeat(width));
                            } else if drawing_row == height - 3 {
                                println!("{}", " ".repeat(width));
                            } else if processing_row
                                <= self.docs[doc_index].as_bin_doc().data.len()
                                    / BinaryDocument::bytes_per_row()
                            {
                                let mut line = String::with_capacity(width);
                                let mut line_width = 0;
                                let mut str_repr_width = 0;

                                line.push_str(&format!("{:08X}  ", processing_row * bytes_per_row));
                                line_width += 10;

                                let mut str_repr = String::new();

                                let mut last_i = -1;
                                let high_nibble = self.docs[doc_index].as_bin_doc().high_nibble;

                                for offset in 0..BinaryDocument::bytes_per_row() {
                                    if cursor + offset
                                        >= self.docs[doc_index].as_bin_doc().data.len()
                                    {
                                        break;
                                    }

                                    let b = self.docs[doc_index].as_bin_doc().data[cursor + offset];

                                    let high_nibble_c =
                                        format!("{:02X}", b).chars().nth(0).unwrap();
                                    let low_nibble_c = format!("{:02X}", b).chars().nth(1).unwrap();

                                    if offset == col && row == processing_row {
                                        str_repr.push_str(&BinaryDocument::cursor_style());

                                        if high_nibble {
                                            line.push_str(&format!(
                                                "{}{}{}{}{}{}{}",
                                                BinaryDocument::cursor_style(),
                                                high_nibble_c,
                                                crossterm::style::SetAttribute(
                                                    crossterm::style::Attribute::Reset
                                                ),
                                                crossterm::style::SetBackgroundColor(Color::from(
                                                    self.config.theme.background_color
                                                )),
                                                crossterm::style::SetForegroundColor(Color::from(
                                                    self.config.theme.foreground_color
                                                )),
                                                low_nibble_c,
                                                if offset + 1 < bytes_per_row { " " } else { "" }
                                            ));
                                        } else {
                                            line.push_str(&format!(
                                                "{}{}{}{}{}{}{}",
                                                high_nibble_c,
                                                BinaryDocument::cursor_style(),
                                                low_nibble_c,
                                                crossterm::style::SetAttribute(
                                                    crossterm::style::Attribute::Reset
                                                ),
                                                crossterm::style::SetBackgroundColor(Color::from(
                                                    self.config.theme.background_color
                                                )),
                                                crossterm::style::SetForegroundColor(Color::from(
                                                    self.config.theme.foreground_color
                                                )),
                                                if offset + 1 < bytes_per_row { " " } else { "" }
                                            ));
                                        }
                                    } else {
                                        line.push_str(&format!(
                                            "{:02X}{}",
                                            b,
                                            if offset + 1 < bytes_per_row { " " } else { "" }
                                        ));
                                    }
                                    line_width +=
                                        2 + if offset + 1 < bytes_per_row { 1 } else { 0 };
                                    if b <= 127 {
                                        let c = b as char;

                                        if !c.is_control() {
                                            str_repr.push(c);
                                        } else {
                                            str_repr.push_str(&format!(
                                                "{}{}{}{}",
                                                crossterm::style::SetForegroundColor(Color::Red),
                                                '?',
                                                crossterm::style::SetBackgroundColor(Color::from(
                                                    self.config.theme.background_color
                                                )),
                                                crossterm::style::SetForegroundColor(Color::from(
                                                    self.config.theme.foreground_color
                                                ))
                                            ));
                                        }
                                        str_repr_width += 1;
                                    } else {
                                        str_repr.push('?');
                                        str_repr_width += 1;
                                    }

                                    if offset == col && row == processing_row {
                                        str_repr.push_str(&format!(
                                            "{}{}{}",
                                            crossterm::style::SetAttribute(
                                                crossterm::style::Attribute::Reset
                                            ),
                                            crossterm::style::SetBackgroundColor(Color::from(
                                                self.config.theme.background_color
                                            )),
                                            crossterm::style::SetForegroundColor(Color::from(
                                                self.config.theme.foreground_color
                                            ))
                                        ));
                                    }

                                    last_i = offset as i32;
                                }

                                cursor += BinaryDocument::bytes_per_row();

                                for i in last_i + 1..bytes_per_row as i32 {
                                    line.push_str(&format!(
                                        "..{}",
                                        if i + 1 < bytes_per_row as i32 {
                                            " "
                                        } else {
                                            ""
                                        }
                                    ));
                                    line_width +=
                                        2 + if i + 1 < bytes_per_row as i32 { 1 } else { 0 };
                                }

                                line.push_str(&format!(" | {}", str_repr));
                                line_width += 3 + str_repr_width;

                                println!("{}{}", line, " ".repeat(width - line_width));

                                processing_row += 1;
                            } else {
                                println!("{}", " ".repeat(width - 1));
                            }

                            drawing_row += 1;
                        }
                        self.draw_tabs();
                        self.write_status_bar(None);

                        std::io::stdout().flush()?;
                    }

                    let event = read().unwrap();
                    let mut process_event = true;

                    if let crossterm::event::Event::Key(k) = event {
                        for (name, keybound_event) in &self.config.keybinds {
                            if keybound_event.equals(&k) {
                                match &name[..] {
                                    "close_file" => close_file!(),
                                    "open_file" => open_file!(),
                                    "save_file" => save_file!(),
                                    "new_file" => new_file!(),
                                    _ => {}
                                }
                                process_event = false;
                            }
                        }
                    }

                    if !process_event {
                        continue;
                    }

                    match event {
                        Key(k) => match k.code {
                            KeyCode::Esc => break,
                            KeyCode::Right => {
                                if k.modifiers.contains(KeyModifiers::CONTROL)
                                    && k.modifiers.contains(KeyModifiers::SHIFT)
                                {
                                    if self.open_doc.unwrap() + 1 < self.docs.len() {
                                        self.open_doc = Some(self.open_doc.unwrap() + 1);
                                    } else {
                                        self.open_doc = Some(0);
                                    }
                                    continue;
                                }

                                if self.docs[doc_index].as_bin_doc().high_nibble {
                                    self.docs[doc_index].as_bin_doc().high_nibble = false;
                                } else {
                                    if self.docs[doc_index].as_bin_doc().cursor + 1
                                        < self.docs[doc_index].as_bin_doc().data.len()
                                    {
                                        self.docs[doc_index].as_bin_doc().cursor += 1;
                                        self.docs[doc_index].as_bin_doc().high_nibble = true;
                                    }
                                }
                            }
                            KeyCode::Left => {
                                if k.modifiers.contains(KeyModifiers::CONTROL)
                                    && k.modifiers.contains(KeyModifiers::SHIFT)
                                {
                                    if self.open_doc.unwrap() != 0 {
                                        self.open_doc = Some(self.open_doc.unwrap() - 1);
                                    } else if self.open_doc.unwrap() == 0 && self.docs.len() == 1 {
                                    } else {
                                        self.open_doc = Some(self.docs.len());
                                    }
                                    continue;
                                }

                                if !self.docs[doc_index].as_bin_doc().high_nibble {
                                    self.docs[doc_index].as_bin_doc().high_nibble = true;
                                } else {
                                    if self.docs[doc_index].as_bin_doc().cursor != 0 {
                                        self.docs[doc_index].as_bin_doc().cursor -= 1;
                                        self.docs[doc_index].as_bin_doc().high_nibble = false;
                                    }
                                }
                            }
                            KeyCode::Up => {
                                if self.docs[doc_index].as_bin_doc().cursor
                                    >= BinaryDocument::bytes_per_row()
                                {
                                    self.docs[doc_index].as_bin_doc().cursor -=
                                        BinaryDocument::bytes_per_row();
                                }
                            }
                            KeyCode::Down => {
                                if self.docs[doc_index].as_bin_doc().cursor
                                    + BinaryDocument::bytes_per_row()
                                    < self.docs[doc_index].as_bin_doc().data.len()
                                {
                                    self.docs[doc_index].as_bin_doc().cursor +=
                                        BinaryDocument::bytes_per_row();
                                }
                            }
                            KeyCode::Char(c) => {
                                let allowed_chars = "1234567890abcdef";

                                let high_nibble = self.docs[doc_index].as_bin_doc().high_nibble;
                                let cursor = self.docs[doc_index].as_bin_doc().cursor;

                                if allowed_chars.contains(c) {
                                    let val = c.to_digit(16).unwrap() as u8;
                                    let mut prev = self.docs[doc_index].as_bin_doc().data[cursor];

                                    if high_nibble {
                                        prev &= 0b00001111;
                                        prev |= val << 4;
                                    } else {
                                        prev &= 0b11110000;
                                        prev |= val;
                                    }

                                    self.docs[doc_index].as_bin_doc().dirty += 1;

                                    self.docs[doc_index].as_bin_doc().data[cursor] = prev;

                                    if high_nibble {
                                        self.docs[doc_index].as_bin_doc().high_nibble = false;
                                    } else {
                                        if self.docs[doc_index].as_bin_doc().cursor + 1
                                            < self.docs[doc_index].as_bin_doc().data.len()
                                        {
                                            self.docs[doc_index].as_bin_doc().high_nibble = true;
                                            self.docs[doc_index].as_bin_doc().cursor += 1;
                                        }
                                    }
                                } else if c == 'i' {
                                    self.docs[doc_index].as_bin_doc().data.insert(cursor, 0);
                                } else if c == 'I' {
                                    if cursor + 1 < self.docs[doc_index].as_bin_doc().data.len() {
                                        self.docs[doc_index]
                                            .as_bin_doc()
                                            .data
                                            .insert(cursor + 1, 0);
                                    } else {
                                        self.docs[doc_index].as_bin_doc().data.push(0);
                                    }
                                }
                            }
                            _ => {}
                        },
                        _ => {}
                    }

                    let row =
                        self.docs[doc_index].as_bin_doc().cursor / BinaryDocument::bytes_per_row();
                    let line_start = self.docs[doc_index].as_bin_doc().line_start;
                    if row < line_start {
                        self.docs[doc_index].as_bin_doc().line_start = row;
                    } else if row - line_start >= height - 6 {
                        self.docs[doc_index].as_bin_doc().line_start += 1;
                    }
                }
            } else {
                self.main_screen()?;
            }
        }

        let mut path = dirs::config_dir().expect("Application requires a config directory");
        path.push("kelp");
        path.push("config.json");

        let mut config_file = std::fs::File::create(path)?;

        config_file.write_all(serde_json::to_string_pretty(&self.config)?.as_bytes())?;

        print!("\x1B[?1000;1006;1015l");
        std::io::stdout().flush().unwrap();
        crossterm::terminal::disable_raw_mode()?;

        std::io::stdout().execute(crossterm::terminal::LeaveAlternateScreen)?;

        Ok(())
    }

    pub fn main_screen(&mut self) -> Result<(), Box<dyn Error>> {
        let mut redraw = true;

        print!(
            "{}{}",
            crossterm::style::SetBackgroundColor(Color::from(self.config.theme.background_color)),
            crossterm::style::SetForegroundColor(Color::from(self.config.theme.foreground_color))
        );

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
                            self.add_doc(TextDocument::new(filename));
                            return Ok(());
                        } else {
                            self.write_status_bar(Some(format!(
                                "File {} already exists!",
                                filename
                            )))
                        }
                    } else {
                        redraw = true;
                    }
                } else if k.modifiers.contains(KeyModifiers::CONTROL)
                    && k.code == KeyCode::Char('o')
                {
                    if let Ok(filename) = self.read_new_filename(None) {
                        if filename.starts_with("bin:") {
                            let mut chars = filename.chars();
                            chars.next();
                            chars.next();
                            chars.next();
                            chars.next();
                            let filename = chars.as_str().to_owned();
                            let d = BinaryDocument::load(filename)?;
                            self.open_doc = Some(self.docs.len());
                            self.add_bin_doc(d);

                            return Ok(());
                        } else {
                            match TextDocument::load(filename) {
                                Ok(mut doc) => {
                                    let config =
                                        if self.config.languages.contains_key(&doc.extension()) {
                                            &self.config.languages[&doc.extension()]
                                        } else {
                                            &self.config.languages[&"*".to_owned()]
                                        };
                                    self.open_doc = Some(self.docs.len());
                                    doc.tokenize(0, self.height(), config);

                                    self.add_doc(doc);
                                    return Ok(());
                                }
                                Err(filename) => self.write_status_bar(Some(format!(
                                    "File {} not found!",
                                    filename
                                ))),
                            }
                        }
                    } else {
                        redraw = true;
                    }
                } else if k.modifiers.contains(KeyModifiers::CONTROL)
                    && k.code == KeyCode::Char('g')
                {
                    if let Ok(mut command) = self.read_new_filename(None) {
                        if command.starts_with("cd") {
                            command.drain(..2);
                            let mut dir = std::env::current_dir().unwrap();
                            if command.trim() == ".." {
                                if dir.pop() {
                                    std::env::set_current_dir(dir)?;
                                    redraw = true;
                                }
                            } else {
                                dir.push(command.trim());
                                if dir.exists() {
                                    std::env::set_current_dir(dir)?;
                                    redraw = true;
                                }
                            }
                        }
                    }
                } else if k.code == KeyCode::Esc {
                    std::io::stdout()
                        .execute(crossterm::event::DisableMouseCapture)?
                        .execute(crossterm::terminal::Clear(
                            crossterm::terminal::ClearType::All,
                        ))?
                        .execute(crossterm::cursor::MoveTo(0, 0))?;
                    print!("\x1B[?1000;1006;1015l");
                    std::io::stdout().flush().unwrap();
                    crossterm::terminal::disable_raw_mode()?;

                    std::io::stdout().execute(crossterm::terminal::LeaveAlternateScreen)?;

                    return Err("Close".into());
                }
            } else if let Resize(w, h) = event {
                self.resize(w as usize, h as usize);
                redraw = true;
            }
        }
    }

    pub fn write_status_bar(&self, mut extra_info: Option<String>) {
        if extra_info.is_none() {
            extra_info =
                Some("Ctrl+G: Command | Ctrl+N: New file | Ctrl+O: Open file  ".to_owned());
        }

        let (width, height) = (self.width(), self.height());
        // Status bar
        let filename;
        let index;
        if self.open_doc.is_none() {
            filename = "No document open".to_owned();
            index = 0;
        } else {
            filename = self.docs[self.open_doc.unwrap()].filename().clone();
            index = self.open_doc.unwrap() + 1;
        }

        let status_str = format!("[{}] - Doc {} of {}", filename, index, self.docs.len());

        let dir = std::env::current_dir().unwrap_or_default();
        let mut dir = format!(
            "{} [/{}]",
            extra_info.unwrap_or("".to_owned()),
            dir.iter()
                .last()
                .unwrap()
                .to_os_string()
                .into_string()
                .unwrap()
        );

        if width < status_str.len() + dir.len() + 8 {
            dir.drain(
                ..dir
                    .char_indices()
                    .nth(status_str.len() + dir.len() + 8 - width)
                    .unwrap()
                    .0,
            );
            dir.insert_str(0, "...");
        }

        print!(
            "{}{}{}{}{}{}{}",
            crossterm::cursor::MoveTo(0, height as u16 - 2),
            crossterm::style::SetBackgroundColor(Color::from(self.config.theme.foreground_color)),
            crossterm::style::SetForegroundColor(Color::from(self.config.theme.background_color)),
            status_str,
            " ".repeat(width as usize - status_str.len() - dir.len()),
            dir,
            crossterm::style::Attribute::Reset
        );

        std::io::stdout().flush().unwrap();
    }

    pub fn draw_tabs(&self) {
        let width = crossterm::terminal::size().unwrap().0 as usize;

        let mut doc_bar = String::new();

        doc_bar.push('+');
        doc_bar.push('─');

        let mut len = 2;
        let mut i = 0;
        for doc in &self.docs {
            if doc_bar.len() + doc.filename().len() + 3 < width {
                let tab_str = format!(
                    "{}{}{} x",
                    doc.display_name(),
                    if doc.is_binary_doc() { " (Binary)" } else { "" },
                    if doc.dirty() > 0 { " *" } else { "" }
                );
                len += tab_str.width() + 3;
                if let Some(open_doc) = self.open_doc {
                    if open_doc == i {
                        doc_bar.push_str(&format!(
                            "{}|{}|{}{}{}─",
                            crossterm::style::Attribute::Reverse,
                            tab_str,
                            crossterm::style::Attribute::Reset,
                            crossterm::style::SetBackgroundColor(Color::from(
                                self.config.theme.background_color
                            )),
                            crossterm::style::SetForegroundColor(Color::from(
                                self.config.theme.foreground_color
                            ))
                        ));
                        i += 1;
                        len += 1;
                        continue;
                    }
                }
                doc_bar.push_str(&format!("|{}|─", tab_str));
            }
            i += 1;
        }

        len -= 1;

        print!(
            "{}{}{}{}{}+",
            crossterm::cursor::MoveTo(0, 0),
            doc_bar.trim_end(),
            crossterm::style::SetBackgroundColor(Color::from(self.config.theme.background_color)),
            crossterm::style::SetForegroundColor(Color::from(self.config.theme.foreground_color)),
            "─".repeat(width - len - 1)
        );
    }

    pub fn read_new_filename(
        &self,
        filename_def: Option<String>,
    ) -> Result<String, Box<dyn Error>> {
        let (width, height) = (self.width(), self.height());

        let mut prefix = String::new();
        let mut filename = if filename_def.is_none() {
            String::new()
        } else {
            filename_def.unwrap()
        };
        let index = self.docs.len();

        let dir = std::env::current_dir().unwrap();
        
        let paths = std::fs::read_dir(dir).unwrap();

        let mut path_list = Vec::new();

        for path in paths {
            let path = path.unwrap();

            if path.file_type().unwrap().is_dir() {
                continue;
            }

            let fname = path.file_name();
            path_list.push(fname.to_str().unwrap().to_owned());
        }

        let mut file_index = 0;

        loop {
            let mut status_str = format!("[{}] - Doc {} of {}", filename, index, self.docs.len());

            let dir = std::env::current_dir().unwrap_or_default();
            let dir = format!(
                "in [/{}]",
                dir.iter()
                    .last()
                    .unwrap()
                    .to_os_string()
                    .into_string()
                    .unwrap()
            );

            if width < status_str.len() + dir.len() + 10 {
                status_str.drain(
                    ..status_str
                        .char_indices()
                        .nth(status_str.len() + dir.len() + 14 - width)
                        .unwrap()
                        .0,
                );
                status_str.insert_str(0, "[...");
            }

            print!(
                "{}{}{}{}{}{}{}{}",
                crossterm::cursor::MoveTo(0, height as u16 - 2),
                crossterm::style::SetForegroundColor(Color::from(
                    self.config.theme.background_color
                )),
                crossterm::style::SetBackgroundColor(Color::from(
                    self.config.theme.foreground_color
                )),
                status_str,
                " ".repeat(width as usize - status_str.len() - dir.len()),
                dir,
                crossterm::style::SetBackgroundColor(Color::from(
                    self.config.theme.background_color
                )),
                crossterm::style::SetForegroundColor(Color::from(
                    self.config.theme.foreground_color
                ))
            );

            std::io::stdout().flush().unwrap();

            if let Ok(Key(k)) = read() {
                if let KeyCode::Char(c) = k.code {
                    if k.modifiers.contains(KeyModifiers::CONTROL) && c == 'h' {
                        if filename.starts_with("bin:") {
                            filename = filename[4..].to_owned();
                        }
                        else {
                            filename = format!("bin:{}", filename);
                        }
                    }
                    else {
                        filename.push(c);
                        prefix = filename.clone();
                        file_index = 0;
                    }
                } else if let KeyCode::Enter = k.code {
                    break;
                } else if k.code == KeyCode::Esc {
                    return Err("Stopped".into());
                } else if k.code == KeyCode::Tab {
                    let mut file_list = Vec::new();

                    for file in &path_list {
                        if file.to_lowercase().starts_with(&prefix.to_lowercase()) {
                            file_list.push(file);
                        }
                    }

                    if file_list.len() == 0 {
                        continue;
                    }

                    file_index = file_index % file_list.len();

                    filename = file_list[file_index].clone();
                    file_index = (file_index + 1) % file_list.len();
                } else if k.code == KeyCode::Backspace && filename.len() > 0 {
                    filename.remove(filename.len() - 1);
                    prefix = filename.clone();
                }
            }
        }

        Ok(filename)
    }

    pub fn show_prompt(&self, title: String, msg: String, ok_msg: String, err_msg: String) -> Result<(), ()> {
        print!("{}", crossterm::cursor::Hide);

        let max_len = self.width() / 3 - 2;
        let len = max_len + 2;
        let padded_center = pad_center(title, len - 2);
        let mut selected_left = true;
        let mut msg_lines = Vec::with_capacity(msg.width() / max_len + 1);
        

        let mut update_prompt = true;
        let mut offset = 0;

        loop {
            if update_prompt {
                offset = 0;
                let mut start = 0;
                let mut end = std::cmp::min(msg.len(), max_len);

                msg_lines.clear();

                loop {
                    msg_lines.push(&msg[start..end]);

                    start += end;
                    end += max_len;

                    if start >= msg.len() {
                        break;
                    }

                    end = std::cmp::min(end, msg.len());
                }


                print!(
                    "{}{}{}+{}+",
                    crossterm::style::SetBackgroundColor(Color::from(self.config.theme.foreground_color)),
                    crossterm::style::SetForegroundColor(Color::from(self.config.theme.background_color)),
                    crossterm::cursor::MoveTo(
                        (self.width() / 2 - len / 2 - 1) as u16,
                        (self.height() / 2) as u16 - 2 + offset
                    ),
                    "=".repeat(len - 2)
                );
                offset += 1;
                print!(
                    "{}|{}{}{}|",
                    crossterm::cursor::MoveTo(
                        (self.width() / 2 - len / 2 - 1) as u16,
                        (self.height() / 2) as u16 - 2 + offset
                    ),
                    crossterm::style::Attribute::Bold, padded_center, crossterm::style::Attribute::NormalIntensity
                );
                offset += 1;

                for m in &msg_lines {
                    print!(
                        "{}|{}|",
                        crossterm::cursor::MoveTo(
                            (self.width() / 2 - len / 2 - 1) as u16,
                            (self.height() / 2) as u16 - 2 + offset
                        ),
                        pad_center_str(m, len - 2)
                    );
                    offset += 1;
                }

                print!(
                    "{}|{}|",
                    crossterm::cursor::MoveTo(
                        (self.width() / 2 - len / 2 - 1) as u16,
                        (self.height() / 2) as u16 - 2 + offset
                    ),
                    " ".repeat(len - 2)
                );
                offset += 1;
                print!(
                    "{}|{}|",
                    crossterm::cursor::MoveTo(
                        (self.width() / 2 - len / 2 - 1) as u16,
                        (self.height() / 2) as u16 - 2 + offset
                    ),
                    " ".repeat(len - 2)
                );
                offset += 1;

                if selected_left {
                    print!(
                        "{}|{}{}> {}{}{}{}{}|",
                        crossterm::cursor::MoveTo(
                            (self.width() / 2 - len / 2 - 1) as u16,
                            (self.height() / 2) as u16 - 2 + offset
                        ),
                        crossterm::style::Attribute::Underlined,crossterm::style::Attribute::Bold ,ok_msg,crossterm::style::Attribute::NoUnderline,crossterm::style::Attribute::NormalIntensity ,
                        " ".repeat(len - ok_msg.len() - err_msg.len() - 4),
                        err_msg
                    );
                }
                else {
                    print!(
                        "{}|{}{}{}{}> {}{}{}|",
                        crossterm::cursor::MoveTo(
                            (self.width() / 2 - len / 2 - 1) as u16,
                            (self.height() / 2) as u16 - 2 + offset
                        ),
                        ok_msg,
                        " ".repeat(len - ok_msg.len() - err_msg.len() - 4),
                        crossterm::style::Attribute::Underlined, crossterm::style::Attribute::Bold ,err_msg,crossterm::style::Attribute::NoUnderline, crossterm::style::Attribute::NormalIntensity 
                    );
                }
                
                offset += 1;
                print!(
                    "{}+{}+",
                    crossterm::cursor::MoveTo(
                        (self.width() / 2 - len / 2 - 1) as u16,
                        (self.height() / 2) as u16 - 2 + offset
                    ),
                    "=".repeat(len - 2)
                );
                std::io::stdout().flush().unwrap();
                offset -= 1;

                update_prompt = false;
            }

            match read().unwrap() {
                Mouse(m) => {
                    if let MouseEventKind::Down(button) = m.kind {
                        if button == MouseButton::Left {
                            if m.row == (self.height() / 2) as u16 - 2 + offset
                                && m.column > (self.width() / 2 - len / 2) as u16
                                && m.column < (self.width() / 2 - len / 2 + ok_msg.len()) as u16
                            {
                                print!("{}", crossterm::cursor::Show);
                                return Ok(());
                            } else if m.row == (self.height() / 2) as u16 - 2 + offset
                                && m.column < (self.width() / 2 + len / 2) as u16
                                && m.column > (self.width() / 2 + len / 2 - err_msg.len()) as u16
                            {
                                print!("{}", crossterm::cursor::Show);
                                return Err(());
                            }
                        }
                    }
                }
                Key(e) => {
                    if e.code == KeyCode::Esc {
                        return Err(());
                    }
                    else if e.code == KeyCode::Tab || e.code == KeyCode::Left || e.code == KeyCode::Right {
                        selected_left = !selected_left;
                        update_prompt = true;
                    }
                    else if e.code == KeyCode::Enter {
                        print!("{}", crossterm::cursor::Show);
                        return if selected_left { Ok(()) } else { Err(())};
                    }
                }
                _ => {}
            }
        }
    }

    pub fn show_start_splash(&self) -> Result<(), Box<dyn Error>> {
        print!("{}", crossterm::cursor::MoveTo(0, 0));
        print!(
            "{}{}",
            crossterm::style::SetBackgroundColor(Color::from(self.config.theme.background_color)),
            crossterm::style::SetForegroundColor(Color::from(self.config.theme.foreground_color))
        );

        std::io::stdout().flush()?;

        let (width, height): (usize, usize) = (self.width(), self.height());

        let title_string = format!("╭╮ Kelp Editor - {} ╭╮", kelp_version());
        let by_string = "────╯  Written in Rust by Vertex  ╰────";

        for y in 0..height {
            if y != 0 && y == height / 2 {
                println!(
                    " {}{}{}",
                    " ".repeat(width / 2 - 1 - title_string.width() / 2),
                    title_string,
                    " ".repeat(width - (width / 2 + 1 + title_string.width() / 2))
                );
            } else if y != 0 && y - 1 == height / 2 {
                println!(
                    " {}{}{}",
                    " ".repeat(width / 2 - 1 - by_string.width() / 2),
                    by_string,
                    " ".repeat(width - (width / 2 + 1 + by_string.width() / 2))
                );
            } else {
                if y != height - 1 {
                    println!(" {}", " ".repeat(width - 1));
                } else {
                    print!(" {}", " ".repeat(width - 1));
                    std::io::stdout().flush()?;
                }
            }
        }

        self.write_status_bar(None);

        Ok(())
    }

    fn show_global_prompt(&mut self) -> Result<(), ()> {
        print!("{}", crossterm::cursor::Hide);

        let mut theme = self.config.theme;

        (theme.background_color, theme.foreground_color) = (theme.foreground_color, theme.background_color);

        let mut redraw = true;

        let mut _scroll_index = 0;

        let prompt_height = (self.height() as f64 * 0.7) as usize;
        let prompt_width = (self.width() as f64 * 0.7) as usize;

        let mut actions: Vec<(String, Box<dyn Fn(usize, usize) -> String>)> = Vec::new();

        let close_draw = |w, _| {
            "`".repeat(w)
        };

        actions.push(("Close".to_owned(), Box::new(close_draw)));

        let mut current_action = 0;

        let mut first_slide_len = 0;

        for (k, _) in &actions {
            first_slide_len = std::cmp::max(first_slide_len, k.width() + 4);
        }

        for f in &self.docs {
            first_slide_len = std::cmp::max(first_slide_len, f.display_name().width() + 4);
        }

        let action_slide_len = prompt_width - 3 - first_slide_len;
        let slide_len_unpadded = first_slide_len - 2;

        let action_len = actions.len() + self.docs.len();

        let pad = |s: String, w: usize| -> String {
            if s.width() > w {
                let mut ns = String::with_capacity(w);
                for c in s.chars() {
                    if ns.width() < w {
                        ns.push(c);
                    }
                    else {
                        break;
                    }
                }
                ns
            }
            else {
                format!("{: <w$}", s)
            }
        };

        let shade = "▓";

        let reset = format!("{}{}{}", crossterm::style::Attribute::Reset, 
            crossterm::style::SetBackgroundColor(self.config.theme.foreground_color.into()),
            crossterm::style::SetForegroundColor(self.config.theme.background_color.into())
        );

        loop {
            if redraw {
                let draw_file = |file_index: usize, w: usize, y: usize| -> String {
                    let extension = (&self.docs[file_index - actions.len()].as_text_doc()).extension();

                    if file_index >= actions.len() {
                        if file_index - actions.len() < self.docs.len() {
                            if self.docs[file_index - actions.len()].is_text_doc() {
                                
                                if y == 1 {
                                    pad(self.docs[file_index - actions.len()].display_name(), w)
                                } else if y == 2 {
                                    format!("───┬{}", "─".repeat(w - 4))
                                } else if y >= 3 {
                                    let line = y - 3;

                                    if line < self.docs[file_index - actions.len()].as_text_doc().rows.len() {
                                        let config = if self
                                            .config
                                            .languages
                                            .contains_key(&extension)
                                        {
                                            self.config.languages
                                                [&extension].clone()
                                        } else {
                                            self.config.languages[&"*".to_owned()].clone()
                                        };

                                        let row_str = self.docs[file_index - actions.len()].as_text_doc().rows[line].display_buf_upto(&config, &theme, w - 4);

                                        let mut actual_width = self.docs[file_index - actions.len()].as_text_doc().rows[line].buf.width();

                                        let mut res = format!("{: >3}│{}", line + 1, row_str);

                                        res.push_str(&format!("{}{}",
                                            crossterm::style::SetBackgroundColor(self.config.theme.foreground_color.into()),
                                            crossterm::style::SetForegroundColor(self.config.theme.background_color.into()))
                                        );

                                        while actual_width < w - 4 {
                                            res.push(' ');
                                            actual_width += 1;
                                        }

                                        res.push_str(&reset);

                                        res
                                    }
                                    else {
                                        format!("   │{}", " ".repeat(w - 4))
                                    }
                                } else {
                                    " ".repeat(w)
                                }
                            }
                            else {
                                " ".repeat(w)
                            }
                        }
                        else {
                            " ".repeat(w)
                        }
                    } else {
                        "`".repeat(w)
                    }
                };
                for y in 0..prompt_height {
                    if y == 0 {
                        print!("{} {}{}┌{}┬{}┐{}{}", 
                                crossterm::cursor::MoveTo(((self.width() - prompt_width) / 2) as u16,((self.height() - prompt_height) / 2 + y) as u16),
                                crossterm::style::SetBackgroundColor(self.config.theme.foreground_color.into()),
                                crossterm::style::SetForegroundColor(self.config.theme.background_color.into()),
                                "─".repeat(first_slide_len),
                                "─".repeat(prompt_width - 3 - first_slide_len),
                                crossterm::style::SetBackgroundColor(self.config.theme.background_color.into()),
                                crossterm::style::SetForegroundColor(self.config.theme.foreground_color.into()),
                            );
                    } else if y + 1 == prompt_height {
                        print!("{}{}{}{shade}└{}┴{}{}┘{}{}", 
                                crossterm::cursor::MoveTo(((self.width() - prompt_width) / 2) as u16,((self.height() - prompt_height) / 2 + y) as u16),
                                crossterm::style::SetBackgroundColor(self.config.theme.foreground_color.into()),
                                crossterm::style::SetForegroundColor(self.config.theme.background_color.into()),
                                "─".repeat(first_slide_len),
                                if current_action < actions.len() {
                                    "────"
                                } else {
                                    "───┴"
                                },
                                "─".repeat(prompt_width - 7 - first_slide_len),
                                crossterm::style::SetBackgroundColor(self.config.theme.background_color.into()),
                                crossterm::style::SetForegroundColor(self.config.theme.foreground_color.into()),
                            );
                    } else {
                        if current_action < actions.len() {
                            print!("{}{}{}{shade}{}{}{}{}│{}{}", 
                                    crossterm::cursor::MoveTo(((self.width() - prompt_width) / 2) as u16,((self.height() - prompt_height) / 2 + y) as u16),
                                    crossterm::style::SetBackgroundColor(self.config.theme.foreground_color.into()),
                                    crossterm::style::SetForegroundColor(self.config.theme.background_color.into()),
                                    if y - 1 == actions.len() {
                                        "├"
                                    }
                                    else {
                                        "│"
                                    },
                                    if y - 1 < actions.len() {
                                        if y - 1 == current_action {
                                            format!("{}{}> {: <slide_len_unpadded$}{}{}",
                                                crossterm::style::Attribute::Underlined,
                                                crossterm::style::Attribute::Bold,
                                                &actions[y - 1].0,
                                                crossterm::style::Attribute::NoUnderline,
                                                crossterm::style::Attribute::NormalIntensity,
                                            )
                                        } else {
                                            format!("  {: <slide_len_unpadded$}",&actions[y - 1].0)
                                        }
                                    } else if y - 1 == actions.len() {
                                        "─".repeat(first_slide_len)
                                    } else {
                                        format!("{}{}",
                                        if current_action == y - 2 {
                                            "> "
                                        }
                                        else {
                                            "  "
                                        },
                                        if y - 2 - actions.len() < self.docs.len() {
                                            pad(self.docs[y - actions.len() - 2].display_name(), first_slide_len - 2)
                                        }
                                        else {
                                            " ".repeat(first_slide_len - 2)
                                        })
                                    },
                                    if y - 1 == actions.len() {
                                        "┤"
                                    }
                                    else {
                                        "│"
                                    },
                                    (actions[current_action].1)(action_slide_len, y - 1),
                                    crossterm::style::SetBackgroundColor(self.config.theme.background_color.into()),
                                    crossterm::style::SetForegroundColor(self.config.theme.foreground_color.into()),
                                );
                        }
                        else {
                            print!("{}{}{}{shade}│{}{}{}│{}{}", 
                                    crossterm::cursor::MoveTo(((self.width() - prompt_width) / 2) as u16,((self.height() - prompt_height) / 2 + y) as u16),
                                    crossterm::style::SetBackgroundColor(self.config.theme.foreground_color.into()),
                                    crossterm::style::SetForegroundColor(self.config.theme.background_color.into()),
                                    if y - 1 < actions.len() {
                                        if y - 1 == current_action {
                                            format!("{}{}> {: <slide_len_unpadded$}{}{}",
                                                crossterm::style::Attribute::Underlined,
                                                crossterm::style::Attribute::Bold,
                                                &actions[y - 1].0,
                                                crossterm::style::Attribute::NoUnderline,
                                                crossterm::style::Attribute::NormalIntensity,
                                            )
                                        } else {
                                            format!("  {: <slide_len_unpadded$}",&actions[y - 1].0)
                                        }
                                    } else if y - 1 == actions.len() {
                                        "─".repeat(first_slide_len)
                                    } else {
                                        format!("{}{}",
                                        if current_action == y - 2 {
                                            "> "
                                        }
                                        else {
                                            "  "
                                        },
                                        if y - 2 - actions.len() < self.docs.len() {
                                            pad(self.docs[y - actions.len() - 2].display_name(), first_slide_len - 2)
                                        }
                                        else {
                                            " ".repeat(first_slide_len - 2)
                                        })
                                    },
                                    if current_action < actions.len() {
                                        "│"
                                    }
                                    else {
                                        if y - 1 == actions.len() {
                                            "┼"
                                        }
                                        else {
                                            "│"
                                        }
                                    }//┤
                                    ,
                                    draw_file(current_action, prompt_width - 3 - first_slide_len, y),
                                    crossterm::style::SetBackgroundColor(self.config.theme.background_color.into()),
                                    crossterm::style::SetForegroundColor(self.config.theme.foreground_color.into()),
                                );
                        }
                    }
                }
                print!("{}{}{}{}{}{}",
                    crossterm::cursor::MoveTo(((self.width() - prompt_width) / 2) as u16,((self.height() + prompt_height) / 2) as u16),
                    crossterm::style::SetBackgroundColor(self.config.theme.foreground_color.into()),
                    crossterm::style::SetForegroundColor(self.config.theme.background_color.into()),
                    shade.repeat(prompt_width),
                    crossterm::style::SetBackgroundColor(self.config.theme.background_color.into()),
                    crossterm::style::SetForegroundColor(self.config.theme.foreground_color.into()),
                );

                _ = stdout().flush();

                redraw = false;
            }


            match read().unwrap() {
                Key(e) => {
                    if e.code == KeyCode::Esc {
                        return Ok(());
                    }
                    else if e.code == KeyCode::Down {
                        current_action = (current_action + 1) % action_len;
                        redraw = true;
                    }
                    else if e.code == KeyCode::Up {
                        if current_action == 0 {
                            current_action = action_len - 1;
                        }
                        else {
                            current_action -= 1;
                            current_action %= action_len;
                        }
                        redraw = true;
                    }
                    else if e.code == KeyCode::Enter {
                        if current_action < actions.len() {
                            if actions[current_action].0 == "Close" {
                                print!("{}", crossterm::cursor::Show);
                                return Err(());
                            }
                        }
                        else {
                            let doc_index = current_action - actions.len();
                            self.open_doc = Some(doc_index);
                            print!("{}", crossterm::cursor::Show);
                            return Ok(());
                        }
                    }
                }
                _ => {}
            }
        }
    }
}
