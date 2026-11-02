# Z-Color-Picker

![alt text](img/new_egui_tiles.png)

# MAIN FEATURES

- Easy creation of gradients
- Interactive ui
- Gradient preview
- Multiple Spline Modes
- Create/Save/Load Presets
- Quickly Export
- Coded in Rust

# HOW TO

## Usage

Each control point is a value in a color space (currently only HSV).
Hue of each individual control point can be changed in the hue bar, and saturation & value in the 2d slider.
The selected control point's color is shown in the top left. The previewer shows the control point colors, the gradient using the selected spline mode, and a quantized gradient.
To create a new preset, click the preset combo box and select `<NEW>`

## Controls

- Double-click-M1: Add control-point
- Drag-M1: Move a control-point, a Bezier tangent handle, or a hue/T gizmo
- Right-click-M2: Remove control-point
- Drag-M1 on a previewer key color: resize its band
- Click the color readout or Ctrl+C: copy the selected point's color in the Copy-as format
- Middle-click: Copy screen color as the selected format, on previewer, copies image to clipboard
- F12: debug window with the control point values
- Esc: quit (ignored while typing, or while a popup, dialog or the Create Preset window is open)

## Options

- Lock: Apply any control point coordinate translation to all control points
- Auto hue: Select hue for start/end control points, middle control point's hue is automatically selected
- Insert left/right: Side of the selected point where a new control point goes (start/end when none is selected)
- Clamp: Keep control points inside the S/V square and wrap hue into 0..1
- Comb: Show the curvature comb (Bezier only)
- Auto comb: Line up each point's two handles and solve their lengths so the comb is continuous (Bezier only); dragging a handle then only turns it. `N unsolved` counts points where the handle directions allow no solution
- Copy as: Text format for a copied color; previewer strips are copied as images
- Spline: Linear, Bezier (drag the tangent handles) or Hermite (Catmull-Rom, automatic tangents)
- Flip: Reverse the control point order
- Auto-save: Rewrite the selected preset file after every edit
- Levels (previewer): Number of colors in the quantized gradient
- Reset widths (previewer): Make all key color bands equal width

## Presets

- One JSON file per preset in `./presets`, relative to the working directory
- The combo marks unsaved changes with `*`; `<None>` clears the selection
- Picking another preset with unsaved changes asks to save, discard or cancel
- Save without a selection creates a preset; Delete asks first

# TODO:

## References
### https://acko.net/blog/curvature-beziers/

## TODO: Features

# Currently being worked on
- Control point ordering visuals on color picker
- Control point Hue visuals
- Size slider for color picker spline
- T should be modifiable by user, potentially normalized on display
- Spline mode interactivity

#

- Add confirm window for exiting when having an unsaved preset active.
- Regular Color pick for control points
- Visualization curves
  - Value
  - Saturation
  - Hue
- View multiple presets at the same time
- Photoshop workflow, plugin integration?
- Different color spaces
- Better ui widget dynamic rezising
- Polynomial spline mode
- Auto gradient button with "undo"
- Add Interpolation::Bezier with just one tanget point
- 3D visualization
- Screen-mouse gradient picker
