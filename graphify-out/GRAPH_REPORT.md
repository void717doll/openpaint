# Graph Report - openpaint  (2026-11-10)

## Corpus Check
- Corpus is ~36,446 words - fits in a single context window. You may not need a graph.

## Summary
- 713 nodes · 1604 edges · 27 communities (24 shown, 3 thin omitted)
- Extraction: 97% EXTRACTED · 3% INFERRED · 0% AMBIGUOUS · INFERRED: 51 edges (avg confidence: 0.75)
- token usage and cost: unavailable for semantic extraction; agent tools did not expose usage. the later code-only refresh used no llm.

## Community Hubs (Navigation)
- packaging and release scripts
- interface rendering and panels
- editor input and jobs
- tool selection catalog
- layer blend modes
- unicode text editing
- menus and commands
- drawing and selection tools
- documents layers and history
- interface action dispatch
- release workflow triggers
- release platform matrix
- release signing configuration
- release artifact transfers
- release environment settings
- package job configuration
- release publication permissions
- release job steps
- modal input actions
- cargo package manifest
- continuous integration checks
- project files and saves
- adjustments and effects
- application icon artwork
- architecture parity and reference

## God Nodes (most connected - your core abstractions)
1. `Document` - 76 edges
2. `Action` - 74 edges
3. `Editor` - 44 edges

## Surprising Connections (you probably didn't know these)
- `resource-supported feature inventory` --semantically_similar_to--> `editing tools and layers`  [INFERRED] [semantically similar]
  docs/reference.md → README.md
- `distribution boundary` --semantically_similar_to--> `reference artifact exclusion`  [INFERRED] [semantically similar]
  docs/architecture.md → THIRD_PARTY.md
- `DejaVu Sans` --references--> `dependency and font notices`  [EXTRACTED]
  assets/DejaVuSans-LICENSE.txt → THIRD_PARTY.md

## Import Cycles
- None detected.

## Hyperedges (group relationships)
- **snapshot edit flow** — docs_architecture_raster_model, docs_architecture_history, docs_architecture_effect_jobs [EXTRACTED 1.00]
- **native package matrix** — docs_releases_linux_packages, docs_releases_windows_packages, docs_releases_macos_packages, docs_releases_artifact_verification [EXTRACTED 1.00]

## Communities (27 total, 3 thin omitted)

### Community 6 - "packaging and release scripts"
Cohesion: 0.14
Nodes (20): collect(), appimage(), bundle_x11_libraries(), copy(), copy_notices(), deb(), glibc_requirement(), linux() (+12 more)

### Community 0 - "interface rendering and panels"
Cohesion: 0.07
Nodes (54): Field, Modal, ModalKind, Model, Output, Ui, ACCENT, CloseTab (+46 more)

### Community 1 - "editor input and jobs"
Cohesion: 0.06
Nodes (33): ClipboardJob, Drag, DragTransform, Editor, EffectJob, EffectPreview, FileDialogJob, Tab (+25 more)

### Community 10 - "tool selection catalog"
Cohesion: 0.08
Nodes (23): Tool, .ALL, CloneStamp, ColorPicker, Ellipse, EllipseSelect, Eraser, Gradient (+15 more)

### Community 11 - "layer blend modes"
Cohesion: 0.12
Nodes (16): BlendMode, Additive, .ALL, ColorBurn, ColorDodge, Darken, Difference, Glow (+8 more)

### Community 12 - "unicode text editing"
Cohesion: 0.16
Nodes (12): TextEdit, main(), run(), window_conf(), window_icon(), edit_text(), editing_preserves_unicode_boundaries(), line_bounds() (+4 more)

### Community 13 - "menus and commands"
Cohesion: 0.19
Nodes (10): MenuCommand, MenuItem, About, Action, Effect, New, Properties, Resize (+2 more)

### Community 2 - "drawing and selection tools"
Cohesion: 0.08
Nodes (59): Pixel, GradientKind, SelectionMode, SelectionTransform, ToolSettings, brush_coverage(), clone_stroke(), color_distance() (+51 more)

### Community 3 - "documents layers and history"
Cohesion: 0.08
Nodes (25): Document, HistoryEntry, Layer, PendingEdit, Snapshot, blend_channel(), blend_modes_match_basic_opaque_cases(), cancelled_and_empty_edits_do_not_create_history() (+17 more)

### Community 4 - "interface action dispatch"
Cohesion: 0.04
Nodes (46): Action, ActualSize, AddLayer, CancelEffect, ConfirmCancel, ConfirmDiscard, ConfirmSave, Copy (+38 more)

### Community 14 - "release workflow triggers"
Cohesion: 0.18
Nodes (10): uses, concurrency, cancel-in-progress, group, jobs, checks, name, on (+2 more)

### Community 15 - "release platform matrix"
Cohesion: 0.20
Nodes (10): arch, executable, platform, runner, rustflags, target, include, strategy (+2 more)

