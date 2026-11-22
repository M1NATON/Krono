use crate::i18n::Lang;
use eframe::egui;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KeyCode {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    Num0,
    Num1,
    Num2,
    Num3,
    Num4,
    Num5,
    Num6,
    Num7,
    Num8,
    Num9,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    Tab,
    Space,
    Enter,
    Backspace,
    Delete,
    Backslash,
    Comma,
    Minus,
    Equals,
    Plus,
    Slash,
    Semicolon,
    OpenBracket,
    CloseBracket,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
}

impl KeyCode {
    pub fn from_egui(key: egui::Key) -> Option<Self> {
        Some(match key {
            egui::Key::A => Self::A,
            egui::Key::B => Self::B,
            egui::Key::C => Self::C,
            egui::Key::D => Self::D,
            egui::Key::E => Self::E,
            egui::Key::F => Self::F,
            egui::Key::G => Self::G,
            egui::Key::H => Self::H,
            egui::Key::I => Self::I,
            egui::Key::J => Self::J,
            egui::Key::K => Self::K,
            egui::Key::L => Self::L,
            egui::Key::M => Self::M,
            egui::Key::N => Self::N,
            egui::Key::O => Self::O,
            egui::Key::P => Self::P,
            egui::Key::Q => Self::Q,
            egui::Key::R => Self::R,
            egui::Key::S => Self::S,
            egui::Key::T => Self::T,
            egui::Key::U => Self::U,
            egui::Key::V => Self::V,
            egui::Key::W => Self::W,
            egui::Key::X => Self::X,
            egui::Key::Y => Self::Y,
            egui::Key::Z => Self::Z,
            egui::Key::Num0 => Self::Num0,
            egui::Key::Num1 => Self::Num1,
            egui::Key::Num2 => Self::Num2,
            egui::Key::Num3 => Self::Num3,
            egui::Key::Num4 => Self::Num4,
            egui::Key::Num5 => Self::Num5,
            egui::Key::Num6 => Self::Num6,
            egui::Key::Num7 => Self::Num7,
            egui::Key::Num8 => Self::Num8,
            egui::Key::Num9 => Self::Num9,
            egui::Key::F1 => Self::F1,
            egui::Key::F2 => Self::F2,
            egui::Key::F3 => Self::F3,
            egui::Key::F4 => Self::F4,
            egui::Key::F5 => Self::F5,
            egui::Key::F6 => Self::F6,
            egui::Key::F7 => Self::F7,
            egui::Key::F8 => Self::F8,
            egui::Key::F9 => Self::F9,
            egui::Key::F10 => Self::F10,
            egui::Key::F11 => Self::F11,
            egui::Key::F12 => Self::F12,
            egui::Key::Tab => Self::Tab,
            egui::Key::Space => Self::Space,
            egui::Key::Enter => Self::Enter,
            egui::Key::Backspace => Self::Backspace,
            egui::Key::Delete => Self::Delete,
            egui::Key::Backslash => Self::Backslash,
            egui::Key::Comma => Self::Comma,
            egui::Key::Minus => Self::Minus,
            egui::Key::Equals => Self::Equals,
            egui::Key::Plus => Self::Plus,
            egui::Key::Slash => Self::Slash,
            egui::Key::Semicolon => Self::Semicolon,
            egui::Key::OpenBracket => Self::OpenBracket,
            egui::Key::CloseBracket => Self::CloseBracket,
            egui::Key::ArrowLeft => Self::ArrowLeft,
            egui::Key::ArrowRight => Self::ArrowRight,
            egui::Key::ArrowUp => Self::ArrowUp,
            egui::Key::ArrowDown => Self::ArrowDown,
            _ => return None,
        })
    }

