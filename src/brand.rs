//! Centralized brand identity for Spiritty.
//!
//! Spiritty's icon is a genie lamp (`assets/icons/spiritty.svg`). A terminal cannot
//! render a vector image, so the in-TUI brand marker is the emoji lampe (🧞) while
//! the SVG is embedded here and available as the app's official icon asset (future
//! desktop-entry / installer / About usage).

/// The in-TUI brand glyph. Kept in one place so the whole app uses the same marker.
pub const BRAND_GLYPH: &str = "🧞";

/// Name used in titles / export headers: `Spiritty` prefixed with the brand glyph.
pub fn brand_title(version: &str) -> String {
    format!("{} Spiritty v{} ", BRAND_GLYPH, version)
}

/// The official vector icon, embedded at build time.
pub const ICON_SVG: &str = include_str!("../assets/icons/spiritty.svg");
