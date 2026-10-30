# architecture

openpaint separates the raster model from the window and interface. engine tests do not need a display server. the application uses cached rust dependencies available in the development environment; no paint.net code or assets are embedded.

| module | responsibility |
| --- | --- |
| `src/document.rs` | rgba layers, compositing, snapshots, dirty state, structural transforms, bounds |
| `src/tools.rs` | selection masks, drawing, floods, geometry, clone/recolor, text, clipboard raster operations |
| `src/effects.rs` | pure rgba adjustment/effect functions |
| `src/io.rs` | image codecs, layered projects, checked loading, atomic saves, settings location |
| `src/app.rs` | documents, commands, canvas input, textures, asynchronous jobs, close/save flow |
| `src/ui.rs` | compact native chrome, menus, panels, fields, dialogs, input capture |
| `src/dialogs.rs` | platform file dialogs with an in-app fallback |
| `src/clipboard.rs` | platform image clipboard helpers |
| `src/main.rs` | command-line handling and native window startup |

## pixels and history

layers contain full-canvas, straight-alpha rgba8 buffers in bottom-to-top order. selection masks have one coverage byte per canvas pixel. source-over blending converts colors appropriately for alpha; resize and blur filters use premultiplied colors to avoid leaking invisible rgb values.

`begin_edit` and `finish_edit` group a gesture into one undo step. empty edits do not create history. snapshots include layers, dimensions, the active layer, and the selection. selection-only changes do not dirty saved image content. history revisions identify the savepoint even after undo/redo or branching edits. memory bounds are enforced on image dimensions, layer buffers, and retained history; large operations can temporarily allocate additional working buffers.

effects run on worker threads over an immutable source snapshot. repeated previews always start from that snapshot. cancelling restores the original without a history entry; accepting creates one edit. dropping a cancelled job's receiver prevents late results from changing the image. a cancelled calculation can finish in the background.

## project format

`.opaint` version 1 consists of:

1. eight bytes: `OPNT`, byte `01`, carriage return, newline, zero.
2. a little-endian u32 json-header length.
3. a utf-8 json object with `width`, `height`, `active_layer`, and `layers`.
4. one gzip stream containing every layer's rgba8 pixels in layer order.

each layer entry contains `name`, `visible`, `opacity`, and `blend`. pixel buffer sizes derive from the dimensions; untrusted project lengths, counts, metadata, and decompressed sizes are bounded. selections and undo history are session state and are not written into projects.

files are written to a newly created temporary sibling, flushed and synced, then renamed over the destination. failure removes the temporary file and leaves the original in place. saving requires an extension with an implemented encoder. `.pdn` is rejected explicitly: REA found paint.net-specific BinaryFormatter/DeferredFormatter serialization, and no interoperability was established.

## settings and release boundaries

recent files are stored in `openpaint/settings.json` under the platform configuration directory: `XDG_CONFIG_HOME` or `~/.config`, windows `APPDATA`, or macos `~/Library/Application Support`. no document contents are stored there.

the original installer and extracted/reference evidence remain ignored under the workspace and `analysis/`. packaging selects the executable, original icons, platform metadata, and actual license notices explicitly; it does not archive the workspace. all build and package outputs go under ignored `target/` and `dist/` directories.

github workflows define cross-platform checks and release builds. local compilation is not evidence of successful hosted jobs, native execution on other operating systems, signing, or installation.
