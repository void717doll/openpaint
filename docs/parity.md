# parity checklist

this is a comparison checklist, not a claim of complete paint.net compatibility. `not compared` means the original 5.1.12 application has not been exercised against openpaint in a matching runtime scenario. implementation status must be established separately by tests and GUI checks.

reference sources and REA evidence are recorded in [reference.md](reference.md). all rows currently lack original-application runtime comparison. the following implementation inventory records the actual current scope; it does not mark the acceptance scenarios below as passed.

| area | implemented | remaining differences |
| --- | --- | --- |
| shell/documents | native window, menus with keyboard navigation, thumbnail tabs, recent files, movable utility panels, close prompts, save-all | tab reordering, persistent panel positions, measured geometry/themes, accessibility |
| viewport/colors | pointer-centered wheel zoom, actual size/fit/fit-selection, pan, palette, rgba and hex, primary/secondary sampling | rulers, pixel grid, color wheel, complete reference zoom/scroll semantics |
| drawing | continuous brush/pencil/eraser, width/hardness/opacity, selection clipping, secondary color | pressure, spacing, path smoothing, gamma settings, exact stroke coverage |
| selections/transforms | four selection tools, five combination modes, fixed-size/ratio selections, move pixels/outline, corner scaling, right-drag rotation, constrained transforms, nudge | editable transform numeric controls, exact reference interpolation |
| gradients/shapes | seven gradient geometries and repeat modes; line, rectangle, ellipse, rounded rectangle; outline/fill and constraints | editable gradient/curve handles, complete shapes, caps/dashes |
| clone/recolor/text | clone source, tolerance replacement, bundled font, size, multiline text and unicode field editing | font chooser, styles/alignment, movable text before commit, reference clone alignment modes |
| layers/history | fourteen blend modes, layer import, operations/properties, undo/redo, history navigation, dirty revisions | reference rounding/gamma verification, exact operation grouping |
| image transforms | premultiplied bilinear resize, anchored/fill canvas resize, crop, flips, quarter-turns | filter selector, aspect-lock controls |
| clipboard | cut/copy/copy-merged/paste, paste into layer/image, selection alpha, native helper integration | native clipboard validation on each host |
| adjustments/effects | 27 implemented effects plus exposure, highlights/shadows, levels, and curves; selected-region application, asynchronous preview/apply/cancel, repeat-last | complete reference catalog and parameters, exact GPU/color numerical match, graphical curves, automatic preview |
| files | bounded `.opaint` projects; png/bmp/tga/ico/pnm saving; webp/dds import; safe atomic saves | `.pdn`, jpeg/gif/tiff/avif/heic/jxl, icc/metadata, export-quality dialogs |
| plugins | none | managed/GPU plugin host and ABI compatibility |
| packaging | all requested workflows/scripts; actual linux deb built and extracted | hosted jobs, rpm/AppImage tooling runs, windows/mac native packaging, installation/uninstallation and signing |

## reference acceptance scenarios

| area | acceptance scenario | reference comparison |
| --- | --- | --- |
| shell | compare menus, toolbar, tool options, document thumbnails, utility windows, status/zoom, DPI scaling, and theme | not compared |
| documents | create multiple images, switch/reorder tabs, open recent files, save, save-as, save-all, and cancel dirty closes | not compared |
| viewport | zoom at cursor, actual size, fit window/selection, space/middle-button pan, rulers and pixel grid | not compared |
| colors | primary/secondary colors, swap, wheel/palette, alpha, numeric channels, and picker sampling | not compared |
| drawing | brush/pencil/eraser, smooth strokes, right-button color, width/hardness/spacing, pressure, antialiasing, gamma | not compared |
| flood tools | contiguous/global fill and wand, tolerance, alpha mode, modifiers, and selection clipping | not compared |
| selections | rectangle, ellipse, lasso, wand, five combination modes, invert, deselect, selection clipboard | not compared |
| move tools | move selected pixels versus selection outline, copy-move, resize, rotate, constraints, commit/cancel | not compared |
| gradients | seven geometry modes, repeat modes, alpha blending, editable handles, direction swap, commit/cancel | not compared |
| clone/recolor | clone anchor and aligned sampling; primary/secondary replacement with tolerance | not compared |
| text | font selection, size, styles, alignment, rendering, multiline input, reposition before commit | not compared |
| lines/shapes | curve handles, line caps/dashes, outline/fill, shape catalog, constraints, editable transforms | not compared |
| layers | add/delete/duplicate/import, reorder, visibility, name, opacity, merge down, flatten | not compared |
| blending | all fourteen reference layer modes, transparency boundaries, rounding, gamma, merge equivalence | not compared |
| history | one step per operation, undo/redo, history navigation, saved-state tracking, redo invalidation | not compared |
| image transforms | resize sampling/aspect ratio, canvas anchoring/fill, crop, flips, quarter-turns | not compared |
| clipboard | cut/copy/copy-merged, paste, paste into image/layer, partial alpha and external interoperability | not compared |
| adjustments | complete adjustment inventory, parameter ranges/defaults, preview/accept/cancel, selected-region handling | not compared |
| effects | complete effect inventory from reference.md, categories, parameters, preview, cancellation, repeat last | not compared |
| file codecs | reference input/output formats, export settings, transparency, profiles, metadata, unsupported-file errors | not compared |
| native project format | actual `.pdn` samples read/write correctly in both applications | no interoperable codec verified |
| plugins | actual reference effect/file-type plugins load and run through a compatible host | no binary compatibility verified |
| platform integration | dialogs, clipboard, file associations, open-with, icons, printing/scanning where applicable | not compared |
| packaging | install/run/uninstall each deb, rpm, AppImage, windows x64, macos arm64, and macos x64 artifact | requires platform-specific validation |

local checks verify openpaint's own invariants even before the original application is available. passing those checks does not change the reference-comparison column. retain discrepancies until a matching scenario proves them resolved.

## local verification

on 2026-11-11, `cargo fmt --all -- --check`, `cargo check --offline --locked --all-targets`, and `git diff --check` passed after the REA-guided parity changes. the test suite was not rerun during this pass.

on 2026-10-30, `cargo fmt --all -- --check`, `cargo clippy --offline --locked --all-targets -- -D warnings`, all 52 tests, and `cargo build --offline --locked --release` passed. tests cover alpha/compositing, selections and transforms, undo/savepoints, corrupted projects and failed saves, effect previews/cancellation, dirty-close handling, and unicode/menu editing helpers.

the deb package was built, inspected, extracted into a staging directory, and its executable passed `--version` and `--help`. desktop and AppStream metadata validation and workflow YAML/Python parsing passed. the locally produced deb requires glibc 2.39 or newer; hosted linux builds use ubuntu 22.04 and derive their dependency minimum from the resulting binary.

launching the linux GUI failed at `XOpenDisplay() failed!` because the session cannot access the desktop display. no interactive screenshots or pointer-path acceptance checks passed. no windows/mac executable was run, no hosted workflow ran, and no release was published. repository initialization and commits were blocked by the read-only `.git` mount.
