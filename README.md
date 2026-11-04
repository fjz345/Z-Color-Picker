# Z-Color-Picker

![alt text](img/preview_newest.png)

# MAIN FEATURES

- Easy creation of gradients
- Interactive ui
- Gradient preview
- 3D gradient view
- Multiple Spline Modes
- Multiple Color Spaces
- Curvature comb
- Create/Save/Load Presets
- Quickly Export
- Coded in Rust

# HOW TO

## Usage

Place control points on the square and hue bar; the gradient is interpolated through them. "Auto hue" & "Auto Lightness" are core features that auto adjust control points for you. Adjust control points with the color picker or 3d viewer.

### Controls

- Double-click: Add control-point
- Right-click: Remove control-point
- Drag: Move a control-point
- Middle-click: Copy screen color as the selected format, on previewer, copies image to clipboard
- 3D view: Orbit & Zoom
- F12: Debug window
- Esc: Quit

### Presets

- One JSON file per preset in `./presets`
- `<NEW>` in the combo creates a preset, `<None>` clears the selection
- `*` in the combo marks unsaved changes
- Switching away from unsaved changes asks to save, discard or cancel

# TODO:

## References
### https://acko.net/blog/curvature-beziers/

## TODO: Features

- "Regular Color pick" for control points
- Visualization curves
  - Value
  - Saturation
  - Hue
- View multiple presets at the same time
- Photoshop workflow, plugin integration?
- "Auto gradient button"
- "Random gradient button"
- "Screen-mouse-drag" gradient picker
- Undo feature
