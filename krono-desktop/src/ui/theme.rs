use crate::config::{AccentPreset, ThemePreset};
use crate::ui::motion::{lerp_color, lerp_f32};
use eframe::egui::{
    self, epaint::Shadow, vec2, Color32, FontData, FontDefinitions, FontFamily, FontId, Rounding,
    Stroke,
};
use krono_core::{collection::ColorToken, note::NoteStatus};

#[derive(Clone, Copy, Debug)]
pub struct ThemePalette {
    pub is_dark: bool,
    pub bg_sidebar: Color32,
    pub bg_canvas: Color32,
    pub bg_elevated: Color32,
    pub bg_card: Color32,
    pub bg_input: Color32,
    pub bg_hover: Color32,
    pub bg_selected: Color32,
    pub bg_code: Color32,
    pub border_hairline: Color32,
    pub border_subtle: Color32,
    pub border_strong: Color32,
    pub text_primary: Color32,
    pub text_secondary: Color32,
    pub text_muted: Color32,
    pub accent: Color32,
    pub accent_hover: Color32,
    pub accent_soft: Color32,
    pub accent_text: Color32,
    pub accent_on_fill: Color32,
    pub danger: Color32,
    pub danger_soft: Color32,
    pub warning: Color32,
    pub warning_soft: Color32,
    pub success: Color32,
    pub success_soft: Color32,
    pub shadow_elevated: Shadow,
    pub shadow_floating: Shadow,
}

impl ThemePalette {
    pub fn lerp(a: &Self, b: &Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let lerp_shadow = |sa: Shadow, sb: Shadow| Shadow {
            offset: vec2(
                lerp_f32(sa.offset.x, sb.offset.x, t),
                lerp_f32(sa.offset.y, sb.offset.y, t),
            ),
            blur: lerp_f32(sa.blur, sb.blur, t),
            spread: lerp_f32(sa.spread, sb.spread, t),
            color: lerp_color(sa.color, sb.color, t),
        };
        Self {
            is_dark: if t < 0.5 { a.is_dark } else { b.is_dark },
            bg_sidebar: lerp_color(a.bg_sidebar, b.bg_sidebar, t),
            bg_canvas: lerp_color(a.bg_canvas, b.bg_canvas, t),
            bg_elevated: lerp_color(a.bg_elevated, b.bg_elevated, t),
            bg_card: lerp_color(a.bg_card, b.bg_card, t),
            bg_input: lerp_color(a.bg_input, b.bg_input, t),
            bg_hover: lerp_color(a.bg_hover, b.bg_hover, t),
            bg_selected: lerp_color(a.bg_selected, b.bg_selected, t),
            bg_code: lerp_color(a.bg_code, b.bg_code, t),
            border_hairline: lerp_color(a.border_hairline, b.border_hairline, t),
            border_subtle: lerp_color(a.border_subtle, b.border_subtle, t),
            border_strong: lerp_color(a.border_strong, b.border_strong, t),
            text_primary: lerp_color(a.text_primary, b.text_primary, t),
            text_secondary: lerp_color(a.text_secondary, b.text_secondary, t),
            text_muted: lerp_color(a.text_muted, b.text_muted, t),
            accent: lerp_color(a.accent, b.accent, t),
            accent_hover: lerp_color(a.accent_hover, b.accent_hover, t),
            accent_soft: lerp_color(a.accent_soft, b.accent_soft, t),
            accent_text: lerp_color(a.accent_text, b.accent_text, t),
            accent_on_fill: lerp_color(a.accent_on_fill, b.accent_on_fill, t),
            danger: lerp_color(a.danger, b.danger, t),
            danger_soft: lerp_color(a.danger_soft, b.danger_soft, t),
            warning: lerp_color(a.warning, b.warning, t),
            warning_soft: lerp_color(a.warning_soft, b.warning_soft, t),
            success: lerp_color(a.success, b.success, t),
            success_soft: lerp_color(a.success_soft, b.success_soft, t),
            shadow_elevated: lerp_shadow(a.shadow_elevated, b.shadow_elevated),
            shadow_floating: lerp_shadow(a.shadow_floating, b.shadow_floating),
        }
    }

    pub fn for_preset(preset: ThemePreset, accent: AccentPreset) -> Self {
        let mut p = match preset {
            ThemePreset::Graphite => Self::graphite(),
            ThemePreset::ObsidianOled => Self::obsidian_oled(),
            ThemePreset::Everforest => Self::everforest(),
            ThemePreset::WarmEspresso => Self::warm_espresso(),
            ThemePreset::Paper => Self::paper(),
            ThemePreset::EditorialSand => Self::editorial_sand(),
        };
        apply_accent(&mut p, accent);
        p
    }

