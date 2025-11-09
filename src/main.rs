#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

#[cfg(target_os = "windows")]
mod actions;
mod errors;

use crate::actions::*;
use eframe::egui::{self, Color32, Ui};
use enigo::{Enigo, Settings};
use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::thread::JoinHandle;

fn main() -> Result<(), eframe::Error> {
    //env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1080.0, 720.0]),
        ..Default::default()
    };

    eframe::run_native(
        "AutoClicker by crazyklatsch",
        options,
        Box::new(|cc| {
            // This gives us image support:
            egui_extras::install_image_loaders(&cc.egui_ctx);

            Box::<MyApp>::default()
        }),
    )
}

struct MyApp {
    root_action: LoopAction,
    start_thread: Arc<AtomicBool>,
    stop_thread: Arc<AtomicBool>,
    thread_running: Arc<AtomicBool>,
    thread_handle: JoinHandle<()>,
    thread_stop_signal: Arc<AtomicBool>,
    hotkey_manager: GlobalHotKeyManager,
    hotkey_start_id: u32,
    hotkey_stop_id: u32,
    save_name: String,
}

impl MyApp {
    fn start_thread(&mut self) {
        self.stop_thread();
        let mut enigo = Enigo::new(&Settings::default()).unwrap();
        let action_copy = self.root_action.clone();
        let stop_signal = self.thread_stop_signal.clone();
        let running = self.thread_running.clone();
        self.thread_handle = thread::spawn(move || {
            running.store(true, Ordering::Relaxed);
            if let Err(err) = action_copy.execute(&mut enigo, Some(stop_signal)) {
                println!("Execution Thread encountered an error: {}", err);
            }
            running.store(false, Ordering::Relaxed);
        });
    }

    fn stop_thread(&mut self) {
        self.thread_stop_signal.store(true, Ordering::Relaxed);
        self.thread_stop_signal = Arc::new(AtomicBool::new(false));
        self.thread_running.store(false, Ordering::Relaxed);
    }
}

