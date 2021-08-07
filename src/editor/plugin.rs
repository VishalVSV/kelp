use crate::Editor;
use crossterm::event::Event;
use std::any::Any;

pub trait Plugin: Any + Send + Sync {
    fn on_load(&self, _editor: &mut Editor) {}
    fn on_event(&self, _editor: &mut Editor, _event: &Event) {}
}

#[macro_export]
macro_rules! declare_plugin {
    ($plugin_type:ty, $constructor:path) => {
        #[no_mangle]
        pub extern "C" fn _plugin_create() -> *mut $crate::Plugin {
            // make sure the constructor is the correct type.
            let constructor: fn() -> $plugin_type = $constructor;

            let object = constructor();
            let boxed: Box<$crate::Plugin> = Box::new(object);
            Box::into_raw(boxed)
        }
    };
}
