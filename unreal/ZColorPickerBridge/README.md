# ZColorPickerBridge

Unreal Engine editor plugin: receives gradients from Z-Color-Picker over Remote Control and writes them into Curve Linear Color assets. Built and tested on UE 5.5.4 (automation test and a live Remote Control round trip) and on a 5.8.3 source build (compiled, automation test passed; no live run there).

## Install

- Copy this folder to `<Project>/Plugins/ZColorPickerBridge`.
- C++ project: the editor builds the plugin on open.
- Blueprint-only project: build a package, then copy the package's folder to `<Project>/Plugins/ZColorPickerBridge` instead:
  ```
  <UE>/Engine/Build/BatchFiles/RunUAT.bat BuildPlugin -Plugin=<abs path>/ZColorPickerBridge.uplugin -Package=<abs output dir> -TargetPlatforms=Win64
  ```
  Keep the `-Package` path short: UBT refuses intermediate paths of 260 characters or more, and those reach about 150 characters below it. A package is pinned to the engine that built it (`EngineVersion` in its `.uplugin`): build one per engine.
- A prebuilt UE 5.5 package is in `unreal/packaged/UE_5.5/ZColorPickerBridge` on the machine that built it (gitignored, not in the repo).
- The plugin enables Remote Control itself (a dependency in its `.uplugin`).
- Source-built engine: Remote Control's modules (and `WebSocketNetworking`) are only built when some project enables them. If the editor reports them missing or out of date, build your C++ project once with this plugin added, which compiles them into the engine.

## What it does

`UZColorPickerBridgeLibrary::ApplyGradient(AssetPath, Colors)`, static and BlueprintCallable:

- Creates a `CurveLinearColor` at `AssetPath` when none exists, else updates the existing one in place (same object, references stay valid).
- Keys color `i` of `n` at time `i / (n - 1)` on R, G, B and A, linear interpolation. One color is one key at 0.
- Notifies curve listeners (Curve Atlases re-render) and marks the package dirty. It does not save; save it like any other edit.
- Returns `""` on success, else the error: `AssetPath` is not a valid long package name under `/Game/`, `Colors` is empty or has a non-finite value, or another asset class sits at that path.

## The call it serves

```
PUT http://127.0.0.1:30010/remote/object/call
Content-Type: application/json

{"objectPath":"/Script/ZColorPickerBridge.Default__ZColorPickerBridgeLibrary",
 "functionName":"ApplyGradient",
 "parameters":{"AssetPath":"/Game/ZColorPicker/<Name>",
               "Colors":[{"R":0.1,"G":0.2,"B":0.3,"A":1.0}, ...]},
 "generateTransaction":false}
```

- `Content-Type` must be exactly `application/json`; a `; charset=...` suffix gets a 400.
- `AssetPath`: `<asset folder>/<asset name>`, folder default `/Game/ZColorPicker`, name = the export name with every character outside `[A-Za-z0-9_]` replaced by `_`.
- `Colors`: Z-Color-Picker sends 64 samples at T = i/63, linear sRGB, A = 1.
- Plugin result: HTTP 200 with `{"ReturnValue":""}`, or `{"ReturnValue":"<error>"}`.
- Remote Control failure: 400, 404 or 500 with `{"errorMessage":"..."}`.
- Connection refused: the editor is not running, or the plugin or Remote Control is not enabled.

## Remote Control

- Its HTTP server starts with the editor, on port 30010. Change it in Project Settings > Plugins > Remote Control (Remote Control HTTP Server Port), and set the same port in Z-Color-Picker.
- It listens on loopback only by default.
- It does not start under `-nullrhi` or `-RCWebControlDisable`, or when the port is taken (the editor log shows the bind error).

## Using the curve

- Materials: add the curve to a Curve Atlas (`CurveLinearColorAtlas`) and sample its row with a Curve Atlas Row Parameter. The atlas re-renders on every update.
- Niagara: a Color Curve data interface copies the curve data when the asset is picked and does not follow later updates; re-select the asset (or recompile the system) to resync. Niagara's import-from-asset swaps green and blue (an engine bug, `NiagaraDataInterfaceCurveTypeEditorUtilities.cpp`, `ImportSelectedAsset`); set the data interface's Curve Asset instead.

## Test

Automation test `ZColorPicker.ApplyGradient` (Session Frontend, or `UnrealEditor-Cmd.exe <Project>.uproject -ExecCmds="Automation RunTests ZColorPicker;Quit" -unattended -nullrhi`). It creates and deletes assets under `/Game/ZColorPickerTests/`.
