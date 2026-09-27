# Implementation Plan — Swap Split Panels (chat/terminal order)

**Date:** September 27, 2026
**Status:** Complete — `F5` swap shipped (Unreleased)

---

## 🎯 Context & Objective

Today the split always renders the chat first and the terminal second:

- **Vertical** (side by side): chat left / terminal right.
- **Horizontal** (stacked): chat top / terminal bottom.

Users want to flip the order without changing the orientation, so they can get:

- **Vertical:** terminal left / chat right.
- **Horizontal:** terminal top / chat bottom.

`F4` keeps toggling the *orientation*; a new `F5` toggles the *panel order* (swap).

---

## 📐 Design

### State

- `App.split_swapped: bool` (`false` = chat first, current behaviour).
- Persisted as `split_swapped` in `~/.config/spiritty/config.toml` (`Config::get_split_swapped()`), so the choice survives a restart.
- `F5` (`toggle_split_swapped`) flips it, persists it and shows an i18n toast.

### Layout math (`src/ui/mod.rs`)

`draw()` still computes `body_chunks` from the orientation, then assigns the areas based on the order:

- not swapped: `chat_area = chunks[0]`, `terminal_area = chunks[1]`.
- swapped: `chat_area = chunks[1]`, `terminal_area = chunks[0]`.

The divider is drawn on the **first** chunk's inner edge (`chunks[0]`), which is the boundary in both cases.

### Interaction (`src/app.rs`)

- Divider hit-test: derive the boundary from `min(lefts)`/`max(lefts)` (vertical) and `min(tops)`/`max(tops)` (horizontal), independent of the order.
- Mouse drag: the ratio always tracks the **chat** panel, so invert the computed percentage when swapped.
- `Alt + ←/→` (vertical) / `Alt + ↑/↓` (horizontal): keep the physical divider direction; invert the ratio delta when swapped.
- PTY sizing (terminal always gets the complement of the ratio) needs no change.

### i18n

- `HelpDescSwapPanels` (help modal row on `F5`).
- `ToastLayoutVerticalSwapped` / `ToastLayoutHorizontalSwapped`, and a shared `layout_toast()` so `F4` and `F5` both report the real arrangement.

---

## ✅ Checklist

- [x] `Config.split_swapped` + getter + default.
- [x] `App.split_swapped` + init + `toggle_split_swapped()`.
- [x] `draw()` area assignment + divider position.
- [x] Divider hit-test + drag + keyboard direction.
- [x] i18n keys (mod/fr/en) + help modal row.
- [x] Tests (`config_test`, `basic_test`).
- [x] README / README.fr / ARCHITECTURE / CHANGELOG.
