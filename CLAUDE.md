# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

Z-Color-Picker is a single-binary Rust (edition 2021) desktop app built on `eframe`/`egui` 0.31 + `egui_tiles`. The user places HSV control points on a 2D saturation/value picker plus a hue bar, and the app interpolates a color gradient through them using a selectable spline mode. Presets can be saved and loaded, and gradients or colors can be copied to the clipboard.

## Commands

```
cargo build
cargo run            # run from repo root: presets resolve relative to the CWD
cargo clippy         # currently fails on a deny-level lint at preset.rs (`presets.len() <= 0`)
cargo test
cargo test hermite   # single test by name filter (bin crate; tests live in #[cfg(test)] modules under src/)
```

- The crate has many existing dead-code and unused-import warnings. To check whether a change adds new ones, compare against a build of HEAD instead of expecting a clean build.
- The only tests so far are the spline-sampling tests in `src/ui_egui/control_points.rs`. UI behavior is checked by running the app.
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
- `Polynomial` is `todo!()`.
- `flatten_control_points` unwraps hue across the 0/1 boundary before interpolation.
- The previewer (`src/ui_egui/previewer.rs`) samples with `sample_spline_normalized`, which maps x in [0,1] onto the real keys. It handles the end of a Catmull-Rom spline, where the `splines` sampler returns `None`. For non-Hermite modes, the quantized preview first remaps key `t` to the preview sizes (`modify_spline_t_to_preview_sizes`).
- Curvature comb drawing lives in `src/ui_egui/curvature.rs` and has its own cubic evaluation. It is toggled via `ZColorPickerOptions::is_show_curvature_comb`.

### Presets
- `src/preset.rs`: one JSON file per `PresetEntity` in `./presets/` (the `presets/` folder is gitignored). The path comes from `std::env::current_dir()`.
- Presets are written through to disk: Create, Save, Rename and Delete in the preset bar (`WindowPreset::draw_content`, `ZApp::show_create_preset_window`) hit disk immediately via `PresetHandler`. Nothing is synced at exit.
- When `auto_save_presets` is enabled, the selected preset is rewritten only when the current control points or spline mode differ from it.
- On startup, `init_presets` reloads the presets from disk, overriding the copy in eframe storage.

### Clipboard / screen capture
Pressing the middle button stores a target rect in `pending_screenshot_rect` and requests one `ViewportCommand::Screenshot`. The target is the previewer rect under the cursor (rects come from `stored_ui_responses`), or else a 1x1 pixel. The screenshot event arrives in a later frame. `handle_screenshot_event` crops it to the stored rect and turns it into a `ClipboardCopyEvent`. That event is consumed at the start of the next `Idle` frame (`handle_clipboardcopy_event`). A single pixel is copied as text in `color_copy_format`; a larger region is copied as an image via `arboard`.

### Misc
- `main.rs` forces `RUST_LOG=debug`.
- `LogCollector` (`src/logger.rs`) captures log lines into a shared `Arc<Mutex<Vec<String>>>`, capped at 2000 lines, which the Log pane displays. Never call `log::*` while holding that mutex; `LogCollector::log` locks it too, so it would deadlock.
- The UI scale is derived from a hardcoded 2560x1440 monitor size (`HARDCODED_MONITOR_SIZE`) because the monitor size is not available at creation time.
- The debug windows live in `src/ui_egui/debug_windows.rs`.
- Windows-only dependency: `winapi`.