    pub fn to_egui(self) -> egui::Key {
        match self {
            Self::A => egui::Key::A,
            Self::B => egui::Key::B,
            Self::C => egui::Key::C,
            Self::D => egui::Key::D,
            Self::E => egui::Key::E,
            Self::F => egui::Key::F,
            Self::G => egui::Key::G,
            Self::H => egui::Key::H,
            Self::I => egui::Key::I,
            Self::J => egui::Key::J,
            Self::K => egui::Key::K,
            Self::L => egui::Key::L,
            Self::M => egui::Key::M,
            Self::N => egui::Key::N,
            Self::O => egui::Key::O,
            Self::P => egui::Key::P,
            Self::Q => egui::Key::Q,
            Self::R => egui::Key::R,
            Self::S => egui::Key::S,
            Self::T => egui::Key::T,
            Self::U => egui::Key::U,
            Self::V => egui::Key::V,
            Self::W => egui::Key::W,
            Self::X => egui::Key::X,
            Self::Y => egui::Key::Y,
            Self::Z => egui::Key::Z,
            Self::Num0 => egui::Key::Num0,
            Self::Num1 => egui::Key::Num1,
            Self::Num2 => egui::Key::Num2,
            Self::Num3 => egui::Key::Num3,
            Self::Num4 => egui::Key::Num4,
            Self::Num5 => egui::Key::Num5,
            Self::Num6 => egui::Key::Num6,
            Self::Num7 => egui::Key::Num7,
            Self::Num8 => egui::Key::Num8,
            Self::Num9 => egui::Key::Num9,
            Self::F1 => egui::Key::F1,
            Self::F2 => egui::Key::F2,
            Self::F3 => egui::Key::F3,
            Self::F4 => egui::Key::F4,
            Self::F5 => egui::Key::F5,
            Self::F6 => egui::Key::F6,
            Self::F7 => egui::Key::F7,
            Self::F8 => egui::Key::F8,
            Self::F9 => egui::Key::F9,
            Self::F10 => egui::Key::F10,
            Self::F11 => egui::Key::F11,
            Self::F12 => egui::Key::F12,
            Self::Tab => egui::Key::Tab,
            Self::Space => egui::Key::Space,
            Self::Enter => egui::Key::Enter,
            Self::Backspace => egui::Key::Backspace,
            Self::Delete => egui::Key::Delete,
            Self::Backslash => egui::Key::Backslash,
            Self::Comma => egui::Key::Comma,
            Self::Minus => egui::Key::Minus,
            Self::Equals => egui::Key::Equals,
            Self::Plus => egui::Key::Plus,
            Self::Slash => egui::Key::Slash,
            Self::Semicolon => egui::Key::Semicolon,
            Self::OpenBracket => egui::Key::OpenBracket,
            Self::CloseBracket => egui::Key::CloseBracket,
            Self::ArrowLeft => egui::Key::ArrowLeft,
            Self::ArrowRight => egui::Key::ArrowRight,
            Self::ArrowUp => egui::Key::ArrowUp,
            Self::ArrowDown => egui::Key::ArrowDown,
        }
    }

