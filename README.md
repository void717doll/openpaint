# openpaint

a native rust raster editor developed on linux, using the supplied paint.net 5.1.12 installer as a behavioral reference. this is an independent implementation with original code and artwork. it is not yet a 1:1 replacement.

```sh
cargo run --release
cargo run --release -- picture.png
```

the desktop build uses macroquad for the native window, input, and rendering. there is no browser or webview. linux needs a working x11/xwayland display and opengl driver. windows x64 and macos arm64/x64 have native build workflows.

## editing

- dark interface with multiple documents, recent files, dirty-close prompts, undo/redo, and history.
- layers with visibility, names, opacity, fourteen blend modes, merging, and flattening.
- brush, pencil, eraser, fill, five gradients, color picker, clone stamp, recolor, text, lines, rectangles, ellipses, and rounded rectangles.
- rectangle, ellipse, lasso, and magic-wand selections; replace, union, subtract, intersect, and xor modes.
- selection movement/scaling/rotation, cropped copying, pasting, document resizing, canvas resizing, cropping, flips, and rotation.
- 27 adjustments/effects, background computation, and explicit preview/apply/cancel.
- primary/secondary rgba colors, numeric/hex editing, document thumbnails, movable panels, zoom at the pointer, panning, and keyboard menu navigation.

file dialogs use zenity/kdialog on linux, powershell's system dialogs on windows, and osascript on macos. an in-app path field is available when native dialogs cannot launch. system image clipboard integration uses xclip, powershell, or osascript; an internal clipboard remains available between openpaint documents.

## files

| format | open | save |
| --- | --- | --- |
| `.opaint` layered project | yes | yes |
| png, bmp, tga, ico, pnm | yes | yes |
| webp, dds | decoder support from image 0.24 | no |
| jpeg, gif, tiff, avif, heic, jxl | not implemented | not implemented |
| paint.net `.pdn` | not implemented | not implemented |

use `.opaint` to preserve layers, visibility, opacity, and blending. flattened exports do not mark a layered project saved. `.opaint` is a separate versioned format; renaming it to `.pdn` does not make it compatible. image dimensions are limited to 8192 on either axis and 256 mib of layer pixels per document. history has its own 256 mib budget.

png is the preferred flat export when transparency matters. metadata and icc profiles are not preserved. the current format support does not match paint.net's complete codec inventory.

## controls

| action | key or gesture |
| --- | --- |
| new / open / save / save as | ctrl+n / ctrl+o / ctrl+s / ctrl+shift+s |
| undo / redo | ctrl+z / ctrl+y or ctrl+shift+z |
| copy / cut / paste | ctrl+c / ctrl+x / ctrl+v |
| paste into new layer / image | ctrl+shift+v / ctrl+alt+v |
| select all / deselect / invert selection | ctrl+a / ctrl+d / ctrl+i |
| brush / pencil / eraser | b / p / e |
| bucket / gradient / picker | f / g / k |
| clone / recolor / text | l / r / t |
| cycle selections / move tools / shapes | s / m / o |
| secondary color | right mouse button |
| swap colors | x |
| set clone source | ctrl+click |
| global fill or wand | shift+click |
| constrained selection or shape | shift+drag |
| centered shape | alt+drag |
| scale selected pixels or selection | drag a corner handle with a move tool |
| rotate selected pixels or selection | right drag with a move tool; shift snaps to 15° |
| multiline text | shift+enter in the text dialog |
| pan | middle drag, space+left drag, or h |
| zoom | mouse wheel, or z and click |
| fit / actual size | ctrl+f / ctrl+0 |
| layer properties | f4 |
| tools / history / layers / colors | f5 / f6 / f7 / f8 |

command also acts as ctrl on macos. select a tool before dragging on the image. text is inserted at the point clicked with the text tool. escape cancels an unfinished canvas drag. these are implemented controls; exact reference shortcut parity is tracked separately.

## checks and packages

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked --release
python3 scripts/package.py linux --format deb
```

[release instructions](docs/releases.md) cover deb, rpm, AppImage, windows installer/portable zip, both macos disk images, optional signing, and the tag-triggered github workflow. workflows publish only after all required jobs pass. no release has been published from this checkout.

## remaining parity work

the original windows application could not be run in the development session. static REA findings establish the feature inventory, not visual or pixel-equivalent behavior. [reference evidence](docs/reference.md), the [parity checklist](docs/parity.md), and [architecture notes](docs/architecture.md) record that boundary.

remaining areas include `.pdn` interoperability, paint.net plugins, the complete codec/effect/shape catalogs, full font/style support, pressure input, editable curves and gradient handles, color management, and platform accessibility. current interface placement and algorithms are implementations of the observed requirements, with no claim of pixel-perfect equivalence.

license: [MIT](LICENSE). dependency and font notices: [THIRD_PARTY.md](THIRD_PARTY.md).
