# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

Z-Color-Picker is a single-binary Rust (edition 2021) desktop app built on `eframe`/`egui` 0.31 + `egui_tiles`. The user places HSV control points on a 2D saturation/value picker plus a hue bar, and the app interpolates a color gradient through them using a selectable spline mode. Presets can be saved and loaded, and gradients or colors can be copied to the clipboard.

## Commands

```
cargo build
cargo run            # run from repo root: presets resolve relative to the CWD
cargo clippy         # exits 0; warn-level style lints remain
cargo test
cargo test hermite   # single test by name filter (bin crate; tests live in #[cfg(test)] modules under src/)
```

- The build is warning-free. Keep it that way: delete unused code, no `#[allow]`.
- Unit tests live in `#[cfg(test)] mod tests` in `math.rs`, `hsv_key_value.rs`, `common.rs`, `control_points.rs`, `preset.rs` (tempfile dirs, never `./presets`), `clipboard.rs`, `color_picker.rs` and `error.rs`. UI behavior is checked by running the app.
- Line endings differ from file to file (some CRLF, some LF). Keep each file's existing endings.
- `.vscode/launch.json` has lldb (CodeLLDB) configs for the binary and its unit tests.
- The `accesskit` Cargo feature doesn't compile. The block it gates in `ZColorPicker::add_contents` is leftover code copied from egui's Slider.

## Controls (from README)
- Double-click (M1) on the picker: add a control point.
- Right-click (M2) on a point: remove it.
- Middle-click: copy the color under the cursor in the selected format. On the previewer, it copies the image instead.
- F11/F12: debug windows.
- Esc: quit.

## Architecture

### App state and ownership
- `src/main.rs` starts eframe. If eframe persistence storage holds a serialized `ZApp` (JSON under `eframe::APP_KEY`), the app is deserialized from it and `request_init()` sends it back through the startup path. Otherwise `ZApp::new` is used.
- `ZApp` (`src/ui_egui/app.rs`) is a small state machine (`Startup` -> `Idle` -> `Exit`). It owns the `egui_tiles::Tree<Pane>` and an `Rc<RefCell<ZColorPickerAppContext>>`.
- `ZColorPickerAppContext` is the central mutable state: `control_points`, `spline_mode`, `PresetHandler`, the picker and previewer widgets, pending input events, and so on. Each pane (`src/ui_egui/panes.rs`: ColorPicker, ColorPickerOptions, Previewer, Log) holds a clone of that `Rc`. After deserialization, `startup()` walks the tile tree and calls `update_ctx` to re-point every pane at the live context. Serde does not preserve `Rc` sharing, so this step is load-bearing.
- Because everything shares one `RefCell`, borrow conflicts are the main runtime hazard. The existing code clones data out, drops the borrow, draws, then re-borrows mutably to write back. Examples are the `ui()` methods in `panes.rs` and `show_create_preset_window`. Follow that pattern.
- Any cloned state has to be written back explicitly, or the edits are lost without any error. For example, `ColorPickerOptionsPane::ui` has to assign `options_window`, `presets_window` and `preset_handler` back to the context.
- The preset bar (`WindowPreset`) is drawn only in the options pane. The "Create Preset" window is drawn from `ZApp::draw_ui_tree` before the tile tree.
- Fields marked `#[serde(skip)]` are transient UI/event state. Anything without it is persisted in eframe storage, so changing a persisted type's shape can break loading of existing saved state. Loading falls back silently to defaults on a parse failure.

### Data model
- `ControlPoint` (`src/datatypes/control_point.rs`) is an enum over `ControlPointStorage { val, t, tangents: [Option<_>; 2] }`.
- `ControlPointValue` = `HsvKeyValue` (`src/datatypes/hsv_key_value.rs`), which stores `[f32; 3]`. The index order is **`[s (x), v (y), h]`, not `[h, s, v]`**: index 0/1 are the 2D picker position and index 2 is hue. `HsvKeyValue` implements `splines::Interpolate`, with hue-aware lerp from `src/math.rs`.
- Tangents are stored as offsets relative to the point's value.