    fn dark_semantics() -> (Color32, Color32, Color32, Color32, Color32, Color32, Shadow, Shadow) {
        (
            Color32::from_rgb(228, 102, 98),
            Color32::from_rgba_unmultiplied(228, 102, 98, 28),
            Color32::from_rgb(217, 155, 67),
            Color32::from_rgba_unmultiplied(217, 155, 67, 28),
            Color32::from_rgb(79, 168, 118),
            Color32::from_rgba_unmultiplied(79, 168, 118, 28),
            Shadow {
                offset: vec2(0.0, 3.0),
                blur: 10.0,
                spread: 0.0,
                color: Color32::from_rgba_unmultiplied(4, 4, 4, 110),
            },
            Shadow {
                offset: vec2(0.0, 14.0),
                blur: 28.0,
                spread: 0.0,
                color: Color32::from_rgba_unmultiplied(2, 2, 2, 175),
            },
        )
    }

    fn light_semantics() -> (Color32, Color32, Color32, Color32, Color32, Color32, Shadow, Shadow) {
        (
            Color32::from_rgb(198, 56, 52),
            Color32::from_rgba_unmultiplied(198, 56, 52, 20),
            Color32::from_rgb(184, 118, 22),
            Color32::from_rgba_unmultiplied(184, 118, 22, 24),
            Color32::from_rgb(32, 142, 82),
            Color32::from_rgba_unmultiplied(32, 142, 82, 22),
            Shadow {
                offset: vec2(0.0, 2.0),
                blur: 6.0,
                spread: 0.0,
                color: Color32::from_rgba_unmultiplied(26, 25, 23, 20),
            },
            Shadow {
                offset: vec2(0.0, 10.0),
                blur: 24.0,
                spread: 0.0,
                color: Color32::from_rgba_unmultiplied(26, 25, 23, 36),
            },
        )
    }

    /// "Graphite" — calm warm neutral slate with opaque tonal dividers.
    pub fn graphite() -> Self {
        let (danger, danger_soft, warning, warning_soft, success, success_soft, shadow_elevated, shadow_floating) =
            Self::dark_semantics();
        Self {
            is_dark: true,
            bg_sidebar: Color32::from_rgb(20, 20, 19),       // #141413
            bg_canvas: Color32::from_rgb(22, 22, 21),        // #161615
            bg_elevated: Color32::from_rgb(42, 42, 39),      // #2A2A27
            bg_card: Color32::from_rgb(35, 35, 33),          // #232321
            bg_input: Color32::from_rgb(23, 23, 21),         // #171715
            bg_hover: Color32::from_rgb(44, 44, 40),         // #2C2C28
            bg_selected: Color32::from_rgb(50, 49, 44),      // #32312C
            bg_code: Color32::from_rgb(19, 19, 18),          // #131312
            border_hairline: Color32::from_rgb(40, 40, 37),  // #282825
            border_subtle: Color32::from_rgb(50, 50, 46),    // #32322E
            border_strong: Color32::from_rgb(65, 64, 59),    // #41403B
            text_primary: Color32::from_rgb(236, 234, 228),  // #ECEAE4
            text_secondary: Color32::from_rgb(156, 153, 144),// #9C9990
            text_muted: Color32::from_rgb(105, 102, 95),     // #69665F
            accent: Color32::from_rgb(90, 139, 224),
            accent_hover: Color32::from_rgb(110, 155, 236),
            accent_soft: Color32::from_rgba_unmultiplied(90, 139, 224, 34),
            accent_text: Color32::from_rgb(132, 174, 245),
            accent_on_fill: Color32::WHITE,
            danger,
            danger_soft,
            warning,
            warning_soft,
            success,
            success_soft,
            shadow_elevated,
            shadow_floating,
        }
    }

    /// "Obsidian OLED" — pure ink black canvas with crisp tonal elevation steps.
    pub fn obsidian_oled() -> Self {
        let (danger, danger_soft, warning, warning_soft, success, success_soft, shadow_elevated, shadow_floating) =
            Self::dark_semantics();
        Self {
            is_dark: true,
            bg_sidebar: Color32::from_rgb(5, 5, 5),          // #050505
            bg_canvas: Color32::from_rgb(8, 8, 9),           // #080809
            bg_elevated: Color32::from_rgb(28, 28, 32),      // #1C1C20
            bg_card: Color32::from_rgb(21, 21, 24),          // #151518
            bg_input: Color32::from_rgb(8, 8, 9),            // #080809
            bg_hover: Color32::from_rgb(29, 29, 32),         // #1D1D20
            bg_selected: Color32::from_rgb(36, 36, 40),      // #242428
            bg_code: Color32::from_rgb(6, 6, 7),             // #060607
            border_hairline: Color32::from_rgb(27, 27, 30),  // #1B1B1E
            border_subtle: Color32::from_rgb(36, 36, 40),    // #242428
            border_strong: Color32::from_rgb(52, 52, 58),    // #34343A
            text_primary: Color32::from_rgb(239, 239, 239),  // #EFEFEF
            text_secondary: Color32::from_rgb(152, 152, 160),// #9898A0
            text_muted: Color32::from_rgb(98, 98, 106),      // #62626A
            accent: Color32::from_rgb(90, 139, 224),
            accent_hover: Color32::from_rgb(110, 155, 236),
            accent_soft: Color32::from_rgba_unmultiplied(90, 139, 224, 34),
            accent_text: Color32::from_rgb(132, 174, 245),
            accent_on_fill: Color32::WHITE,
            danger,
            danger_soft,
            warning,
            warning_soft,
            success,
            success_soft,
            shadow_elevated,
            shadow_floating,
        }
    }

