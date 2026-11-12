# reference investigation

the feature-to-implementation mapping and retained REA evidence ids are summarized in the [binary audit](binary-audit.md).

the supplied installer was inspected statically on linux. no paint.net process was executed, no original application screenshots were captured, and no behavioral or pixel-level equivalence has been established.

## artifact identity

| artifact | bytes | sha-256 |
| --- | ---: | --- |
| `paint.net.5.1.12.install.x64.exe` | 84380680 | `9c34b285eabcc4658335ad91f9c76ccf8f96d939b9640fac08e4942e394e1e8d` |
| extracted `x64/paintdotnet.dll` | 19610920 | `313fad2fb569baf7248a69d1cb981fd713187270abaee64849a723db90013aba` |

7-zip identified the outer executable as `SetupSfx.exe`, containing an embedded 7z archive. the full inventory has 536 files and one directory, totaling 627606068 uncompressed bytes. the installer is not the image editor executable. its version resource reports `5.112.9563.32325`.

the extracted `paintdotnet.dll` is a managed x86-64 PE containing both CIL and ReadyToRun code, with module id `76c45a70-d224-4997-9eec-e7763aec7976`. REA identifies `.NETCoreApp,Version=v9.0`. the shipped runtime configuration lists `Microsoft.NETCore.App` and `Microsoft.WindowsDesktop.App` at `9.0.13`. this is not a NativeAOT-only target.

## reproducible local analysis

analysis outputs and extracted files belong in the ignored `analysis/` directory. they are not application dependencies or release contents.

```sh
sha256sum paint.net.5.1.12.install.x64.exe
7z l -slt paint.net.5.1.12.install.x64.exe > analysis/reference/installer-inventory.txt
7z x -y -oanalysis/reference/extracted paint.net.5.1.12.install.x64.exe \
  'x64/PaintDotNet*' 'x64/paintdotnet*' 'x64/Resources/*' 'x64/License.txt'
rea doctor --client codex --json
rea inspect-managed-artifact analysis/reference/extracted/x64/paintdotnet.dll --json
rea inspect-managed-members analysis/reference/extracted/x64/paintdotnet.dll --json
rea inspect-managed-members analysis/reference/extracted/x64/PaintDotNet.Data.dll --json
rea inspect-managed-members analysis/reference/extracted/x64/PaintDotNet.Effects.dll --json
```

this run used the existing public REA 6.7.0 launcher at `/home/mia/.npm/_npx/9b9809298fe40d2c/node_modules/.bin/rea` with `/home/mia/.nvm/versions/node/v22.22.3/bin` prepended to `PATH`. the doctor reported missing MCP registration and native engines. no client configuration or analysis-engine installation was changed. the portable managed CLI inspections worked without those providers.

| evidence | result |
| --- | --- |
| `ev_dd9ccb42503840788f23b4ca412ace26386b0e802424a9a1c7c5d74b9a44b88f` | main assembly identity and metadata inventory complete; 1870 type definitions and 13936 method definitions |
| `ev_069c8e8a5e7ce5aecc656e354e1136c0c64c953f98844853b027f1f185e72c78` | main assembly member inspection unavailable: metadata representation exceeds REA's 67108864-byte budget |
| `ev_5c6418e1c9ba4c69845764b61ef36494b90db663b7379ddea688f4920dfceadb` | data assembly member inspection complete: 117 types and 876 methods |
| `ev_177915179d117277d293f6c9c20b81fcdb41f8d67d4ffa3aff7feb0f68ddd402` | effects assembly member inspection complete |

the original REA records are retained as `analysis/reference/rea-*.json`. complete metadata coverage does not mean complete implementation recovery, dependency resolution, execution, or decompilation. no recovered proprietary code was copied into openpaint.

## resource observations

