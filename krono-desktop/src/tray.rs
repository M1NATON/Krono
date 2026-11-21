use crate::i18n::Lang;
use eframe::egui;
use std::sync::mpsc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayCommand {
    OpenScratchpad,
    OpenTodayDaily,
    ShowMainWindow,
    Quit,
}

pub struct SystemTrayHandle {
    rx: mpsc::Receiver<TrayCommand>,
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    _tray: Option<tray_icon::TrayIcon>,
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    _hotkey_mgr: Option<global_hotkey::GlobalHotKeyManager>,
}

impl SystemTrayHandle {
    pub fn start(ctx: egui::Context, lang: Lang) -> Self {
        #[cfg(any(target_os = "windows", target_os = "macos"))]
        {
            Self::start_native(ctx, lang)
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            let _ = (ctx, lang);
            let (_tx, rx) = mpsc::channel();
            Self { rx }
        }
    }

    pub fn poll(&self) -> Vec<TrayCommand> {
        let mut out = Vec::new();
        while let Ok(cmd) = self.rx.try_recv() {
            out.push(cmd);
        }
        out
    }

    #[cfg(any(target_os = "windows", target_os = "macos"))]
    fn start_native(ctx: egui::Context, lang: Lang) -> Self {
        use global_hotkey::hotkey::{Code, HotKey, Modifiers};
        use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
        use tray_icon::menu::{Menu, MenuEvent, MenuItem};
        use tray_icon::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

        let (tx, rx) = mpsc::channel();

        let (label_scratch, label_daily, label_show, label_quit) = match lang {
            Lang::Ru => (
                "Быстрый захват (Ctrl+Shift+Space)",
                "Сегодняшний дневник",
                "Открыть Krono",
                "Выход",
            ),
            Lang::En => (
                "Scratchpad (Ctrl+Shift+Space)",
                "Today's Daily Note",
                "Show Krono",
                "Quit",
            ),
        };

        let item_scratch = MenuItem::new(label_scratch, true, None);
        let item_daily = MenuItem::new(label_daily, true, None);
        let item_show = MenuItem::new(label_show, true, None);
        let item_quit = MenuItem::new(label_quit, true, None);

        let id_scratch = item_scratch.id().clone();
        let id_daily = item_daily.id().clone();
        let id_show = item_show.id().clone();
        let id_quit = item_quit.id().clone();

        let menu = Menu::new();
        let _ = menu.append(&item_scratch);
        let _ = menu.append(&item_daily);
        let _ = menu.append(&item_show);
        let _ = menu.append(&item_quit);

        let tray_icon_img = load_tray_icon();
        let tray = tray_icon_img.and_then(|icon| {
            TrayIconBuilder::new()
                .with_menu(Box::new(menu))
                .with_tooltip("Krono")
                .with_icon(icon)
                .build()
                .ok()
        });

        let mut hotkey_id: Option<u32> = None;
        let hotkey_mgr = GlobalHotKeyManager::new().ok().and_then(|mgr| {
            let primary = HotKey::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::Space);
            if mgr.register(primary).is_ok() {
                hotkey_id = Some(primary.id());
                Some(mgr)
            } else {
                let fallback = HotKey::new(Some(Modifiers::ALT), Code::KeyN);
                if mgr.register(fallback).is_ok() {
                    hotkey_id = Some(fallback.id());
                    Some(mgr)
                } else {
                    None
                }
            }
        });

        std::thread::spawn(move || {
            let menu_rx = MenuEvent::receiver();
            let tray_rx = TrayIconEvent::receiver();
            let hk_rx = GlobalHotKeyEvent::receiver();

            loop {
                let mut woke = false;
                while let Ok(ev) = menu_rx.try_recv() {
                    let cmd = if ev.id == id_scratch {
                        Some(TrayCommand::OpenScratchpad)
                    } else if ev.id == id_daily {
                        Some(TrayCommand::OpenTodayDaily)
                    } else if ev.id == id_show {
                        Some(TrayCommand::ShowMainWindow)
                    } else if ev.id == id_quit {
                        Some(TrayCommand::Quit)
                    } else {
                        None
                    };
                    if let Some(c) = cmd {
                        if tx.send(c).is_err() {
                            return;
                        }
                        woke = true;
                    }
                }

                while let Ok(ev) = tray_rx.try_recv() {
                    let clicked = matches!(
                        ev,
                        TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        } | TrayIconEvent::DoubleClick {
                            button: MouseButton::Left,
                            ..
                        }
                    );
                    if clicked {
                        if tx.send(TrayCommand::ShowMainWindow).is_err() {
                            return;
                        }
                        woke = true;
                    }
                }

                while let Ok(ev) = hk_rx.try_recv() {
                    if ev.state == HotKeyState::Pressed
                        && hotkey_id.is_some_and(|id| ev.id == id)
                    {
                        if tx.send(TrayCommand::OpenScratchpad).is_err() {
                            return;
                        }
                        woke = true;
                    }
                }

                if woke {
                    ctx.request_repaint();
                }

                std::thread::sleep(std::time::Duration::from_millis(60));
            }
        });

        Self {
            rx,
            _tray: tray,
            _hotkey_mgr: hotkey_mgr,
        }
    }
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn load_tray_icon() -> Option<tray_icon::Icon> {
    let bytes = include_bytes!("../assets/tray_icon.png");
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    tray_icon::Icon::from_rgba(img.into_raw(), w, h).ok()
}
