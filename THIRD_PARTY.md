# third-party notices

openpaint's original code is MIT licensed. the supplied paint.net installer is a reference artifact, not part of this application or its distribution. openpaint is an independent project.

## rust dependencies

the exact dependency versions and checksums are recorded in Cargo.lock. direct dependencies:

| package | license | purpose |
| --- | --- | --- |
| macroquad | MIT or Apache-2.0 | native window, input, graphics, and interface rendering |
| image | MIT or Apache-2.0 | image codecs and raster resampling |
| serde_json | MIT or Apache-2.0 | project metadata and settings |
| flate2 | MIT or Apache-2.0 | project compression |
| fontdue | MIT or Apache-2.0 or Zlib | raster text |

macroquad embeds the ProggyClean font under MIT; its notice is retained by the packaging scripts. openpaint uses the bundled DejaVu Sans font for interface and image text; its copyright and license are in assets/DejaVuSans-LICENSE.txt. release packages include these notices and dependency license texts collected from the exact Cargo.lock packages. quad-rand declares MIT in its manifest but ships no standalone license text; the collector preserves that manifest and records the limitation.

## packaging tools

appimagetool and the AppImage runtime are used by the linux packaging workflow. the workflow pins their release and checksums; AppImage runtime license information is retained with packaging inputs. NSIS creates the windows installer. neither tool is used to implement image editing behavior.