    /// "Everforest" — forest moss and parchment text for long low-strain writing.
    pub fn everforest() -> Self {
        let (danger, danger_soft, warning, warning_soft, success, success_soft, shadow_elevated, shadow_floating) =
            Self::dark_semantics();
        Self {
            is_dark: true,
            bg_sidebar: Color32::from_rgb(34, 41, 45),       // #22292D
            bg_canvas: Color32::from_rgb(41, 49, 54),        // #293136
            bg_elevated: Color32::from_rgb(53, 64, 70),      // #354046
            bg_card: Color32::from_rgb(47, 56, 62),          // #2F383E
            bg_input: Color32::from_rgb(33, 40, 44),         // #21282C
            bg_hover: Color32::from_rgb(59, 71, 78),         // #3B474E
            bg_selected: Color32::from_rgb(66, 80, 87),      // #425057
            bg_code: Color32::from_rgb(30, 37, 41),          // #1E2529
            border_hairline: Color32::from_rgb(55, 66, 71),  // #374247
            border_subtle: Color32::from_rgb(65, 78, 84),    // #414E54
            border_strong: Color32::from_rgb(80, 96, 104),   // #506068
            text_primary: Color32::from_rgb(211, 198, 170),  // #D3C6AA
            text_secondary: Color32::from_rgb(157, 169, 160),// #9DA9A0
            text_muted: Color32::from_rgb(116, 130, 123),    // #74827B
            accent: Color32::from_rgb(90, 139, 224),
            accent_hover: Color32::from_rgb(110, 155, 236),
            accent_soft: Color32::from_rgba_unmultiplied(90, 139, 224, 34),
            accent_text: Color32::from_rgb(132, 174, 245),
            accent_on_fill: Color32::WHITE,
            danger,
            danger_soft,
            warning,
            warning_soft,
            success,
            success_soft,
            shadow_elevated,
            shadow_floating,
        }
    }

    /// "Warm Espresso" — deep roasted sepia dark surface with cream ink.
    pub fn warm_espresso() -> Self {
        let (danger, danger_soft, warning, warning_soft, success, success_soft, shadow_elevated, shadow_floating) =
            Self::dark_semantics();
        Self {
            is_dark: true,
            bg_sidebar: Color32::from_rgb(24, 20, 17),       // #181411
            bg_canvas: Color32::from_rgb(32, 27, 23),        // #201B17
            bg_elevated: Color32::from_rgb(46, 39, 33),      // #2E2721
            bg_card: Color32::from_rgb(39, 33, 28),          // #27211C
            bg_input: Color32::from_rgb(26, 21, 18),         // #1A1512
            bg_hover: Color32::from_rgb(54, 46, 39),         // #362E27
            bg_selected: Color32::from_rgb(62, 53, 45),      // #3E352D
            bg_code: Color32::from_rgb(22, 18, 15),          // #16120F
            border_hairline: Color32::from_rgb(49, 41, 35),  // #312923
            border_subtle: Color32::from_rgb(61, 51, 43),    // #3D332B
            border_strong: Color32::from_rgb(78, 65, 55),    // #4E4137
            text_primary: Color32::from_rgb(239, 230, 216),  // #EFE6D8
            text_secondary: Color32::from_rgb(175, 161, 144),// #AFA190
            text_muted: Color32::from_rgb(122, 110, 97),     // #7A6E61
            accent: Color32::from_rgb(90, 139, 224),
            accent_hover: Color32::from_rgb(110, 155, 236),
            accent_soft: Color32::from_rgba_unmultiplied(90, 139, 224, 34),
            accent_text: Color32::from_rgb(132, 174, 245),
            accent_on_fill: Color32::WHITE,
            danger,
            danger_soft,
            warning,
            warning_soft,
            success,
            success_soft,
            shadow_elevated,
            shadow_floating,
        }
    }