### Community 16 - "release signing configuration"
Cohesion: 0.22
Nodes (9): APPLE_APP_PASSWORD, APPLE_ID, APPLE_TEAM_ID, MACOS_CERTIFICATE_BASE64, MACOS_CERTIFICATE_PASSWORD, MACOS_SIGNING_IDENTITY, WINDOWS_CERTIFICATE_BASE64, WINDOWS_CERTIFICATE_PASSWORD (+1 more)

### Community 17 - "release artifact transfers"
Cohesion: 0.22
Nodes (9): with, compression-level, if-no-files-found, merge-multiple, name, path, pattern, persist-credentials (+1 more)

### Community 19 - "release environment settings"
Cohesion: 0.25
Nodes (8): CARGO_TERM_COLOR, GH_TOKEN, MACOSX_DEPLOYMENT_TARGET, RELEASE_TAG, RUSTFLAGS, RUSTUP_TOOLCHAIN, env, env

### Community 20 - "package job configuration"
Cohesion: 0.29
Nodes (7): run, package, defaults, name, runs-on, timeout-minutes, shell

### Community 21 - "release publication permissions"
Cohesion: 0.29
Nodes (7): publish, permissions, contents, needs, permissions, runs-on, timeout-minutes

### Community 22 - "release job steps"
Cohesion: 0.38
Nodes (7): steps, steps, if, name, run, shell, uses

### Community 24 - "modal input actions"
Cohesion: 0.40
Nodes (5): OpenPath, Primary, SavePath, Secondary, modal_action()

### Community 7 - "continuous integration checks"
Cohesion: 0.05
Nodes (39): defaults, env, name, runs-on, steps, strategy, timeout-minutes, concurrency (+31 more)

### Community 8 - "project files and saves"
Cohesion: 0.10
Nodes (23): atomic_write(), exporting_without_alpha_does_not_mark_project_saved(), extension(), failed_save_preserves_original(), file_name(), first_undo_after_loading_restores_loaded_layers(), invalid_documents_and_oversized_metadata_preserve_existing_files(), layered_round_trip() (+15 more)

### Community 9 - "adjustments and effects"
Cohesion: 0.16
Nodes (29): accumulate(), adjust_hsv(), apply(), auto_levels(), auto_levels_ignores_invisible_pixels(), blur_axis(), blur_does_not_leak_transparent_colors(), byte() (+21 more)

### Community 23 - "application icon artwork"
Cohesion: 0.50
Nodes (5): blue checkerboard, image transparency, paintbrush, openpaint raster icon, openpaint vector icon

### Community 5 - "architecture parity and reference"
Cohesion: 0.06
Nodes (37): Bitstream Vera license, DejaVu Sans, MIT license, ProggyClean, effect preview jobs, history and savepoints, .opaint project format, raster model (+29 more)

## Knowledge Gaps
- **113 isolated node(s):** `Confirm`, `Effect`, `Message`, `New`, `Path` (+108 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 237 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **3 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `Document` connect `documents layers and history` to `interface rendering and panels`, `editor input and jobs`, `drawing and selection tools`, `project files and saves`?**
  _High betweenness centrality (0.139) - this node is a cross-community bridge._
- **What connects `Confirm`, `Effect`, `Message` to the rest of the system?**
  _113 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `packaging and release scripts` be split into smaller, more focused modules?**
  _Cohesion score 0.13821138211382114 - nodes in this community are weakly interconnected._
## extraction limits

- graph integrity: 102 dangling-endpoint edges, 97 directed and 98 undirected collapsed edges; 0 missing endpoint fields and 0 self-loops. these are extractor/representation limits, not confirmed application defects. see `graph-health.txt`.
- all nine documents and both images were fully read or viewed. cached semantic output was preserved during the packaging fix. `Cargo.toml` and `src/lib.rs` yielded no symbols; file nodes remain.
- raw cohesion values are in `community-summary.json`; no quality grade is assigned.
- eight unclassified files remain outside extraction: `.gitignore`, `LICENSE`, `assets/DejaVuSans.ttf`, `assets/openpaint.ico`, `packaging/openpaint-mime.xml`, `packaging/org.openpaint.OpenPaint.desktop`, `packaging/org.openpaint.OpenPaint.metainfo.xml`, and `packaging/windows.nsi`.
- reference binaries, extracted payloads, caches and build output were excluded. no sensitive files were reported skipped. wine tests stopped before application startup; no functional parity is established.
- the html viewer loads vis-network 9.1.6 from unpkg.com and needs network access for that library. browser interaction was not verified.
- `graphify-out/` is untracked in the working tree, not ignored. only intended shareable outputs should be committed; caches and backups are local run artifacts.
- `benchmark.txt` contains estimated query token counts, not measured agent usage or answer-quality results.
