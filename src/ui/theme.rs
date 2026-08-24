use ratatui::style::Color;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ThemeId {
    #[default]
    SpirittyDark,
    CatppuccinMocha,
    TokyoNight,
    Nord,
    GruvboxDark,
    Dracula,
    Monokai,
}

impl ThemeId {
    pub fn all() -> &'static [ThemeId] {
        &[
            ThemeId::SpirittyDark,
            ThemeId::CatppuccinMocha,
            ThemeId::TokyoNight,
            ThemeId::Nord,
            ThemeId::GruvboxDark,
            ThemeId::Dracula,
            ThemeId::Monokai,
        ]
    }

    pub fn key_str(&self) -> &'static str {
        match self {
            ThemeId::SpirittyDark => "spiritty_dark",
            ThemeId::CatppuccinMocha => "catppuccin_mocha",
            ThemeId::TokyoNight => "tokyo_night",
            ThemeId::Nord => "nord",
            ThemeId::GruvboxDark => "gruvbox_dark",
            ThemeId::Dracula => "dracula",
            ThemeId::Monokai => "monokai",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            ThemeId::SpirittyDark => "Spiritty Dark (Default)",
            ThemeId::CatppuccinMocha => "Catppuccin Mocha",
            ThemeId::TokyoNight => "Tokyo Night",
            ThemeId::Nord => "Nord Arctic",
            ThemeId::GruvboxDark => "Gruvbox Dark",
            ThemeId::Dracula => "Dracula",
            ThemeId::Monokai => "Monokai Pro",
        }
    }
}

impl std::str::FromStr for ThemeId {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.trim().to_lowercase().as_str() {
            "catppuccin" | "catppuccin_mocha" | "catppuccin-mocha" | "mocha" => ThemeId::CatppuccinMocha,
            "tokyo" | "tokyo_night" | "tokyonight" | "tokyo-night" => ThemeId::TokyoNight,
            "nord" | "nordic" => ThemeId::Nord,
            "gruvbox" | "gruvbox_dark" | "gruvbox-dark" => ThemeId::GruvboxDark,
            "dracula" => ThemeId::Dracula,
            "monokai" | "monokai_pro" | "monokai-pro" => ThemeId::Monokai,
            _ => ThemeId::SpirittyDark,
        })
    }
}

impl ThemeId {
    pub fn parse_or_default(s: &str) -> Self {
        s.parse().unwrap_or(ThemeId::SpirittyDark)
    }