### Spline evaluation
- `control_points_to_spline` (`src/ui_egui/control_points.rs`) converts control points into a `splines::Spline`, one match arm per `SplineMode` (`src/common.rs`).
- Keys are placed at `t = index`, not at `ControlPoint.t`. Anything that walks segments, such as the curvature comb, must use index order too.
- `ControlPoint.t` (edited with the T slider) doesn't affect the gradient yet; it's listed as work in progress in the README.
- `HermiteBezier` is actually Catmull-Rom: the endpoints are duplicated as phantom keys.
- There is no `Polynomial` mode; saved JSON that names it fails to parse.
- `flatten_control_points` unwraps hue across the 0/1 boundary before interpolation.
- The previewer (`src/ui_egui/previewer.rs`) samples with `sample_spline_normalized`, which maps x in [0,1] onto the real keys. It handles the end of a Catmull-Rom spline, where the `splines` sampler returns `None`. For non-Hermite modes, the quantized preview first remaps key `t` to the preview sizes (`modify_spline_t_to_preview_sizes`).
- Curvature comb drawing lives in `src/ui_egui/curvature.rs` and has its own cubic evaluation. It is toggled via `ZColorPickerOptions::is_show_curvature_comb`.

### Presets
- `src/preset.rs`: one JSON file per `PresetEntity` in `./presets/` (the `presets/` folder is gitignored). The path comes from `std::env::current_dir()`.
- Presets are written through to disk: Create, Save, Rename and Delete in the preset bar (`WindowPreset::draw_content`, `ZApp::show_create_preset_window`) hit disk immediately via `PresetHandler`. Nothing is synced at exit.
- Create writes the file, then adds it to the list. Rename uses `fs::rename`. Delete removes the file first, behind a confirm `Modal`.
- Name rules, enforced by `validate_preset_name` / `PresetHandler::check_name`: not empty or whitespace; no `< > : " / \ | ? *` or control chars; no trailing dot or space; no reserved device name (CON, NUL, COM1..); at most 64 chars; no case-insensitive duplicate. The UI disables Create/Rename and shows the reason in red.
- When `auto_save_presets` is enabled, the selected preset is rewritten only when the current control points or spline mode differ from it (`PresetHandler::auto_save`), and only once no pointer button is held, so a drag writes once, on release. A failed write keeps the preset dirty, shows the error in the preset bar, and is not retried until the content changes again.
- `PresetHandler.presets` is `#[serde(skip)]`: presets are not in eframe storage, disk is the truth. At startup `init_presets` loads them from `presets_dir` (default `./presets`), skips non-`.json` or malformed files with `log::warn`, and keeps the selection only if it is still in range (otherwise `None`).

### Clipboard / screen capture
Pressing the middle button stores a `ScreenshotTarget` in `pending_screenshot_target` and requests one `ViewportCommand::Screenshot`. It closes the copy popup first, because eframe captures the frame painted in that same update. The target is the previewer rect under the cursor (rects come from `stored_ui_responses`), or else the pixel under the cursor. The screenshot event arrives in a later frame. `handle_screenshot_event` crops it with `crop_screenshot` (`src/ui_egui/clipboard.rs`) and turns it into a `ClipboardCopyEvent`. A pixel target takes the physical pixel at `floor(cursor * ppp)`, clamped to the screenshot; a previewer crop is clamped too. That event is consumed at the start of the next `Idle` frame (`handle_clipboardcopy_event`). Nothing in the app requests that frame: egui repaints right after any pass whose input has events, and the Screenshot event counts. A single pixel is copied as text in `color_copy_format`; a larger region is copied as an image via `arboard`. A failed write shows `Copy failed: <error>` in the popup.
- `format_color_as` supports all 11 `ColorStringCopy` formats, from unmultiplied sRGB; `RGB`/`RGBA` are linear.

### Misc
- The log filter defaults to `info,z_color_picker=debug` (`DEFAULT_LOG_FILTER` in `src/logger.rs`); a set `RUST_LOG` replaces it, e.g. `RUST_LOG=debug` shows dependency debug output too.
- `LogCollector` (`src/logger.rs`) captures log lines into a shared `Arc<Mutex<Vec<String>>>`, capped at 2000 lines, which the Log pane displays. Never call `log::*` while holding that mutex; `LogCollector::log` locks it too, so it would deadlock.
- The UI scale is derived from a hardcoded 2560x1440 monitor size (`HARDCODED_MONITOR_SIZE`) because the monitor size is not available at creation time.
- The debug windows live in `src/ui_egui/debug_windows.rs`.