    /// "Paper" — crisp warm alabaster light theme with opaque stone borders.
    pub fn paper() -> Self {
        let (danger, danger_soft, warning, warning_soft, success, success_soft, shadow_elevated, shadow_floating) =
            Self::light_semantics();
        Self {
            is_dark: false,
            bg_sidebar: Color32::from_rgb(243, 242, 238),    // #F3F2EE
            bg_canvas: Color32::from_rgb(252, 252, 251),     // #FCFCFB
            bg_elevated: Color32::from_rgb(255, 255, 255),   // #FFFFFF
            bg_card: Color32::from_rgb(255, 255, 255),       // #FFFFFF
            bg_input: Color32::from_rgb(246, 245, 241),      // #F6F5F1
            bg_hover: Color32::from_rgb(234, 233, 227),      // #EAE9E3
            bg_selected: Color32::from_rgb(227, 225, 218),   // #E3E1DA
            bg_code: Color32::from_rgb(241, 240, 234),       // #F1F0EA
            border_hairline: Color32::from_rgb(229, 227, 220),// #E5E3DC
            border_subtle: Color32::from_rgb(216, 214, 206), // #D8D6CE
            border_strong: Color32::from_rgb(196, 193, 184), // #C4C1B8
            text_primary: Color32::from_rgb(26, 25, 23),     // #1A1917
            text_secondary: Color32::from_rgb(95, 93, 86),   // #5F5D56
            text_muted: Color32::from_rgb(142, 139, 130),    // #8E8B82
            accent: Color32::from_rgb(45, 99, 196),
            accent_hover: Color32::from_rgb(35, 82, 168),
            accent_soft: Color32::from_rgba_unmultiplied(45, 99, 196, 22),
            accent_text: Color32::from_rgb(38, 86, 174),
            accent_on_fill: Color32::WHITE,
            danger,
            danger_soft,
            warning,
            warning_soft,
            success,
            success_soft,
            shadow_elevated,
            shadow_floating,
        }
    }

    /// "Editorial Sand" — soft warm oat parchment for bright rooms without glare.
    pub fn editorial_sand() -> Self {
        let (danger, danger_soft, warning, warning_soft, success, success_soft, shadow_elevated, shadow_floating) =
            Self::light_semantics();
        Self {
            is_dark: false,
            bg_sidebar: Color32::from_rgb(236, 229, 216),    // #ECE5D8
            bg_canvas: Color32::from_rgb(245, 239, 228),     // #F5EFE4
            bg_elevated: Color32::from_rgb(253, 251, 246),   // #FDFBF6
            bg_card: Color32::from_rgb(251, 247, 240),       // #FBF7F0
            bg_input: Color32::from_rgb(239, 232, 219),      // #EFE8DB
            bg_hover: Color32::from_rgb(229, 221, 208),      // #E5DDD0
            bg_selected: Color32::from_rgb(221, 211, 195),   // #DDD3C3
            bg_code: Color32::from_rgb(234, 226, 212),       // #EAE2D4
            border_hairline: Color32::from_rgb(223, 214, 199),// #DFD6C7
            border_subtle: Color32::from_rgb(210, 199, 181), // #D2C7B5
            border_strong: Color32::from_rgb(189, 176, 155), // #BDB09B
            text_primary: Color32::from_rgb(43, 36, 28),     // #2B241C
            text_secondary: Color32::from_rgb(105, 94, 80),  // #695E50
            text_muted: Color32::from_rgb(150, 138, 122),    // #968A7A
            accent: Color32::from_rgb(45, 99, 196),
            accent_hover: Color32::from_rgb(35, 82, 168),
            accent_soft: Color32::from_rgba_unmultiplied(45, 99, 196, 22),
            accent_text: Color32::from_rgb(38, 86, 174),
            accent_on_fill: Color32::WHITE,
            danger,
            danger_soft,
            warning,
            warning_soft,
            success,
            success_soft,
            shadow_elevated,
            shadow_floating,
        }
    }

    /// Returns `(pill_bg, dot_color, text_color)`.
    pub fn status_colors(&self, status: NoteStatus) -> (Color32, Color32, Color32) {
        if self.is_dark {
            match status {
                NoteStatus::Todo => (
                    Color32::from_rgba_unmultiplied(154, 151, 143, 24),
                    Color32::from_rgb(140, 137, 128),
                    Color32::from_rgb(184, 181, 172),
                ),
                NoteStatus::InProgress => (
                    Color32::from_rgba_unmultiplied(217, 155, 67, 28),
                    Color32::from_rgb(217, 155, 67),
                    Color32::from_rgb(230, 181, 106),
                ),
                NoteStatus::Done => (
                    Color32::from_rgba_unmultiplied(79, 168, 118, 28),
                    Color32::from_rgb(79, 168, 118),
                    Color32::from_rgb(128, 204, 159),
                ),
            }
        } else {
            match status {
                NoteStatus::Todo => (
                    Color32::from_rgb(238, 237, 232),
                    Color32::from_rgb(122, 119, 110),
                    Color32::from_rgb(76, 74, 68),
                ),
                NoteStatus::InProgress => (
                    Color32::from_rgb(252, 242, 224),
                    Color32::from_rgb(192, 126, 28),
                    Color32::from_rgb(148, 94, 14),
                ),
                NoteStatus::Done => (
                    Color32::from_rgb(226, 244, 233),
                    Color32::from_rgb(32, 142, 82),
                    Color32::from_rgb(22, 110, 62),
                ),
            }
        }
    }

