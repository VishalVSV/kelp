use crate::editor::Editor;

mod editor;

fn main() {
    let editor = Editor::new();
    let _ = editor.start();
}