the shipped English `PaintDotNet.Strings.3.resources` contains 1626 entries. a local format-aware reader decoded the resource table and its string values into `analysis/reference/strings-en.json`. this is ordinary shell/Python inspection, not REA evidence. the format was cross-checked against the [.NET resource reader](https://github.com/dotnet/runtime/blob/main/src/libraries/System.Private.CoreLib/src/System/Resources/ResourceReader.cs).

the following are resource-supported requirements. resource presence alone does not prove a command is reachable or establish exact implementation semantics.

exact display labels for the main tool set are `Rectangle Select`, `Lasso Select`, `Ellipse Select`, `Magic Wand`, `Move Selected Pixels`, `Move Selection`, `Zoom`, `Pan`, `Paint Bucket`, `Gradient`, `Paintbrush`, `Eraser`, `Pencil`, `Color Picker`, `Clone Stamp`, `Recolor`, `Text`, `Line / Curve`, and `Shapes`.

| resource family | exact display labels |
| --- | --- |
| selection mode | `Replace`, `Add (union)`, `Subtract`, `Intersect`, `Invert ("xor")` |
| selection sizing | `Any Size`, `Fixed Ratio`, `Fixed Size` |
| flood mode | `Contiguous`, `Global` |
| gradient geometry | `Linear`, `Linear (Reflected)`, `Diamond`, `Radial`, `Conical`, `Spiral (Clockwise)`, `Spiral (Counter Clockwise)` |
| gradient repetition | `No Repeat`, `Repeat Reflected`, `Repeat Wrapped` |
| shape rendering | `Draw Shape Outline`, `Draw Filled Shape`, `Draw Filled Shape With Outline` |
| tool fields | `Hardness:`, `Spacing:`, `Tolerance alpha mode`, `Pressure sensitivity`, `Path smoothing`, `Gamma mode`, `Selection quality`, `Blending mode` |
| resize filters | `Adaptive (Sharp)`, `Bicubic`, `Bicubic (Smooth)`, `Fant`, `Lanczos`, `Bilinear`, `Bilinear (Low Quality)`, `Nearest Neighbor` |

- tool names include rectangle select, lasso select, ellipse select, magic wand, move selected pixels, move selection, zoom, pan, paint bucket, gradient, paintbrush, eraser, pencil, color picker, clone stamp, recolor, text, line/curve, and shapes. legacy rectangle/ellipse/rounded-rectangle labels also remain in the resources; this does not establish separate current toolbar buttons.
- primary and secondary colors are selected by left and right mouse buttons. the color picker uses Ctrl for merged-layer sampling. clone stamp uses Ctrl+click to select its anchor.
- selection combination includes replace, union, subtraction, intersection, and xor. resources associate Ctrl+left with union, Alt+left with subtraction, Ctrl+right with xor, and Alt+right with intersection.
- shift constrains rectangular/elliptical selections and drawing angles. alt enables centered line/shape drawing. move-tool help describes resizing and rotation handles; right dragging rotates selected pixels.
- flood tools offer contiguous and global matching. tolerance includes premultiplied and straight-alpha modes; the resource describes premultiplied comparison as the default.
- gradient modes include linear, reflected linear, diamond, radial, conical, and both spiral directions. repeat modes include none, reflected, and wrapped.
- tool options include brush width, hardness, spacing, pressure sensitivity, antialiasing, gamma, path smoothing, line styles/caps, text size/style, selection quality, and shape fill/outline modes.
- utility windows are tools (F5), history (F6), layers (F7), and colors (F8), with separate reset commands.
- image commands include resize, canvas size, crop, flips, quarter-turn rotation, flatten, and color profiles. canvas-size help says newly exposed space uses the secondary color.
- layer commands include add/delete/duplicate, merge down, reorder, import, visibility, properties, and transformations.

layer blend-mode resources enumerate normal, multiply, additive, color burn, color dodge, reflect, glow, overlay, difference, negation, lighten, darken, screen, and xor. a separate content-blending list also includes overwrite. layer blending and tool overwrite must not be conflated.

adjustment resources include auto-level, black and white, brightness/contrast, curves, exposure, highlights/shadows, hue/saturation, invert alpha/colors, levels, posterize, sepia, and temperature/tint. exact grouping and ordering require runtime verification.

effect labels include gaussian, bokeh, fragment, median, motion, radial, sketch, square, surface, unfocus, and zoom blurs; bulge, dents, frosted glass, tile reflection, and twist distortions; add/reduce noise, median, and quantize; clouds, julia/mandelbrot fractals, and turbulence; ink sketch, oil painting, and pencil sketch; glow, red-eye removal, sharpen, soften portrait, straighten, vignette; crystalize, edge detect, emboss, outline, relief, morphology, and drop shadow. resources also identify artistic, blurs, color, distort, noise, object, photo, render, and stylize categories. algorithm parameters, numerical behavior, and placement remain unverified.

## native project format and extension boundary

the data assembly contains the literal `PDN3` at file offset 1362613. REA records these anchors in `PaintDotNet.Document`:

- `FromStreamImpl`, token `0x06000077`, reads header bytes, parses UTF-8 XML, invokes `BinaryFormatter.Deserialize` with a custom binder, and invokes `DeferredFormatter.FinishDeserialization`. a separate path constructs a `GZipStream` before deserialization.
- `SaveToStream`, token `0x06000079`, emits a document header followed by `BinaryFormatter.Serialize` and `DeferredFormatter.FinishSerialization` calls.
- the shipped XML API documentation describes `Document.FromStream` as the supported document deserialization entry point.

these are static calls, not a complete format specification. a correct rust codec still needs bounded parsing of the serialization records, layer/type mappings, deferred pixel payloads, format versions, and reference-generated interoperability samples. a JSON/PNG project file cannot be relabeled as a compatible `.pdn` file.

plugin support is also a separate boundary. the payload contains managed effect APIs, compatibility components, GPU effects, and file-type interfaces. no rust implementation of the host ABI or managed plugin runtime has been verified.

## documented visual supplement

the [official main-window documentation](https://paint.net/doc/latest/MainWindow.html), revised in 2023, describes a menu row, standard toolbar, active-tool options, document thumbnails, gray canvas surroundings, four utility windows, and a bottom status/zoom area. its illustrated arrangement places tools and colors to the left and history and layers to the right. it documents an 800×600 initial canvas at 100% display scaling.

the [official shortcut documentation](https://paint.net/doc/latest/KeyboardMouseCommands.html), revised in 2024, supplements the resource evidence: selection tools share S; move tools share M; brush B, pencil P, fill F, gradient G, eraser E, picker K, recolor R, clone L, text T, line/shapes O, pan H, and zoom Z. X swaps colors. space-drag pans and Ctrl+wheel zooms.

these pages are older documentation, not observation of the supplied 5.1.12 executable. exact spacing, window dimensions, icons, default values, interaction timing, and rendered pixels remain unmeasured. the documentation screenshot was accessible through the web tool, but a local download failed because shell DNS access was unavailable.

wine 10.0 is now installed. on 2026-11-10, isolated launches of both the installer and the fully extracted application stopped before startup because wineserver could not create `/run/user/1000/wine` on the restricted filesystem. display `:0` also remained inaccessible. [wine test records](wine-reference.md) document the attempts, retained logs, and reproduction commands; no original-application functionality has been verified yet.

## remaining evidence work

- run the actual reference in a suitable windows environment and capture each feature at controlled DPI, theme, and canvas size.
- obtain actual `.pdn` fixtures covering layers, modes, opacity, metadata, and format versions.
- inspect missing main-assembly member paths through bounded metadata tooling or a suitable managed decompiler.
- compare matched pointer/keyboard scenarios, screenshots, and pixel outputs. independent implementation tests do not substitute for comparison with the reference.
