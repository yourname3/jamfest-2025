use std::collections::{BTreeMap, HashMap};

pub use winit::keyboard::KeyCode;
pub use winit::event::MouseButton;

struct ButtonStateTracker<T: Clone + Ord + PartialEq> {
    // TODO: Consider using Vec instead of BTreeMap
    current: BTreeMap<T, bool>,
    last   : BTreeMap<T, bool>
}

impl<T: Clone + Ord + PartialEq> ButtonStateTracker<T> {
    pub fn new() -> Self {
        ButtonStateTracker {
            current: BTreeMap::new(),
            last   : BTreeMap::new(),
        }
    }

    pub fn is_pressed(&self, key: &T) -> bool {
        self.current.get(key).cloned().unwrap_or(false)
    }

    pub fn was_pressed(&self, key: &T) -> bool {
        self.last.get(key).cloned().unwrap_or(false)
    }

    pub fn is_just_pressed(&self, key: &T) -> bool {
        self.is_pressed(key) && !self.was_pressed(key)
    }

    pub fn is_just_released(&self, key: &T) -> bool {
        self.was_pressed(key) && !self.is_pressed(key)
    }

    pub fn tick_end(&mut self) {
        for (k, v) in self.current.iter() {
            self.last.insert(k.clone(), *v);
        }
    }

    pub fn update(&mut self, key: T, value: bool) {
        self.current.insert(key, value);
    }
}

#[derive(Clone, PartialOrd, Ord, PartialEq, Eq)]
pub(crate) enum AnyButton {
    PhysicalKey(winit::keyboard::PhysicalKey),
    Mouse(MouseButton),
}

pub struct Input {
    tracker: ButtonStateTracker<AnyButton>
}

impl Input {
    pub fn new() -> Self {
        Input { tracker: ButtonStateTracker::new() }
    }

    pub fn is_physical_key_pressed(&self, code: KeyCode) -> bool {
        self.tracker.is_pressed(&AnyButton::PhysicalKey(winit::keyboard::PhysicalKey::Code(code)))
    }

    pub fn was_physical_key_pressed(&self, code: KeyCode) -> bool {
        self.tracker.was_pressed(&AnyButton::PhysicalKey(winit::keyboard::PhysicalKey::Code(code)))
    }

    pub fn is_physical_key_just_pressed(&self, code: KeyCode) -> bool {
        self.tracker.is_just_pressed(&AnyButton::PhysicalKey(winit::keyboard::PhysicalKey::Code(code)))
    }

    pub fn was_physical_key_just_released(&self, code: KeyCode) -> bool {
        self.tracker.is_just_released(&AnyButton::PhysicalKey(winit::keyboard::PhysicalKey::Code(code)))
    }

    pub fn is_mouse_pressed(&self, button: MouseButton) -> bool {
        self.tracker.is_pressed(&AnyButton::Mouse(button))
    }

    pub fn was_mouse_pressed(&self, button: MouseButton) -> bool {
        self.tracker.was_pressed(&AnyButton::Mouse(button))
    }

     pub fn is_mouse_just_pressed(&self, button: MouseButton) -> bool {
        self.tracker.is_just_pressed(&AnyButton::Mouse(button))
    }

    pub fn is_mouse_just_released(&self, button: MouseButton) -> bool {
        self.tracker.is_just_released(&AnyButton::Mouse(button))
    }

    pub fn tick_end(&mut self) {
        self.tracker.tick_end();
    }

    pub(crate) fn update_button(&mut self, button: AnyButton, pressed: bool) {
        self.tracker.update(button, pressed);
    }
}