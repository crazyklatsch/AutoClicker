use enigo::*;
use serde::{Deserialize, Serialize};
use serde_json;
use std::{fs::File, io::{Error, Read, Result, Write}, path::Path, sync::{atomic::{AtomicBool, Ordering}, Arc}};
use std::{thread, time};

#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub enum KeyButton {
    KeyboardKey(enigo::Key),
    MouseButton(enigo::MouseButton),
}
impl From<enigo::Key> for KeyButton {
    fn from(value: enigo::Key) -> Self {
        KeyButton::KeyboardKey(value)
    }
}
impl From<enigo::MouseButton> for KeyButton {
    fn from(value: enigo::MouseButton) -> Self {
        KeyButton::MouseButton(value)
    }
}
impl std::fmt::Display for KeyButton {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KeyButton::KeyboardKey(val) => write!(f, "{:?}", val),
            KeyButton::MouseButton(val) => write!(f, "{:?}", val),
        }
    }
}
impl KeyButton {
    pub fn down(self, enigo: &mut Enigo) {
        match self {
            KeyButton::KeyboardKey(key) => enigo.key_down(key),
            KeyButton::MouseButton(button) => enigo.mouse_down(button),
        }
    }
    pub fn up(self, enigo: &mut Enigo) {
        match self {
            KeyButton::KeyboardKey(key) => enigo.key_up(key),
            KeyButton::MouseButton(button) => enigo.mouse_up(button),
        }
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct PressAction {
    pub keybutton: KeyButton,
    pub down: bool,
    pub up: bool,
    pub hold_time_ms: u64,
    pub delay_after_ms: u64,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct MoveAction {
    pub x: i32,
    pub y: i32,
    pub relative: bool,
    pub _move_time_ms: u64,
    pub delay_after_ms: u64,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct DelayAction {
    pub delay_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopAction {
    pub infinite: bool,
    pub iterations: u64,
    pub actions: Vec<Action>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Action {
    Loop(LoopAction),
    Press(PressAction),
    Move(MoveAction),
    Delay(DelayAction),
}
impl From<MoveAction> for Action {
    fn from(value: MoveAction) -> Self {
        Action::Move(value)
    }
}
impl From<PressAction> for Action {
    fn from(value: PressAction) -> Self {
        Action::Press(value)
    }
}
impl From<DelayAction> for Action {
    fn from(value: DelayAction) -> Self {
        Action::Delay(value)
    }
}
impl From<LoopAction> for Action {
    fn from(value: LoopAction) -> Self {
        Action::Loop(value)
    }
}

impl MoveAction {
    pub fn execute(self, enigo: &mut Enigo) {
        if self.relative {
            enigo.mouse_move_relative(self.x, self.y);
        } else {
            enigo.mouse_move_to(self.x, self.y);
        }
        thread::sleep(time::Duration::from_millis(self.delay_after_ms));
    }
}

impl PressAction {
    pub fn execute(self, enigo: &mut Enigo) {
        if self.down {
            self.keybutton.down(enigo);
        } else if self.up {
            self.keybutton.up(enigo);
        }
        if self.down && self.up {
            thread::sleep(time::Duration::from_millis(self.hold_time_ms));
            self.keybutton.up(enigo);
        }
        thread::sleep(time::Duration::from_millis(self.delay_after_ms));
    }
}

impl DelayAction {
    pub fn execute(self) {
        thread::sleep(time::Duration::from_millis(self.delay_ms));
    }
}

impl LoopAction {
    pub fn execute(self, enigo: &mut Enigo, stop_execution: Option<Arc<AtomicBool>>) {
        let mut i = 0;
        let mut terminate = false;

        while (i < self.iterations || self.infinite) && !terminate {
            for action in &self.actions {
                action.clone().execute(enigo, stop_execution.clone());
                if stop_execution
                    .as_ref()
                    .is_some_and(|b| b.load(Ordering::Relaxed))
                {
                    // Handle Key downs? (will not be necessary with next enigo release)
                    terminate = true;
                    break;
                }
            }
            if !self.infinite {
                i = i + 1;
            }
        }
    }

    pub fn save_to_disk<P: AsRef<Path>>(&self, path: &P) -> Result<()> {
        let mut f = File::create(path.as_ref())?;
        let buf = serde_json::to_vec(&self)?;
        f.write_all(&buf[..])?;
        return Ok(());
    }

    pub fn load_from_disk<P: AsRef<Path>>(&mut self, path: &P) -> Result<()> {
        let mut f = File::open(path.as_ref())?;
        let mut buf = vec![];
        match f.read_to_end(&mut buf) {
            Ok(_) => {
                if let Ok(loopaction) = serde_json::from_slice::<LoopAction>(&buf[..]) {
                    self.clone_from(&loopaction);
                }
                else {
                    return Err(Error::new(std::io::ErrorKind::Other,"Couldn't deserialize buf into a LoopAction"));
                }
            }
            Err(val) => return Err(val),
        }

        return Ok(());
    }
}

impl Action {
    fn execute(self, enigo: &mut Enigo, stop_execution: Option<Arc<AtomicBool>>) {
        match self {
            Action::Loop(val) => val.execute(enigo, stop_execution),
            Action::Move(val) => val.execute(enigo),
            Action::Press(val) => val.execute(enigo),
            Action::Delay(val) => val.execute(),
        }
    }
}
