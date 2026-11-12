# paint.net 5.1.12 binary audit

this report records static analysis of the supplied installer and the implementation changes driven by it. it does not claim that openpaint is a drop-in replacement. the target application was not launched successfully, and no matched screenshot or pixel comparison exists.

## target identity and method

the installer sha-256 is `9c34b285eabcc4658335ad91f9c76ccf8f96d939b9640fac08e4942e394e1e8d`. the extracted `x64/paintdotnet.dll` sha-256 is `313fad2fb569baf7248a69d1cb981fd713187270abaee64849a723db90013aba`; its module id is `76c45a70-d224-4997-9eec-e7763aec7976`. this managed x64 PE targets .NET 9 and includes CIL plus ReadyToRun. evidence is scoped to those exact bytes.

REA 6.7.0's portable managed inspection recovered metadata, targeted member lists, and selected CIL without a running process. broad main-assembly member inspection exceeded REA's 64 MiB response budget; bounded type/method scans were used instead. the complete retained REA records are under the ignored `analysis/reference/` tree. the `ev_...` identifiers below map to those records.

## recovered behavior and implementation

| binary observation | openpaint implementation | remaining parity work |
| --- | --- | --- |
| seven gradient geometries: linear, reflected linear, diamond, radial, conical, clockwise spiral, counter-clockwise spiral; three repeat modes: none, reflected, wrapped (`ev_7d9efec94b80ba9a28a723ba99769557af1960d5d11ee11170ff5b3b281fc092`, `ev_a98bad603cd3c3a9eeff58426ff83b559a521f02711f4489b0ff6cebc6a7829f`, `ev_90040291d07e9982438696a828f14ca3eca7bd3cca1dbf1b6e744d23aba6824b`) | all seven modes and repeat selection are available in `src/tools.rs` and the tool options | GPU raster output, alpha edge cases, and handle interaction need direct comparison |
| selection sizes include any, fixed ratio, and fixed size; combination modes include replace, union, subtract, intersect, xor | fixed sizing and ratio plus five combination modes are implemented | modifier timing and exact mask coverage need direct comparison |
| canvas resize uses nine anchors and fills every layer; default settings identify top-left and transparent, with persisted user settings able to change them (`ev_c4bd0b9d3449c0ef61e3ab6bb54d71510af0b2020afe9c458ec0d0e24e52f27e`, `ev_ca2f6a32450e8dccc570b3e043d7d1737cf52cefcdae77aa33888b258b441736`) | nine anchors and fill choice apply to all layers | integer rounding and persisted-setting scenario need comparison |
| import-file path expands canvas to the larger source dimensions, appends imported layers at the top, and pads original layers; inspected path does not show a resize prompt | supported flat images import as layers and expand the canvas in one edit | exact supported input list, metadata, alpha, and UI prompt behavior need runtime checks |
| bounded command scan maps copy merged to ctrl+shift+c, zoom to selection to ctrl+shift+b, repeat effect to ctrl+f, save all to ctrl+alt+s | these menu commands and shortcuts are wired | keyboard behavior under non-US layouts and platform modifiers need checks |
| exposure range is -200..200 with /100 normalization; highlights/shadows are -100..100 with radius 0..10 and default 1.25; temperature/tint are -100..100 (`ev_366ee6a04b6183da53717e0e73b2e58f6729a54d5448a50392e9a501baf843a9`) | exposure, highlights/shadows, and temperature/tint controls are present | highlights clarity/radius and exact Direct2D/color-space output are not matched |
| levels exposes input black/white, gamma, and output black/white; gamma defaults to 1 and automatic gamma clamps 0.1..10; curves use a 1024-entry lookup, natural spline interpolation, and RGB/luminosity transfer modes (`ev_0cddbe5c93b1527641bddec9687bff59a78031a3fdfc7631d679c75c59e6b0f7`, `ev_59821989d2a50c4d810f2ca40b8e38d3661576c5d9d944cbfd7b3f9d386eabd6`) | levels and curve adjustments are available; curve channels include RGB, red, green, blue, luminosity | per-channel levels, simultaneous independent RGB curves, interactive graph points, and GPU rounding remain |

## command and codec inventory

REA inspected 49 targeted menu/action types in `paintdotnet.dll` (`ev_451ea02ba9e7a8b2b906a1f9700d0767ba1140607d8c16c674b0869331208f13`). these results describe static menu construction and command metadata, not every reachable runtime state. relevant action bindings above were also recovered from bounded CIL.

the file-type assembly inventory (`ev_374f014a82ec459eae9eeb0217066165ff3ef118dbab1523dc13194ddce15bd5`) includes AVIF, BMP, DDS, GIF, HEIC, HEIF, JPEG, JPEG XL, JPEG XR, PDN, PNG, TGA, TIFF, and WebP, alongside internal/soft-blocked types. inventory presence does not establish read/write support for each mode or export options. openpaint currently reads/writes its documented subset; `.pdn` is not interoperable.

targeted dialog/plugin types (`ev_5cc6cf61d31bfd367a6988d3a112603c22b69a0c44edb778e7f8dc9ccf8abae7`) include canvas size and resize dialogs, shape categories, and default effect/file-type plugin loaders. openpaint has no managed plugin host. no plugin ABI compatibility is claimed.

## limitations and next comparisons

Wine 10.0 launches stopped before app startup because wineserver could not initialize under the session's filesystem restrictions; the display server was unavailable too. see [wine run notes](wine-reference.md). all statements here are static findings or implementation mapping, not observed UI execution. exact brush coverage, selection masks, blend rounding, gradient pixels, GPU effect output, menus/layout, and dialog flows still need controlled original-versus-openpaint comparisons in a working Windows desktop.

the installed original remains the behavioral authority. recovered type names and CIL guide independent clean-room Rust implementations; no proprietary code or assets were copied. broader evidence and reproduction commands are in [reference.md](reference.md), while current feature gaps are tracked in [parity.md](parity.md).
