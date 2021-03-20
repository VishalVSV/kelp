use crossterm::event::Event;
use crate::Editor;

pub trait Plugin {
    fn on_load(editor: &mut Editor);
    fn on_event(editor: &mut Editor, event: Event);
}