    pub fn is_function_key(self) -> bool {
        matches!(
            self,
            Self::F1
                | Self::F2
                | Self::F3
                | Self::F4
                | Self::F5
                | Self::F6
                | Self::F7
                | Self::F8
                | Self::F9
                | Self::F10
                | Self::F11
                | Self::F12
        )
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
            Self::D => "D",
            Self::E => "E",
            Self::F => "F",
            Self::G => "G",
            Self::H => "H",
            Self::I => "I",
            Self::J => "J",
            Self::K => "K",
            Self::L => "L",
            Self::M => "M",
            Self::N => "N",
            Self::O => "O",
            Self::P => "P",
            Self::Q => "Q",
            Self::R => "R",
            Self::S => "S",
            Self::T => "T",
            Self::U => "U",
            Self::V => "V",
            Self::W => "W",
            Self::X => "X",
            Self::Y => "Y",
            Self::Z => "Z",
            Self::Num0 => "0",
            Self::Num1 => "1",
            Self::Num2 => "2",
            Self::Num3 => "3",
            Self::Num4 => "4",
            Self::Num5 => "5",
            Self::Num6 => "6",
            Self::Num7 => "7",
            Self::Num8 => "8",
            Self::Num9 => "9",
            Self::F1 => "F1",
            Self::F2 => "F2",
            Self::F3 => "F3",
            Self::F4 => "F4",
            Self::F5 => "F5",
            Self::F6 => "F6",
            Self::F7 => "F7",
            Self::F8 => "F8",
            Self::F9 => "F9",
            Self::F10 => "F10",
            Self::F11 => "F11",
            Self::F12 => "F12",
            Self::Tab => "Tab",
            Self::Space => "Space",
            Self::Enter => "Enter",
            Self::Backspace => "Backspace",
            Self::Delete => "Delete",
            Self::Backslash => "\\",
            Self::Comma => ",",
            Self::Minus => "-",
            Self::Equals => "=",
            Self::Plus => "+",
            Self::Slash => "/",
            Self::Semicolon => ";",
            Self::OpenBracket => "[",
            Self::CloseBracket => "]",
            Self::ArrowLeft => "←",
            Self::ArrowRight => "→",
            Self::ArrowUp => "↑",
            Self::ArrowDown => "↓",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KeyCombo {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub key: KeyCode,
}

impl KeyCombo {
    pub const fn new(ctrl: bool, shift: bool, alt: bool, key: KeyCode) -> Self {
        Self {
            ctrl,
            shift,
            alt,
            key,
        }
    }

    pub fn format_display(self) -> String {
        let mut parts: Vec<&'static str> = Vec::with_capacity(4);
        if self.ctrl {
            parts.push(if cfg!(target_os = "macos") { "⌘" } else { "Ctrl" });
        }
        if self.shift {
            parts.push("Shift");
        }
        if self.alt {
            parts.push(if cfg!(target_os = "macos") { "⌥" } else { "Alt" });
        }
        parts.push(self.key.label());
        parts.join("+")
    }

    pub fn matches_input(self, i: &egui::InputState) -> bool {
        let cmd = i.modifiers.ctrl || i.modifiers.command;
        if cmd != self.ctrl || i.modifiers.shift != self.shift || i.modifiers.alt != self.alt {
            return false;
        }
        i.key_pressed(self.key.to_egui())
            || (self.key == KeyCode::Equals && i.key_pressed(egui::Key::Plus))
    }

    pub fn is_safe_when_typing(self) -> bool {
        self.ctrl || self.alt || self.key.is_function_key()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShortcutCategory {
    TabsNav,
    FocusZones,
    SearchCreate,
    EditorView,
}

impl ShortcutCategory {
    pub const ALL: &'static [Self] = &[
        Self::TabsNav,
        Self::FocusZones,
        Self::SearchCreate,
        Self::EditorView,
    ];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ShortcutAction {
    NextTab,
    PrevTab,
    CloseTab,
    NavBack,
    NavForward,
    FocusSidebar,
    FocusEditor,
    FocusInspector,
    CommandPalette,
    NewItem,
    DailyNote,
    GlobalReplace,
    QuickScratchpad,
    SaveDocument,
    SyncNow,
    ToggleSidebar,
    ToggleInspector,
    ToggleGraph,
    ToggleSplitView,
    ToggleZenMode,
    OpenSettings,
    ZoomIn,
    ZoomOut,
    ZoomReset,
}

impl ShortcutAction {
    pub const ALL: &'static [Self] = &[
        Self::NextTab,
        Self::PrevTab,
        Self::CloseTab,
        Self::NavBack,
        Self::NavForward,
        Self::FocusSidebar,
        Self::FocusEditor,
        Self::FocusInspector,
        Self::CommandPalette,
        Self::NewItem,
        Self::DailyNote,
        Self::GlobalReplace,
        Self::QuickScratchpad,
        Self::SaveDocument,
        Self::SyncNow,
        Self::ToggleSidebar,
        Self::ToggleInspector,
        Self::ToggleGraph,
        Self::ToggleSplitView,
        Self::ToggleZenMode,
        Self::OpenSettings,
        Self::ZoomIn,
        Self::ZoomOut,
        Self::ZoomReset,
    ];

    pub fn category(self) -> ShortcutCategory {
        match self {
            Self::NextTab | Self::PrevTab | Self::CloseTab | Self::NavBack | Self::NavForward => {
                ShortcutCategory::TabsNav
            }
            Self::FocusSidebar | Self::FocusEditor | Self::FocusInspector => {
                ShortcutCategory::FocusZones
            }
            Self::CommandPalette
            | Self::NewItem
            | Self::DailyNote
            | Self::GlobalReplace
            | Self::QuickScratchpad => ShortcutCategory::SearchCreate,
            Self::SaveDocument
            | Self::SyncNow
            | Self::ToggleSidebar
            | Self::ToggleInspector
            | Self::ToggleGraph
            | Self::ToggleSplitView
            | Self::ToggleZenMode
            | Self::OpenSettings
            | Self::ZoomIn
            | Self::ZoomOut
            | Self::ZoomReset => ShortcutCategory::EditorView,
        }
    }

    pub const fn default_combo(self) -> KeyCombo {
        match self {
            Self::NextTab => KeyCombo::new(true, false, false, KeyCode::Tab),
            Self::PrevTab => KeyCombo::new(true, true, false, KeyCode::Tab),
            Self::CloseTab => KeyCombo::new(true, false, false, KeyCode::W),
            Self::NavBack => KeyCombo::new(false, false, true, KeyCode::ArrowLeft),
            Self::NavForward => KeyCombo::new(false, false, true, KeyCode::ArrowRight),
            Self::FocusSidebar => KeyCombo::new(true, true, false, KeyCode::E),
            Self::FocusEditor => KeyCombo::new(false, false, true, KeyCode::Num2),
            Self::FocusInspector => KeyCombo::new(false, false, true, KeyCode::Num3),
            Self::CommandPalette => KeyCombo::new(true, false, false, KeyCode::K),
            Self::NewItem => KeyCombo::new(true, false, false, KeyCode::N),
            Self::DailyNote => KeyCombo::new(true, false, false, KeyCode::D),
            Self::GlobalReplace => KeyCombo::new(true, true, false, KeyCode::H),
            Self::QuickScratchpad => KeyCombo::new(false, false, true, KeyCode::N),
            Self::SaveDocument => KeyCombo::new(true, false, false, KeyCode::S),
            Self::SyncNow => KeyCombo::new(true, true, false, KeyCode::S),
            Self::ToggleSidebar => KeyCombo::new(true, false, false, KeyCode::Backslash),
            Self::ToggleInspector => KeyCombo::new(true, true, false, KeyCode::I),
            Self::ToggleGraph => KeyCombo::new(true, false, false, KeyCode::G),
            Self::ToggleSplitView => KeyCombo::new(true, true, false, KeyCode::Backslash),
            Self::ToggleZenMode => KeyCombo::new(false, false, false, KeyCode::F11),
            Self::OpenSettings => KeyCombo::new(true, false, false, KeyCode::Comma),
            Self::ZoomIn => KeyCombo::new(true, false, false, KeyCode::Equals),
            Self::ZoomOut => KeyCombo::new(true, false, false, KeyCode::Minus),
            Self::ZoomReset => KeyCombo::new(true, false, false, KeyCode::Num0),
        }
    }

    pub fn label(self, lang: Lang) -> &'static str {
        match (self, lang) {
            (Self::NextTab, Lang::Ru) => "Следующая вкладка",
            (Self::NextTab, Lang::En) => "Next tab",
            (Self::PrevTab, Lang::Ru) => "Предыдущая вкладка",
            (Self::PrevTab, Lang::En) => "Previous tab",
            (Self::CloseTab, Lang::Ru) => "Закрыть активную вкладку",
            (Self::CloseTab, Lang::En) => "Close active tab",
            (Self::NavBack, Lang::Ru) => "Назад по истории",
            (Self::NavBack, Lang::En) => "Navigate back",
            (Self::NavForward, Lang::Ru) => "Вперёд по истории",
            (Self::NavForward, Lang::En) => "Navigate forward",
            (Self::FocusSidebar, Lang::Ru) => "Фокус на сайдбар",
            (Self::FocusSidebar, Lang::En) => "Focus sidebar",
            (Self::FocusEditor, Lang::Ru) => "Фокус на редактор",
            (Self::FocusEditor, Lang::En) => "Focus editor",
            (Self::FocusInspector, Lang::Ru) => "Фокус на инспектор",
            (Self::FocusInspector, Lang::En) => "Focus inspector",
            (Self::CommandPalette, Lang::Ru) => "Быстрый поиск и команды",
            (Self::CommandPalette, Lang::En) => "Quick search & commands",
            (Self::NewItem, Lang::Ru) => "Новая заметка / элемент",
            (Self::NewItem, Lang::En) => "New note / item",
            (Self::DailyNote, Lang::Ru) => "Заметка дня",
            (Self::DailyNote, Lang::En) => "Today's daily note",
            (Self::GlobalReplace, Lang::Ru) => "Глобальный поиск и замена",
            (Self::GlobalReplace, Lang::En) => "Global find & replace",
            (Self::QuickScratchpad, Lang::Ru) => "Быстрый черновик (Scratchpad)",
            (Self::QuickScratchpad, Lang::En) => "Quick Scratchpad",
            (Self::SaveDocument, Lang::Ru) => "Сохранить текущий файл",
            (Self::SaveDocument, Lang::En) => "Save current file",
            (Self::SyncNow, Lang::Ru) => "Синхронизация с GitHub",
            (Self::SyncNow, Lang::En) => "Sync with GitHub",
            (Self::ToggleSidebar, Lang::Ru) => "Показать / скрыть сайдбар",
            (Self::ToggleSidebar, Lang::En) => "Toggle sidebar",
            (Self::ToggleInspector, Lang::Ru) => "Показать / скрыть инспектор",
            (Self::ToggleInspector, Lang::En) => "Toggle inspector",
            (Self::ToggleGraph, Lang::Ru) => "Граф знаний",
            (Self::ToggleGraph, Lang::En) => "Knowledge graph",
            (Self::ToggleSplitView, Lang::Ru) => "Split View (вторая панель)",
            (Self::ToggleSplitView, Lang::En) => "Toggle Split View",
            (Self::ToggleZenMode, Lang::Ru) => "Режим фокуса (Zen Mode)",
            (Self::ToggleZenMode, Lang::En) => "Toggle Zen Mode",
            (Self::OpenSettings, Lang::Ru) => "Открыть / закрыть настройки",
            (Self::OpenSettings, Lang::En) => "Open / close settings",
            (Self::ZoomIn, Lang::Ru) => "Увеличить масштаб",
            (Self::ZoomIn, Lang::En) => "Zoom in",
            (Self::ZoomOut, Lang::Ru) => "Уменьшить масштаб",
            (Self::ZoomOut, Lang::En) => "Zoom out",
            (Self::ZoomReset, Lang::Ru) => "Сбросить масштаб",
            (Self::ZoomReset, Lang::En) => "Reset zoom",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct KeybindingsConfig {
    pub overrides: BTreeMap<ShortcutAction, KeyCombo>,
}

impl KeybindingsConfig {
    pub fn combo_for(&self, action: ShortcutAction) -> KeyCombo {
        self.overrides
            .get(&action)
            .copied()
            .unwrap_or_else(|| action.default_combo())
    }

    pub fn set(&mut self, action: ShortcutAction, combo: KeyCombo) {
        if combo == action.default_combo() {
            self.overrides.remove(&action);
        } else {
            self.overrides.insert(action, combo);
        }
    }

    pub fn reset(&mut self, action: ShortcutAction) {
        self.overrides.remove(&action);
    }

    pub fn reset_all(&mut self) {
        self.overrides.clear();
    }

    pub fn is_customized(&self, action: ShortcutAction) -> bool {
        self.overrides.contains_key(&action)
    }

    pub fn has_any_overrides(&self) -> bool {
        !self.overrides.is_empty()
    }
}

/// Returns a map from each conflicting `ShortcutAction` to the list of other actions sharing its `KeyCombo`.
pub fn find_conflicts(cfg: &KeybindingsConfig) -> HashMap<ShortcutAction, Vec<ShortcutAction>> {
    let mut by_combo: HashMap<KeyCombo, Vec<ShortcutAction>> = HashMap::new();
    for &action in ShortcutAction::ALL {
        by_combo
            .entry(cfg.combo_for(action))
            .or_default()
            .push(action);
    }

    let mut conflicts: HashMap<ShortcutAction, Vec<ShortcutAction>> = HashMap::new();
    for group in by_combo.into_values() {
        if group.len() < 2 {
            continue;
        }
        for &action in &group {
            let others: Vec<ShortcutAction> = group
                .iter()
                .copied()
                .filter(|&other| other != action)
                .collect();
            conflicts.insert(action, others);
        }
    }
    conflicts
}

/// Captures a new key combination during Settings recording mode.
pub fn capture_pressed_combo(i: &egui::InputState) -> Option<KeyCombo> {
    for ev in &i.events {
        if let egui::Event::Key {
            key,
            pressed: true,
            modifiers,
            ..
        } = ev
        {
            if *key == egui::Key::Escape {
                continue;
            }
            let Some(code) = KeyCode::from_egui(*key) else {
                continue;
            };
            let ctrl = modifiers.ctrl || modifiers.command || i.modifiers.ctrl || i.modifiers.command;
            let shift = modifiers.shift || i.modifiers.shift;
            let alt = modifiers.alt || i.modifiers.alt;
            if ctrl || shift || alt || code.is_function_key() {
                return Some(KeyCombo::new(ctrl, shift, alt, code));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_keybindings_have_zero_conflicts() {
        let cfg = KeybindingsConfig::default();
        let conflicts = find_conflicts(&cfg);
        assert!(conflicts.is_empty(), "expected 0 conflicts, got: {conflicts:?}");
    }

    #[test]
    fn find_conflicts_detects_two_and_three_way_collisions() {
        let mut cfg = KeybindingsConfig::default();
        let combo = KeyCombo::new(true, false, false, KeyCode::K);
        cfg.set(ShortcutAction::CloseTab, combo);
        let two = find_conflicts(&cfg);
        assert_eq!(two.len(), 2);
        assert_eq!(two[&ShortcutAction::CommandPalette], vec![ShortcutAction::CloseTab]);
        assert_eq!(two[&ShortcutAction::CloseTab], vec![ShortcutAction::CommandPalette]);

        cfg.set(ShortcutAction::NewItem, combo);
        let three = find_conflicts(&cfg);
        assert_eq!(three.len(), 3);
        assert_eq!(three[&ShortcutAction::NewItem].len(), 2);
    }

    #[test]
    fn key_combo_formats_and_roundtrips_json() {
        let c1 = KeyCombo::new(true, true, false, KeyCode::E);
        if cfg!(target_os = "macos") {
            assert_eq!(c1.format_display(), "⌘+Shift+E");
        } else {
            assert_eq!(c1.format_display(), "Ctrl+Shift+E");
        }
        let c2 = KeyCombo::new(false, false, true, KeyCode::Num1);
        if cfg!(target_os = "macos") {
            assert_eq!(c2.format_display(), "⌥+1");
        } else {
            assert_eq!(c2.format_display(), "Alt+1");
        }
        assert_eq!(KeyCombo::new(false, false, false, KeyCode::F11).format_display(), "F11");

        let mut cfg = KeybindingsConfig::default();
        cfg.set(ShortcutAction::CloseTab, c1);
        let json = serde_json::to_string(&cfg).unwrap();
        let decoded: KeybindingsConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, cfg);
        assert_eq!(decoded.combo_for(ShortcutAction::CloseTab), c1);
    }
}