    /// Quiet, harmonious earthy/mineral tag badge palette (no candy neons).
    pub fn tag_colors(&self, tag: &str) -> (Color32, Color32) {
        let mut h: u32 = 2166136261;
        for b in tag.to_lowercase().as_bytes() {
            h ^= *b as u32;
            h = h.wrapping_mul(16777619);
        }
        let idx = (h as usize) % 6;
        if self.is_dark {
            match idx {
                0 => (Color32::from_rgba_unmultiplied(90, 139, 224, 26), Color32::from_rgb(148, 184, 245)),
                1 => (Color32::from_rgba_unmultiplied(84, 168, 158, 26), Color32::from_rgb(134, 204, 196)),
                2 => (Color32::from_rgba_unmultiplied(110, 168, 112, 26), Color32::from_rgb(154, 204, 156)),
                3 => (Color32::from_rgba_unmultiplied(204, 156, 84, 26), Color32::from_rgb(228, 188, 124)),
                4 => (Color32::from_rgba_unmultiplied(198, 118, 114, 26), Color32::from_rgb(228, 158, 154)),
                _ => (Color32::from_rgba_unmultiplied(154, 151, 143, 24), Color32::from_rgb(186, 182, 174)),
            }
        } else {
            match idx {
                0 => (Color32::from_rgb(230, 238, 250), Color32::from_rgb(40, 88, 172)),
                1 => (Color32::from_rgb(226, 242, 240), Color32::from_rgb(24, 114, 106)),
                2 => (Color32::from_rgb(228, 242, 228), Color32::from_rgb(38, 116, 46)),
                3 => (Color32::from_rgb(248, 238, 218), Color32::from_rgb(146, 98, 22)),
                4 => (Color32::from_rgb(248, 232, 230), Color32::from_rgb(158, 58, 54)),
                _ => (Color32::from_rgb(236, 235, 230), Color32::from_rgb(82, 80, 74)),
            }
        }
    }

    /// Muted status/select swatch tokens `(pill_bg, dot, text)` for custom boards and tables.
    pub fn token_colors(&self, token: ColorToken) -> (Color32, Color32, Color32) {
        if self.is_dark {
            match token {
                ColorToken::Slate => (
                    Color32::from_rgba_unmultiplied(154, 151, 143, 28),
                    Color32::from_rgb(154, 151, 143),
                    Color32::from_rgb(198, 195, 187),
                ),
                ColorToken::Blue => (
                    Color32::from_rgba_unmultiplied(90, 139, 224, 30),
                    Color32::from_rgb(90, 139, 224),
                    Color32::from_rgb(148, 184, 245),
                ),
                ColorToken::Green => (
                    Color32::from_rgba_unmultiplied(79, 168, 118, 30),
                    Color32::from_rgb(79, 168, 118),
                    Color32::from_rgb(128, 204, 159),
                ),
                ColorToken::Amber => (
                    Color32::from_rgba_unmultiplied(217, 155, 67, 30),
                    Color32::from_rgb(217, 155, 67),
                    Color32::from_rgb(232, 184, 109),
                ),
                ColorToken::Coral => (
                    Color32::from_rgba_unmultiplied(216, 110, 96, 30),
                    Color32::from_rgb(216, 110, 96),
                    Color32::from_rgb(238, 152, 140),
                ),
                ColorToken::Violet => (
                    Color32::from_rgba_unmultiplied(166, 134, 222, 30),
                    Color32::from_rgb(166, 134, 222),
                    Color32::from_rgb(198, 174, 240),
                ),
                ColorToken::Teal => (
                    Color32::from_rgba_unmultiplied(76, 174, 164, 30),
                    Color32::from_rgb(76, 174, 164),
                    Color32::from_rgb(128, 208, 200),
                ),
                ColorToken::Rose => (
                    Color32::from_rgba_unmultiplied(212, 118, 154, 30),
                    Color32::from_rgb(212, 118, 154),
                    Color32::from_rgb(235, 162, 190),
                ),
            }
        } else {
            match token {
                ColorToken::Slate => (
                    Color32::from_rgb(236, 235, 230),
                    Color32::from_rgb(116, 113, 104),
                    Color32::from_rgb(72, 70, 64),
                ),
                ColorToken::Blue => (
                    Color32::from_rgb(230, 238, 250),
                    Color32::from_rgb(45, 99, 196),
                    Color32::from_rgb(34, 79, 160),
                ),
                ColorToken::Green => (
                    Color32::from_rgb(226, 244, 233),
                    Color32::from_rgb(32, 142, 82),
                    Color32::from_rgb(22, 110, 62),
                ),
                ColorToken::Amber => (
                    Color32::from_rgb(252, 242, 224),
                    Color32::from_rgb(184, 118, 22),
                    Color32::from_rgb(142, 88, 14),
                ),
                ColorToken::Coral => (
                    Color32::from_rgb(250, 232, 228),
                    Color32::from_rgb(184, 72, 56),
                    Color32::from_rgb(148, 52, 40),
                ),
                ColorToken::Violet => (
                    Color32::from_rgb(240, 234, 250),
                    Color32::from_rgb(122, 78, 186),
                    Color32::from_rgb(94, 56, 150),
                ),
                ColorToken::Teal => (
                    Color32::from_rgb(224, 242, 240),
                    Color32::from_rgb(28, 128, 120),
                    Color32::from_rgb(20, 98, 92),
                ),
                ColorToken::Rose => (
                    Color32::from_rgb(250, 232, 240),
                    Color32::from_rgb(176, 62, 110),
                    Color32::from_rgb(140, 44, 84),
                ),
            }
        }
    }
}