impl Default for MyApp {
    fn default() -> Self {
        let mut mods = Modifiers::SHIFT;
        mods.insert(Modifiers::CONTROL);
        let hotkey_start = HotKey::new(Some(mods), Code::F6);
        let hotkey_stop = HotKey::new(Some(mods), Code::F7);

        let myapp = Self {
            root_action: LoopAction {
                infinite: true,
                iterations: 1,
                actions: Vec::new(),
            },
            stop_thread: Arc::new(AtomicBool::new(false)),
            start_thread: Arc::new(AtomicBool::new(false)),
            thread_running: Arc::new(AtomicBool::new(false)),
            thread_stop_signal: Arc::new(AtomicBool::new(false)),
            thread_handle: thread::spawn(|| {}),
            hotkey_manager: GlobalHotKeyManager::new().unwrap(),
            hotkey_start_id: hotkey_start.id(),
            hotkey_stop_id: hotkey_stop.id(),
            save_name: String::new(),
        };

        let res = myapp.hotkey_manager.register(hotkey_start);
        match res {
            Ok(_) => println!("Successfully registered hotkey_start"),
            Err(val) => println!("Could not register hotkey_start: {:?}", val),
        }
        let res = myapp.hotkey_manager.register(hotkey_stop);
        match res {
            Ok(_) => println!("Successfully registered hotkey_stop"),
            Err(val) => println!("Could not register hotkey_stop: {:?}", val),
        }
        myapp
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::SidePanel::new(egui::panel::Side::Left, egui::Id::new("Right side"))
            .exact_width(150.0)
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            if ui.button("Start").clicked() {
                                self.start_thread.store(true, Ordering::Relaxed);
                            }
                            ui.label("or 'ctrl+shift+F6'");
                        });
                    });
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            if ui.button("Stop").clicked() {
                                self.stop_thread.store(true, Ordering::Relaxed);
                            }
                            ui.label("or 'ctrl+shift+F7'");
                        });
                    });
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            if ui.button("Load").clicked() {
                                if let Some(path) = rfd::FileDialog::new().pick_file() {
                                    let res = self.root_action.load_from_disk(&path);
                                    match res {
                                        Ok(_) => {
                                            self.save_name = String::from(
                                                path.file_stem().unwrap().to_str().unwrap(),
                                            )
                                        }
                                        Err(val) => println!(
                                            "Could not read from disk: '{}'",
                                            val.to_string()
                                        ),
                                    }
                                }
                            }
                            if ui.button("Save").clicked() {
                                let curr_path = std::env::current_dir().unwrap();
                                let path = rfd::FileDialog::new()
                                    .set_file_name(format!(
                                        "{}.aclick",
                                        if self.save_name.is_empty() {
                                            "save"
                                        } else {
                                            self.save_name.as_str()
                                        }
                                    ))
                                    .set_directory(&curr_path)
                                    .save_file();
                                if path.is_some() {
                                    if let Err(err) = self.root_action.save_to_disk(&path.unwrap())
                                    {
                                        println!("Could not save to disk: '{}'", err.to_string())
                                    }
                                }
                            }
                        });
                    });
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.label("Running: ");
                            ui.add_space(10.0);
                            let circle_color = if self.thread_running.load(Ordering::Relaxed) {
                                Color32::from_rgb(10, 255, 10)
                            } else {
                                Color32::from_rgb(255, 10, 10)
                            };
                            ui.painter().circle_filled(
                                ui.next_widget_position(),
                                8.0,
                                circle_color,
                            );
                            ui.add_space(10.0);
                        });
                    });
                });
            });
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                let mut loop_index: u32 = 0;
                add_loop_action(ui, &mut self.root_action, &mut loop_index, 0);
            });
        });

        if let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            if event.id == self.hotkey_start_id {
                if event.state == HotKeyState::Pressed {
                    self.start_thread.store(true, Ordering::Relaxed);
                }
            } else if event.id == self.hotkey_stop_id {
                if event.state == HotKeyState::Pressed {
                    self.stop_thread.store(true, Ordering::Relaxed);
                }
            } else {
                println!("Unhandled Event: {:?}", event);
            }
        }

        if self.start_thread.load(Ordering::Relaxed) {
            self.start_thread();
            self.start_thread.store(false, Ordering::Relaxed);
        }
        if self.stop_thread.load(Ordering::Relaxed) {
            self.stop_thread();
            self.stop_thread.store(false, Ordering::Relaxed);
        }

        // Continuous mode
        ctx.request_repaint();
    }
}

