//! Centralized brand identity for Spiritty.
//!
//! Spiritty's icon is the celestial genie (`assets/icons/spiritty.svg` / `assets/icons/spiritty.png`).
//! A terminal cannot render bitmap or vector images, so the in-TUI brand marker is the emoji lampe (🧞) while
//! the SVG/PNG are embedded here and available as the app's official icon assets.

/// The in-TUI brand glyph. Kept in one place so the whole app uses the same marker.
pub const BRAND_GLYPH: &str = "🧞";

/// Name used in titles / export headers: `Spiritty` prefixed with the brand glyph.
pub fn brand_title(version: &str) -> String {
    format!("{} Spiritty v{} ", BRAND_GLYPH, version)
}

/// The official vector icon, embedded at build time.
pub const ICON_SVG: &str = include_str!("../assets/icons/spiritty.svg");

/// The official high-resolution PNG icon bytes, embedded at build time.
pub const ICON_PNG: &[u8] = include_bytes!("../assets/icons/spiritty.png");