fn apply_accent(p: &mut ThemePalette, accent: AccentPreset) {
    let (acc, hov, soft, text, on_fill) = if p.is_dark {
        match accent {
            AccentPreset::SteelBlue => (
                Color32::from_rgb(90, 139, 224),
                Color32::from_rgb(110, 155, 236),
                Color32::from_rgba_unmultiplied(90, 139, 224, 34),
                Color32::from_rgb(132, 174, 245),
                Color32::WHITE,
            ),
            AccentPreset::Sage => (
                Color32::from_rgb(101, 166, 120),
                Color32::from_rgb(118, 184, 137),
                Color32::from_rgba_unmultiplied(101, 166, 120, 32),
                Color32::from_rgb(138, 201, 157),
                Color32::from_rgb(14, 26, 18),
            ),
            AccentPreset::Amber => (
                Color32::from_rgb(212, 152, 66),
                Color32::from_rgb(224, 168, 86),
                Color32::from_rgba_unmultiplied(212, 152, 66, 32),
                Color32::from_rgb(232, 184, 109),
                Color32::from_rgb(26, 19, 8),
            ),
            AccentPreset::Terracotta => (
                Color32::from_rgb(206, 117, 94),
                Color32::from_rgb(220, 135, 113),
                Color32::from_rgba_unmultiplied(206, 117, 94, 32),
                Color32::from_rgb(229, 152, 132),
                Color32::from_rgb(29, 16, 12),
            ),
            AccentPreset::GraphiteMono => (
                Color32::from_rgb(214, 212, 206),
                Color32::from_rgb(230, 228, 222),
                Color32::from_rgba_unmultiplied(214, 212, 206, 26),
                Color32::from_rgb(229, 227, 220),
                Color32::from_rgb(22, 22, 20),
            ),
        }
    } else {
        match accent {
            AccentPreset::SteelBlue => (
                Color32::from_rgb(45, 99, 196),
                Color32::from_rgb(35, 82, 168),
                Color32::from_rgba_unmultiplied(45, 99, 196, 22),
                Color32::from_rgb(38, 86, 174),
                Color32::WHITE,
            ),
            AccentPreset::Sage => (
                Color32::from_rgb(47, 125, 78),
                Color32::from_rgb(37, 102, 63),
                Color32::from_rgba_unmultiplied(47, 125, 78, 22),
                Color32::from_rgb(39, 107, 66),
                Color32::WHITE,
            ),
            AccentPreset::Amber => (
                Color32::from_rgb(181, 114, 20),
                Color32::from_rgb(153, 94, 14),
                Color32::from_rgba_unmultiplied(181, 114, 20, 22),
                Color32::from_rgb(148, 91, 13),
                Color32::WHITE,
            ),
            AccentPreset::Terracotta => (
                Color32::from_rgb(184, 81, 56),
                Color32::from_rgb(158, 66, 43),
                Color32::from_rgba_unmultiplied(184, 81, 56, 22),
                Color32::from_rgb(158, 66, 43),
                Color32::WHITE,
            ),
            AccentPreset::GraphiteMono => (
                Color32::from_rgb(43, 42, 39),
                Color32::from_rgb(26, 25, 23),
                Color32::from_rgba_unmultiplied(43, 42, 39, 20),
                Color32::from_rgb(43, 42, 39),
                Color32::WHITE,
            ),
        }
    };
    p.accent = acc;
    p.accent_hover = hov;
    p.accent_soft = soft;
    p.accent_text = text;
    p.accent_on_fill = on_fill;
}

