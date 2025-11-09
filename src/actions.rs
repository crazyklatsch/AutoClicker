use enigo::*;
use serde::{Deserialize, Serialize};
use serde_json;
use std::{
    fs::File,
    io::{Error, Read, Write},
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use std::{thread, time};

use crate::errors::AppError;

#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub enum KeyButton {
    KeyboardKey(enigo::Key),
    MouseButton(enigo::Button),
}
impl From<enigo::Key> for KeyButton {
    fn from(value: enigo::Key) -> Self {
        KeyButton::KeyboardKey(value)
    }
}
impl From<enigo::Button> for KeyButton {
    fn from(value: enigo::Button) -> Self {
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
    pub fn down(self, enigo: &mut Enigo) -> Result<(), AppError> {
        match self {
            KeyButton::KeyboardKey(key) => enigo.key(key, Direction::Press)?,
            KeyButton::MouseButton(button) => enigo.button(button, Direction::Press)?,
        }
        Ok(())
    }
    pub fn up(self, enigo: &mut Enigo) -> Result<(), AppError> {
        match self {
            KeyButton::KeyboardKey(key) => enigo.key(key, Direction::Release)?,
            KeyButton::MouseButton(button) => enigo.button(button, Direction::Release)?,
        }
        Ok(())
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
    pub move_time_ms: u64,
    pub delay_after_ms: u64,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct DelayAction {
    pub random: bool,
    pub delay_ms_min: u64, // used if not random
    pub delay_ms_max: u64,
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
    pub fn execute(self, enigo: &mut Enigo) -> Result<(), AppError> {
        if self.move_time_ms == 0 {
            if self.relative {
                enigo.move_mouse(self.x, self.y, Coordinate::Rel)?;
            } else {
                enigo.move_mouse(self.x, self.y, Coordinate::Abs)?;
            }
        } else {
            let timestep_ms = 3;
            let mut x_rel = self.x;
            let mut y_rel = self.y;

            if !self.relative {
                let pos = enigo.location()?;
                x_rel = self.x - pos.0;
                y_rel = self.y - pos.1;
            }

            let mut time_passed_ms: u64 = 0;
            let mut x_last_cycle = 0;
            let mut y_last_cycle = 0;

            loop {
                let factor = time_passed_ms as f64 / self.move_time_ms as f64;
                let x = (x_rel as f64 * factor).floor() as i32;
                let y = (y_rel as f64 * factor).floor() as i32;

                enigo.move_mouse(x - x_last_cycle, y - y_last_cycle, Coordinate::Rel)?;

                let sleep_time = if time_passed_ms + timestep_ms < self.move_time_ms {
                    timestep_ms
                } else {
                    self.move_time_ms - time_passed_ms
                };

                if time_passed_ms >= self.move_time_ms {
                    break;
                }
                thread::sleep(time::Duration::from_millis(sleep_time));
                time_passed_ms += sleep_time;
                x_last_cycle = x;
                y_last_cycle = y;
            }
        }

        thread::sleep(time::Duration::from_millis(self.delay_after_ms));
        Ok(())
    }
}

impl PressAction {
    pub fn execute(self, enigo: &mut Enigo) -> Result<(), AppError> {
        if self.down {
            self.keybutton.down(enigo)?;
        } else if self.up {
            self.keybutton.up(enigo)?;
        }
        if self.down && self.up {
            thread::sleep(time::Duration::from_millis(self.hold_time_ms));
            self.keybutton.up(enigo)?;
        }
        thread::sleep(time::Duration::from_millis(self.delay_after_ms));
        Ok(())
    }
}

impl DelayAction {
    pub fn execute(self) {
        if !self.random || self.delay_ms_min >= self.delay_ms_max {
            thread::sleep(time::Duration::from_millis(self.delay_ms_min));
        } else {
            thread::sleep(time::Duration::from_millis(fastrand::u64(
                self.delay_ms_min..self.delay_ms_max,
            )));
        }
    }
}

impl LoopAction {
    pub fn execute(
        self,
        enigo: &mut Enigo,
        stop_execution: Option<Arc<AtomicBool>>,
    ) -> Result<(), AppError> {
        let mut i = 0;
        let mut terminate = false;

        while (i < self.iterations || self.infinite) && !terminate {
            for action in &self.actions {
                action.clone().execute(enigo, stop_execution.clone())?;
                if stop_execution
                    .as_ref()
                    .is_some_and(|b| b.load(Ordering::Relaxed))
                {
                    terminate = true;
                    break;
                }
            }
            if !self.infinite {
                i = i + 1;
            } else if self.actions.is_empty() {
                break;
            }
        }
        Ok(())
    }

    pub fn save_to_disk<P: AsRef<Path>>(&self, path: &P) -> std::io::Result<()> {
        let mut f = File::create(path.as_ref())?;
        let buf = serde_json::to_vec(&self)?;
        f.write_all(&buf[..])?;
        return Ok(());
    }

    pub fn load_from_disk<P: AsRef<Path>>(&mut self, path: &P) -> std::io::Result<()> {
        let mut f = File::open(path.as_ref())?;
        let mut buf = vec![];
        match f.read_to_end(&mut buf) {
            Ok(_) => {
                if let Ok(loopaction) = serde_json::from_slice::<LoopAction>(&buf[..]) {
                    self.clone_from(&loopaction);
                } else {
                    return Err(Error::new(
                        std::io::ErrorKind::Other,
                        "Couldn't deserialize buf into a LoopAction",
                    ));
                }
            }
            Err(val) => return Err(val),
        }

        return Ok(());
    }
}

impl Action {
    fn execute(
        self,
        enigo: &mut Enigo,
        stop_execution: Option<Arc<AtomicBool>>,
    ) -> Result<(), AppError> {
        match self {
            Action::Loop(val) => val.execute(enigo, stop_execution)?,
            Action::Move(val) => val.execute(enigo)?,
            Action::Press(val) => val.execute(enigo)?,
            Action::Delay(val) => val.execute(),
        }
        Ok(())
    }
}
