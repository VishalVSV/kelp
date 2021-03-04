use crate::editor::Editor;

mod editor;

#[macro_use]
extern crate serde_derive;

fn main() {
    let editor = Editor::new();
    let _ = editor.start();
}