pub fn accent_swatch_color(accent: AccentPreset, is_dark: bool) -> Color32 {
    if is_dark {
        match accent {
            AccentPreset::SteelBlue => Color32::from_rgb(90, 139, 224),
            AccentPreset::Sage => Color32::from_rgb(101, 166, 120),
            AccentPreset::Amber => Color32::from_rgb(212, 152, 66),
            AccentPreset::Terracotta => Color32::from_rgb(206, 117, 94),
            AccentPreset::GraphiteMono => Color32::from_rgb(214, 212, 206),
        }
    } else {
        match accent {
            AccentPreset::SteelBlue => Color32::from_rgb(45, 99, 196),
            AccentPreset::Sage => Color32::from_rgb(47, 125, 78),
            AccentPreset::Amber => Color32::from_rgb(181, 114, 20),
            AccentPreset::Terracotta => Color32::from_rgb(184, 81, 56),
            AccentPreset::GraphiteMono => Color32::from_rgb(43, 42, 39),
        }
    }
}

pub fn font_body(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

pub fn font_medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("medium".into()))
}

pub fn font_semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("semibold".into()))
}

pub fn font_bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("bold".into()))
}

pub fn font_serif_semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("serif_semibold".into()))
}

pub fn font_serif_bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("serif_bold".into()))
}

pub fn font_mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}

pub fn font_lucide(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("lucide".into()))
}

pub fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    fonts.font_data.insert(
        "plex_sans_regular".to_owned(),
        FontData::from_static(include_bytes!("../../assets/fonts/IBMPlexSans-Regular.ttf")),
    );
    fonts.font_data.insert(
        "plex_sans_medium".to_owned(),
        FontData::from_static(include_bytes!("../../assets/fonts/IBMPlexSans-Medium.ttf")),
    );
    fonts.font_data.insert(
        "plex_sans_semibold".to_owned(),
        FontData::from_static(include_bytes!("../../assets/fonts/IBMPlexSans-SemiBold.ttf")),
    );
    fonts.font_data.insert(
        "plex_sans_bold".to_owned(),
        FontData::from_static(include_bytes!("../../assets/fonts/IBMPlexSans-Bold.ttf")),
    );
    fonts.font_data.insert(
        "plex_serif_semibold".to_owned(),
        FontData::from_static(include_bytes!("../../assets/fonts/IBMPlexSerif-SemiBold.ttf")),
    );
    fonts.font_data.insert(
        "plex_serif_bold".to_owned(),
        FontData::from_static(include_bytes!("../../assets/fonts/IBMPlexSerif-Bold.ttf")),
    );
    fonts.font_data.insert(
        "plex_mono".to_owned(),
        FontData::from_static(include_bytes!("../../assets/fonts/IBMPlexMono-Regular.ttf")),
    );
    fonts.font_data.insert(
        "lucide".to_owned(),
        FontData::from_static(include_bytes!("../../assets/fonts/lucide.ttf")),
    );

    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "plex_sans_regular".to_owned());

    fonts.families.insert(
        FontFamily::Name("medium".into()),
        vec!["plex_sans_medium".to_owned(), "plex_sans_regular".to_owned()],
    );
    fonts.families.insert(
        FontFamily::Name("semibold".into()),
        vec!["plex_sans_semibold".to_owned(), "plex_sans_regular".to_owned()],
    );
    fonts.families.insert(
        FontFamily::Name("bold".into()),
        vec!["plex_sans_bold".to_owned(), "plex_sans_regular".to_owned()],
    );
    fonts.families.insert(
        FontFamily::Name("serif_semibold".into()),
        vec!["plex_serif_semibold".to_owned(), "plex_sans_semibold".to_owned()],
    );
    fonts.families.insert(
        FontFamily::Name("serif_bold".into()),
        vec!["plex_serif_bold".to_owned(), "plex_sans_bold".to_owned()],
    );

    fonts
        .families
        .entry(FontFamily::Monospace)
        .or_default()
        .insert(0, "plex_mono".to_owned());

    fonts
        .families
        .insert(FontFamily::Name("lucide".into()), vec!["lucide".to_owned()]);

    ctx.set_fonts(fonts);
}

#[cfg(test)]
pub fn apply_theme(ctx: &egui::Context, preset: ThemePreset, accent: AccentPreset) {
    let p = ThemePalette::for_preset(preset, accent);
    apply_palette(ctx, &p);
}