fn add_loop_action(ui: &mut Ui, loopaction: &mut LoopAction, loop_index: &mut u32, depth: u16) {
    let mut index_to_rm: Option<usize> = None;
    *loop_index += 1;
    let current_loop_index = *loop_index;

    ui.horizontal(|ui| {
        ui.checkbox(&mut loopaction.infinite, "Infinite Loop");
        if !loopaction.infinite {
            ui.label("iterations: ");
            ui.add(egui::DragValue::new(&mut loopaction.iterations));
        }
        ui.separator();
        ui.add_space(10.0);
        ui.vertical(|ui| {
            for (pos, action) in loopaction.actions.iter_mut().enumerate() {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(format!("{}.{}: ", current_loop_index, pos));
                        match action {
                            Action::Loop(val) => {
                                add_loop_action(ui, val, loop_index, depth + 1);
                            }
                            Action::Move(val) => {
                                ui.label("x: ");
                                ui.add(egui::DragValue::new(&mut val.x));
                                ui.add_space(10.0);
                                ui.label("y: ");
                                ui.add(egui::DragValue::new(&mut val.y));
                                ui.add_space(10.0);
                                ui.checkbox(&mut val.relative, "Relative");
                                ui.add_space(159.0);
                                // ui.label("Move-Time (ms): ");
                                // ui.add(egui::DragValue::new(&mut val.move_time_ms));
                                // ui.add_space(10.0);
                                ui.label("Delay after (ms): ");
                                ui.add(egui::DragValue::new(&mut val.delay_after_ms));
                            }
                            Action::Press(val) => {
                                egui::ComboBox::new(
                                    pos + (current_loop_index as usize) * 5000,
                                    "Key",
                                )
                                .selected_text(format!("{}", val.keybutton))
                                .show_ui(ui, |ui| {
                                    ui.style_mut().wrap = Some(false);
                                    ui.set_min_width(60.0);

                                    static ALL_KEYS: [enigo::Key; 247] = [
                                        enigo::Key::Num0,
                                        enigo::Key::Num1,
                                        enigo::Key::Num2,
                                        enigo::Key::Num3,
                                        enigo::Key::Num4,
                                        enigo::Key::Num5,
                                        enigo::Key::Num6,
                                        enigo::Key::Num7,
                                        enigo::Key::Num8,
                                        enigo::Key::Num9,
                                        enigo::Key::A,
                                        enigo::Key::B,
                                        enigo::Key::C,
                                        enigo::Key::D,
                                        enigo::Key::E,
                                        enigo::Key::F,
                                        enigo::Key::G,
                                        enigo::Key::H,
                                        enigo::Key::I,
                                        enigo::Key::J,
                                        enigo::Key::K,
                                        enigo::Key::L,
                                        enigo::Key::M,
                                        enigo::Key::N,
                                        enigo::Key::O,
                                        enigo::Key::P,
                                        enigo::Key::Q,
                                        enigo::Key::R,
                                        enigo::Key::S,
                                        enigo::Key::T,
                                        enigo::Key::U,
                                        enigo::Key::V,
                                        enigo::Key::W,
                                        enigo::Key::X,
                                        enigo::Key::Y,
                                        enigo::Key::Z,
                                        enigo::Key::AbntC1,
                                        enigo::Key::AbntC2,
                                        enigo::Key::Accept,
                                        enigo::Key::Add,
                                        enigo::Key::Alt,
                                        enigo::Key::Apps,
                                        enigo::Key::Attn,
                                        enigo::Key::Backspace,
                                        enigo::Key::BrowserBack,
                                        enigo::Key::BrowserFavorites,
                                        enigo::Key::BrowserForward,
                                        enigo::Key::BrowserHome,
                                        enigo::Key::BrowserRefresh,
                                        enigo::Key::BrowserSearch,
                                        enigo::Key::BrowserStop,
                                        enigo::Key::Cancel,
                                        enigo::Key::CapsLock,
                                        enigo::Key::Clear,
                                        enigo::Key::Control,
                                        enigo::Key::Convert,
                                        enigo::Key::Crsel,
                                        enigo::Key::DBEAlphanumeric,
                                        enigo::Key::DBECodeinput,
                                        enigo::Key::DBEDetermineString,
                                        enigo::Key::DBEEnterDLGConversionMode,
                                        enigo::Key::DBEEnterIMEConfigMode,
                                        enigo::Key::DBEEnterWordRegisterMode,
                                        enigo::Key::DBEFlushString,
                                        enigo::Key::DBEHiragana,
                                        enigo::Key::DBEKatakana,
                                        enigo::Key::DBENoCodepoint,
                                        enigo::Key::DBENoRoman,
                                        enigo::Key::DBERoman,
                                        enigo::Key::DBESBCSChar,
                                        enigo::Key::DBESChar,
                                        enigo::Key::Decimal,
                                        enigo::Key::Delete,
                                        enigo::Key::Divide,
                                        enigo::Key::DownArrow,
                                        enigo::Key::End,
                                        enigo::Key::Ereof,
                                        enigo::Key::Escape,
                                        enigo::Key::Execute,
                                        enigo::Key::Exsel,
                                        enigo::Key::F1,
                                        enigo::Key::F2,
                                        enigo::Key::F3,
                                        enigo::Key::F4,
                                        enigo::Key::F5,
                                        enigo::Key::F6,
                                        enigo::Key::F7,
                                        enigo::Key::F8,
                                        enigo::Key::F9,
                                        enigo::Key::F10,
                                        enigo::Key::F11,
                                        enigo::Key::F12,
                                        enigo::Key::F13,
                                        enigo::Key::F14,
                                        enigo::Key::F15,
                                        enigo::Key::F16,
                                        enigo::Key::F17,
                                        enigo::Key::F18,
                                        enigo::Key::F19,
                                        enigo::Key::F20,
                                        enigo::Key::F21,
                                        enigo::Key::F22,
                                        enigo::Key::F23,
                                        enigo::Key::F24,
                                        enigo::Key::Final,
                                        enigo::Key::GamepadA,
                                        enigo::Key::GamepadB,
                                        enigo::Key::GamepadDPadDown,
                                        enigo::Key::GamepadDPadLeft,
                                        enigo::Key::GamepadDPadRight,
                                        enigo::Key::GamepadDPadUp,
                                        enigo::Key::GamepadLeftShoulder,
                                        enigo::Key::GamepadLeftThumbstickButton,
                                        enigo::Key::GamepadLeftThumbstickDown,
                                        enigo::Key::GamepadLeftThumbstickLeft,
                                        enigo::Key::GamepadLeftThumbstickRight,
                                        enigo::Key::GamepadLeftThumbstickUp,
                                        enigo::Key::GamepadLeftTrigger,
                                        enigo::Key::GamepadMenu,
                                        enigo::Key::GamepadRightShoulder,
                                        enigo::Key::GamepadRightThumbstickButton,
                                        enigo::Key::GamepadRightThumbstickDown,
                                        enigo::Key::GamepadRightThumbstickLeft,
                                        enigo::Key::GamepadRightThumbstickRight,
                                        enigo::Key::GamepadRightThumbstickUp,
                                        enigo::Key::GamepadRightTrigger,
                                        enigo::Key::GamepadView,
                                        enigo::Key::GamepadX,
                                        enigo::Key::GamepadY,
                                        enigo::Key::Hangeul,
                                        enigo::Key::Hangul,
                                        enigo::Key::Hanja,
                                        enigo::Key::Help,
                                        enigo::Key::Home,
                                        enigo::Key::Ico00,
                                        enigo::Key::IcoClear,
                                        enigo::Key::IcoHelp,
                                        enigo::Key::IMEOff,
                                        enigo::Key::IMEOn,
                                        enigo::Key::Insert,
                                        enigo::Key::Junja,
                                        enigo::Key::Kana,
                                        enigo::Key::Kanji,
                                        enigo::Key::LaunchApp1,
                                        enigo::Key::LaunchApp2,
                                        enigo::Key::LaunchMail,
                                        enigo::Key::LaunchMediaSelect,
                                        enigo::Key::LButton,
                                        enigo::Key::LControl,
                                        enigo::Key::LeftArrow,
                                        enigo::Key::LMenu,
                                        enigo::Key::LShift,
                                        enigo::Key::LWin,
                                        enigo::Key::MButton,
                                        enigo::Key::MediaNextTrack,
                                        enigo::Key::MediaPlayPause,
                                        enigo::Key::MediaPrevTrack,
                                        enigo::Key::MediaStop,
                                        // meta key (also known as "windows", "super", and "command")
                                        enigo::Key::Meta,
                                        enigo::Key::ModeChange,
                                        enigo::Key::Multiply,
                                        enigo::Key::NavigationAccept,
                                        enigo::Key::NavigationCancel,
                                        enigo::Key::NavigationDown,
                                        enigo::Key::NavigationLeft,
                                        enigo::Key::NavigationMenu,
                                        enigo::Key::NavigationRight,
                                        enigo::Key::NavigationUp,
                                        enigo::Key::NavigationView,
                                        enigo::Key::NoName,
                                        enigo::Key::NonConvert,
                                        enigo::Key::None,
                                        enigo::Key::Numlock,
                                        enigo::Key::Numpad0,
                                        enigo::Key::Numpad1,
                                        enigo::Key::Numpad2,
                                        enigo::Key::Numpad3,
                                        enigo::Key::Numpad4,
                                        enigo::Key::Numpad5,
                                        enigo::Key::Numpad6,
                                        enigo::Key::Numpad7,
                                        enigo::Key::Numpad8,
                                        enigo::Key::Numpad9,
                                        enigo::Key::OEM1,
                                        enigo::Key::OEM102,
                                        enigo::Key::OEM2,
                                        enigo::Key::OEM3,
                                        enigo::Key::OEM4,
                                        enigo::Key::OEM5,
                                        enigo::Key::OEM6,
                                        enigo::Key::OEM7,
                                        enigo::Key::OEM8,
                                        enigo::Key::OEMAttn,
                                        enigo::Key::OEMAuto,
                                        enigo::Key::OEMAx,
                                        enigo::Key::OEMBacktab,
                                        enigo::Key::OEMClear,
                                        enigo::Key::OEMComma,
                                        enigo::Key::OEMCopy,
                                        enigo::Key::OEMCusel,
                                        enigo::Key::OEMEnlw,
                                        enigo::Key::OEMFinish,
                                        enigo::Key::OEMFJJisho,
                                        enigo::Key::OEMFJLoya,
                                        enigo::Key::OEMFJMasshou,
                                        enigo::Key::OEMFJRoya,
                                        enigo::Key::OEMFJTouroku,
                                        enigo::Key::OEMJump,
                                        enigo::Key::OEMMinus,
                                        enigo::Key::OEMNECEqual,
                                        enigo::Key::OEMPA1,
                                        enigo::Key::OEMPA2,
                                        enigo::Key::OEMPA3,
                                        enigo::Key::OEMPeriod,
                                        enigo::Key::OEMPlus,
                                        enigo::Key::OEMReset,
                                        enigo::Key::OEMWsctrl,
                                        enigo::Key::PA1,
                                        enigo::Key::Packet,
                                        enigo::Key::PageDown,
                                        enigo::Key::PageUp,
                                        enigo::Key::Pause,
                                        enigo::Key::Play,
                                        enigo::Key::Processkey,
                                        enigo::Key::RButton,
                                        enigo::Key::RControl,
                                        enigo::Key::Return,
                                        enigo::Key::RightArrow,
                                        enigo::Key::RMenu,
                                        enigo::Key::RShift,
                                        enigo::Key::RWin,
                                        enigo::Key::Scroll,
                                        enigo::Key::Select,
                                        enigo::Key::Separator,
                                        enigo::Key::Shift,
                                        enigo::Key::Sleep,
                                        enigo::Key::PrintScr,
                                        enigo::Key::Space,
                                        enigo::Key::Subtract,
                                        enigo::Key::Tab,
                                        enigo::Key::UpArrow,
                                        enigo::Key::VolumeDown,
                                        enigo::Key::VolumeMute,
                                        enigo::Key::VolumeUp,
                                        enigo::Key::XButton1,
                                        enigo::Key::XButton2,
                                        enigo::Key::Zoom,
                                    ];

                                    static ALL_BUTTONS: [enigo::Button; 9] = [
                                        enigo::Button::Left,
                                        enigo::Button::Middle,
                                        enigo::Button::Right,
                                        enigo::Button::Back,
                                        enigo::Button::Forward,
                                        enigo::Button::ScrollUp,
                                        enigo::Button::ScrollDown,
                                        enigo::Button::ScrollLeft,
                                        enigo::Button::ScrollRight,
                                    ];

                                    for button in ALL_BUTTONS {
                                        ui.selectable_value(
                                            &mut val.keybutton,
                                            button.into(),
                                            format!("{:?}", button),
                                        );
                                    }

                                    for key in ALL_KEYS {
                                        ui.selectable_value(
                                            &mut val.keybutton,
                                            key.into(),
                                            format!("{:?}", key),
                                        );
                                    }
                                });

                                ui.add_space(10.0);
                                ui.vertical(|ui| {
                                    if ui.checkbox(&mut val.down, "Key-Down").clicked() {
                                        if !val.down && !val.up {
                                            val.up = true;
                                        }
                                    };
                                    if ui.checkbox(&mut val.up, "Key-Up").clicked() {
                                        if !val.down && !val.up {
                                            val.down = true;
                                        }
                                    };
                                });
                                if val.down && val.up {
                                    ui.label("Hold-Time (ms): ");
                                    ui.add(egui::DragValue::new(&mut val.hold_time_ms));
                                    ui.add_space(10.0);
                                } else {
                                    ui.add_space(157.0);
                                }
                                ui.label("Delay after (ms): ");
                                ui.add(egui::DragValue::new(&mut val.delay_after_ms));
                            }
                            Action::Delay(val) => {
                                if ui.checkbox(&mut val.random, "Random").clicked() {
                                    if val.delay_ms_max < val.delay_ms_min {
                                        val.delay_ms_max = val.delay_ms_min;
                                    }
                                }
                                if val.random {
                                    ui.label("Delay min (ms): ");
                                    if ui
                                        .add(egui::DragValue::new(&mut val.delay_ms_min))
                                        .changed()
                                    {
                                        if val.delay_ms_min > val.delay_ms_max {
                                            val.delay_ms_min = val.delay_ms_max;
                                        }
                                    }
                                    ui.label("Delay max (ms): ");
                                    if ui
                                        .add(egui::DragValue::new(&mut val.delay_ms_max))
                                        .changed()
                                    {
                                        if val.delay_ms_max < val.delay_ms_min {
                                            val.delay_ms_max = val.delay_ms_min;
                                        }
                                    }
                                    ui.add_space(175.0);
                                } else {
                                    ui.label("Delay (ms): ");
                                    ui.add(egui::DragValue::new(&mut val.delay_ms_min));
                                    ui.add_space(345.0);
                                }
                            }
                        }
                        let trash_icon = egui::include_image!("../assets/trash.svg");
                        if ui
                            .add(egui::Button::image_and_text(trash_icon, ""))
                            .clicked()
                        {
                            index_to_rm = Some(pos);
                        }
                    });
                });
            }
            add_add_buttons(ui, depth, loopaction);
        });
    });

    if index_to_rm.is_some() {
        loopaction.actions.remove(index_to_rm.unwrap());
    }
}

fn add_add_buttons(ui: &mut Ui, depth: u16, loopaction: &mut LoopAction) {
    ui.horizontal(|ui| {
        ui.add_space(20.0 * f32::from(depth));
        if ui.button("Add Key Press").clicked() {
            loopaction.actions.push(
                PressAction {
                    keybutton: enigo::Key::None.into(),
                    down: true,
                    up: true,
                    hold_time_ms: 0,
                    delay_after_ms: 1,
                }
                .into(),
            );
        }
        if ui.button("Add Mouse Move").clicked() {
            loopaction.actions.push(
                MoveAction {
                    x: 0,
                    y: 0,
                    relative: false,
                    _move_time_ms: 0,
                    delay_after_ms: 1,
                }
                .into(),
            )
        }
        if ui.button("Add Delay").clicked() {
            loopaction.actions.push(
                DelayAction {
                    random: false,
                    delay_ms_min: 1,
                    delay_ms_max: 2,
                }
                .into(),
            )
        }
        if ui.button("Add Loop").clicked() {
            loopaction.actions.push(
                LoopAction {
                    infinite: false,
                    iterations: 1,
                    actions: Vec::new(),
                }
                .into(),
            )
        }
    });
}