    pub fn palette(&self) -> ThemePalette {
        match self {
            ThemeId::SpirittyDark => ThemePalette {
                id: *self,
                name: self.display_name(),
                gradient_start: (8, 12, 22),
                gradient_end: (15, 23, 42),
                border_focused: Color::Cyan,
                border_unfocused: Color::DarkGray,
                accent_primary: Color::Cyan,
                accent_secondary: Color::LightCyan,
                success: Color::Green,
                warning: Color::Yellow,
                danger: Color::Red,
                text_primary: Color::White,
                text_secondary: Color::Gray,
                text_dim: Color::DarkGray,
                selection_bg: Color::Rgb(40, 75, 130),
            },
            ThemeId::CatppuccinMocha => ThemePalette {
                id: *self,
                name: self.display_name(),
                gradient_start: (17, 17, 27),   // #11111b (Crust)
                gradient_end: (30, 30, 46),     // #1e1e2e (Base)
                border_focused: Color::Rgb(203, 166, 247),   // Mauve
                border_unfocused: Color::Rgb(88, 91, 112),   // Surface2
                accent_primary: Color::Rgb(137, 180, 250),   // Blue
                accent_secondary: Color::Rgb(203, 166, 247), // Mauve
                success: Color::Rgb(166, 227, 161),          // Green
                warning: Color::Rgb(250, 179, 135),          // Peach
                danger: Color::Rgb(243, 139, 168),           // Red
                text_primary: Color::Rgb(205, 214, 244),     // Text
                text_secondary: Color::Rgb(166, 173, 200),   // Subtext0
                text_dim: Color::Rgb(108, 112, 134),         // Overlay0
                selection_bg: Color::Rgb(69, 71, 90),        // Surface1
            },
            ThemeId::TokyoNight => ThemePalette {
                id: *self,
                name: self.display_name(),
                gradient_start: (15, 15, 24),   // #0f0f18
                gradient_end: (26, 27, 38),     // #1a1b26
                border_focused: Color::Rgb(122, 162, 247),   // #7aa2f7
                border_unfocused: Color::Rgb(65, 72, 104),
                accent_primary: Color::Rgb(122, 162, 247),
                accent_secondary: Color::Rgb(187, 154, 247), // #bb9af7
                success: Color::Rgb(158, 206, 106),          // #9ece6a
                warning: Color::Rgb(224, 175, 104),          // #e0af68
                danger: Color::Rgb(247, 118, 142),           // #f7768e
                text_primary: Color::Rgb(192, 202, 245),
                text_secondary: Color::Rgb(154, 165, 206),
                text_dim: Color::Rgb(86, 95, 137),
                selection_bg: Color::Rgb(40, 52, 85),
            },
            ThemeId::Nord => ThemePalette {
                id: *self,
                name: self.display_name(),
                gradient_start: (24, 28, 36),   // #181c24
                gradient_end: (46, 52, 64),     // #2e3440 (nord0)
                border_focused: Color::Rgb(136, 192, 208),   // #88c0d0 (nord8)
                border_unfocused: Color::Rgb(76, 86, 106),   // #4c566a (nord3)
                accent_primary: Color::Rgb(136, 192, 208),   // #88c0d0
                accent_secondary: Color::Rgb(129, 161, 193), // #81a1c1
                success: Color::Rgb(163, 190, 140),          // #a3be8c
                warning: Color::Rgb(235, 203, 139),          // #ebcb8b
                danger: Color::Rgb(191, 97, 106),            // #bf616a
                text_primary: Color::Rgb(236, 239, 244),     // #eceff4 (nord6)
                text_secondary: Color::Rgb(216, 222, 233),   // #d8dee9 (nord4)
                text_dim: Color::Rgb(94, 129, 172),          // #5e81ac
                selection_bg: Color::Rgb(59, 66, 82),        // #3b4252
            },
            ThemeId::GruvboxDark => ThemePalette {
                id: *self,
                name: self.display_name(),
                gradient_start: (20, 20, 20),   // #141414
                gradient_end: (40, 40, 40),     // #282828
                border_focused: Color::Rgb(254, 128, 25),    // #fe8019 (Orange)
                border_unfocused: Color::Rgb(80, 73, 69),    // #504945
                accent_primary: Color::Rgb(250, 189, 47),    // #fabd2f (Yellow)
                accent_secondary: Color::Rgb(131, 165, 152), // #83a598 (Blue)
                success: Color::Rgb(184, 187, 38),           // #b8bb26 (Green)
                warning: Color::Rgb(254, 128, 25),           // #fe8019
                danger: Color::Rgb(251, 73, 52),             // #fb4934 (Red)
                text_primary: Color::Rgb(235, 219, 178),     // #ebdbb2
                text_secondary: Color::Rgb(168, 153, 132),   // #a89984
                text_dim: Color::Rgb(102, 92, 84),
                selection_bg: Color::Rgb(60, 56, 54),
            },
            ThemeId::Dracula => ThemePalette {
                id: *self,
                name: self.display_name(),
                gradient_start: (20, 20, 30),   // #14141e
                gradient_end: (40, 42, 54),     // #282a36
                border_focused: Color::Rgb(189, 147, 249),   // #bd93f9 (Purple)
                border_unfocused: Color::Rgb(68, 71, 90),    // #44475a
                accent_primary: Color::Rgb(255, 121, 198),   // #ff79c6 (Pink)
                accent_secondary: Color::Rgb(139, 233, 253), // #8be9fd (Cyan)
                success: Color::Rgb(80, 250, 123),           // #50fa7b (Green)
                warning: Color::Rgb(241, 250, 140),          // #f1fa8c (Yellow)
                danger: Color::Rgb(255, 85, 85),             // #ff5555 (Red)
                text_primary: Color::Rgb(248, 248, 242),     // #f8f8f2
                text_secondary: Color::Rgb(189, 147, 249),
                text_dim: Color::Rgb(98, 114, 164),          // #6272a4
                selection_bg: Color::Rgb(68, 71, 90),
            },
            ThemeId::Monokai => ThemePalette {
                id: *self,
                name: self.display_name(),
                gradient_start: (18, 19, 16),   // #121310
                gradient_end: (39, 40, 34),     // #272822
                border_focused: Color::Rgb(166, 226, 46),    // #a6e22e (Green)
                border_unfocused: Color::Rgb(73, 72, 62),    // #49483e
                accent_primary: Color::Rgb(249, 38, 114),    // #f92672 (Pink)
                accent_secondary: Color::Rgb(102, 217, 239), // #66d9ef (Cyan)
                success: Color::Rgb(166, 226, 46),           // #a6e22e (Green)
                warning: Color::Rgb(230, 219, 116),          // #e6db74 (Yellow)
                danger: Color::Rgb(249, 38, 114),            // #f92672 (Red)
                text_primary: Color::Rgb(248, 248, 242),     // #f8f8f2
                text_secondary: Color::Rgb(165, 158, 134),
                text_dim: Color::Rgb(117, 113, 94),          // #75715e
                selection_bg: Color::Rgb(59, 58, 48),
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ThemePalette {
    pub id: ThemeId,
    pub name: &'static str,
    pub gradient_start: (u8, u8, u8),
    pub gradient_end: (u8, u8, u8),
    pub border_focused: Color,
    pub border_unfocused: Color,
    pub accent_primary: Color,
    pub accent_secondary: Color,
    pub success: Color,
    pub warning: Color,
    pub danger: Color,
    pub text_primary: Color,
    pub text_secondary: Color,
    pub text_dim: Color,
    pub selection_bg: Color,
}