pub fn apply_palette(ctx: &egui::Context, p: &ThemePalette) {
    let mut style = (*ctx.style()).clone();

    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(10.0, 5.0);
    style.spacing.window_margin = egui::Margin::same(18.0);
    style.spacing.menu_margin = egui::Margin::same(6.0);
    style.spacing.scroll = egui::style::ScrollStyle {
        floating: true,
        bar_width: 4.5,
        floating_width: 3.0,
        floating_allocated_width: 0.0,
        handle_min_length: 24.0,
        bar_inner_margin: 2.0,
        bar_outer_margin: 2.0,
        foreground_color: false,
        dormant_background_opacity: 0.0,
        active_background_opacity: 0.08,
        interact_background_opacity: 0.16,
        dormant_handle_opacity: 0.0,
        active_handle_opacity: 0.35,
        interact_handle_opacity: 0.65,
    };

    style.text_styles.insert(egui::TextStyle::Body, font_body(14.5));
    style.text_styles.insert(egui::TextStyle::Button, font_medium(13.0));
    style.text_styles.insert(egui::TextStyle::Small, font_body(12.0));
    style.text_styles.insert(egui::TextStyle::Monospace, font_mono(13.5));
    style.text_styles.insert(egui::TextStyle::Heading, font_serif_semibold(20.0));

    let mut v = if p.is_dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };

    v.panel_fill = p.bg_canvas;
    v.window_fill = p.bg_elevated;
    v.extreme_bg_color = p.bg_input;
    v.faint_bg_color = p.bg_hover;
    v.window_rounding = Rounding::same(10.0);
    v.window_stroke = Stroke::new(1.0_f32, p.border_subtle);
    v.window_shadow = p.shadow_floating;
    v.popup_shadow = p.shadow_elevated;
    v.menu_rounding = Rounding::same(8.0);

    v.widgets.noninteractive.bg_fill = p.bg_canvas;
    v.widgets.noninteractive.weak_bg_fill = p.bg_canvas;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, p.border_hairline);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, p.text_primary);
    v.widgets.noninteractive.rounding = Rounding::same(6.0);

    v.widgets.inactive.bg_fill = p.bg_elevated;
    v.widgets.inactive.weak_bg_fill = p.bg_elevated;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, p.border_subtle);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, p.text_primary);
    v.widgets.inactive.rounding = Rounding::same(6.0);

    v.widgets.hovered.bg_fill = p.bg_hover;
    v.widgets.hovered.weak_bg_fill = p.bg_hover;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, p.border_strong);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, p.text_primary);
    v.widgets.hovered.rounding = Rounding::same(6.0);
    v.widgets.hovered.expansion = 0.0;

    v.widgets.active.bg_fill = p.bg_selected;
    v.widgets.active.weak_bg_fill = p.bg_selected;
    v.widgets.active.bg_stroke = Stroke::new(1.0_f32, p.accent);
    v.widgets.active.fg_stroke = Stroke::new(1.0_f32, p.text_primary);
    v.widgets.active.rounding = Rounding::same(6.0);
    v.widgets.active.expansion = 0.0;

    v.widgets.open.bg_fill = p.bg_hover;
    v.widgets.open.bg_stroke = Stroke::new(1.0_f32, p.border_subtle);
    v.widgets.open.rounding = Rounding::same(6.0);

    v.selection.bg_fill = crate::ui::motion::with_alpha(p.accent, if p.is_dark { 0.36 } else { 0.26 });
    v.selection.stroke = Stroke::new(1.0_f32, p.accent);
    v.text_cursor.stroke = Stroke::new(1.5_f32, p.accent);

    style.visuals = v;
    ctx.set_style(style);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_fonts_and_all_themes_load_cleanly() {
        let ctx = egui::Context::default();
        setup_fonts(&ctx);
        for preset in ThemePreset::ALL {
            for accent in AccentPreset::ALL {
                apply_theme(&ctx, preset, accent);
                let p = ThemePalette::for_preset(preset, accent);
                assert_eq!(p.is_dark, preset.is_dark());
            }
        }
    }

    #[test]
    fn interpolates_palettes_cleanly() {
        let a = ThemePalette::for_preset(ThemePreset::Graphite, AccentPreset::SteelBlue);
        let b = ThemePalette::for_preset(ThemePreset::Paper, AccentPreset::Sage);
        let p0 = ThemePalette::lerp(&a, &b, 0.0);
        let p5 = ThemePalette::lerp(&a, &b, 0.5);
        let p1 = ThemePalette::lerp(&a, &b, 1.0);
        assert!(p0.is_dark);
        assert_eq!(p0.bg_canvas, a.bg_canvas);
        assert!(!p5.is_dark);
        assert_ne!(p5.bg_canvas, a.bg_canvas);
        assert_ne!(p5.bg_canvas, b.bg_canvas);
        assert_eq!(p1.bg_canvas, b.bg_canvas);
    }
}
