# packages and releases

`Cargo.toml` supplies the version. a release tag must match it exactly, for example `v0.1.0`. packaging currently accepts stable `major.minor.patch` versions.

## outputs

| platform | runner | artifacts |
| --- | --- | --- |
| linux x86_64 | `ubuntu-22.04` | `.deb`, `.rpm`, `.AppImage` |
| windows x86_64 | `windows-2022` | nsis installer, portable `.zip` |
| macos apple silicon | `macos-15` | `.app` inside an arm64 `.dmg` |
| macos intel | `macos-15-intel` | `.app` inside an x86_64 `.dmg` |

each filename contains the version, platform, and architecture. windows and macos filenames also state `unsigned`, `signed`, or `notarized`. the release contains seven packages and `SHA256SUMS`.

the linux release is built on ubuntu 22.04. deb and rpm dependencies include the glibc version actually required by the executable, determined with `readelf`. the appimage bundles the x11 client libraries and their dependency/license files; glibc and graphics drivers remain system dependencies. a working x11 or xwayland display and opengl driver are required. native wayland support has not been validated. optional linux file dialogs and external clipboard integration use the installed desktop helpers.

windows builds use the static msvc runtime. macos bundles target macos 11 or newer and have separate binaries for each architecture. `scripts/package.py` checks the ELF, PE, or Mach-O architecture before packaging.

## installation

download a package and `SHA256SUMS` from the same release. for files present locally, verify their entries with `sha256sum` on linux, `shasum -a 256` on macos, or `Get-FileHash -Algorithm SHA256` in powershell.

```sh
# debian or ubuntu
sudo apt install ./openpaint-0.1.0-linux-x86_64.deb

# fedora and compatible systems
sudo dnf install ./openpaint-0.1.0-linux-x86_64.rpm

# appimage
chmod +x openpaint-0.1.0-linux-x86_64.AppImage
./openpaint-0.1.0-linux-x86_64.AppImage
```

the appimage also supports `--appimage-extract-and-run` when fuse mounting is unavailable.

on windows, run the installer or extract the portable zip and launch `openpaint.exe`. the installer installs for the current user, adds a start menu entry and an uninstall entry, and registers openpaint as an available handler for `.opaint` without replacing the default application.

on macos, open the disk image and drag `openpaint.app` to applications. unsigned macos packages use an ad-hoc signature and have no developer id or notarization ticket. opening them may require the operating system's explicit open confirmation. macos finder document-open integration is not advertised by the bundle; use the application's open command or launch its executable with paths.

## local packaging

build the release executable first. packaging uses existing binaries and does not rebuild or publish them.

```sh
cargo build --locked --release
python3 scripts/package.py linux --format deb
```

linux needs python 3.9+, `readelf`, `dpkg-deb`, `rpmbuild`, `rpm`, `ldconfig`, `ldd`, `dpkg-query`, and `appstreamcli` for all formats. `--format deb`, `--format rpm`, and `--format appimage` select a single format. appimage packaging on linux uses debian/ubuntu package metadata to collect bundled-library license notices.

```sh
python3 scripts/package.py linux
python scripts/package.py windows
python3 scripts/package.py macos --arch arm64
python3 scripts/package.py macos --arch x86_64
```

run each platform command on the matching operating system. windows requires nsis; macos requires the xcode command line tools. use `--binary PATH` for a cargo target-specific build and `--output PATH` to change the default `dist/` destination. local windows builds should set `RUSTFLAGS="-C target-feature=+crt-static"` before compiling, matching the workflow.

the appimage builder downloads [appimagetool 1.9.1](https://github.com/AppImage/appimagetool/releases/tag/1.9.1) and verifies its [upstream sha256 digest](https://github.com/AppImage/appimagetool/releases/expanded_assets/1.9.1). its bundled runtime is reused, avoiding the builder's default download of a mutable runtime release. an unavailable download or checksum mismatch fails packaging. `target/package/tools/` stores the verified tool for reuse.

desktop metadata is installed with deb and rpm. during github packaging, AppStream homepage and issue links are filled from `GITHUB_REPOSITORY`. local builds omit those links unless that variable is set; source metadata validation treats the missing homepage as informational.

each package includes `LICENSE`, `THIRD_PARTY.md`, both font notices, and a `licenses/` directory. `scripts/licenses.py` uses locked, offline cargo metadata for the selected target to copy the dependency packages' actual license and notice files. it also copies the rust standard library's copyright report and referenced license texts. `licenses/INDEX.json` records versions, license declarations, repositories, and missing standalone texts. quad-rand 0.2.3 declares MIT but ships no standalone license file; its published manifest, readme, and an explicit note are retained instead of fabricating an attribution.

## github actions

`.github/workflows/ci.yml` checks formatting, clippy, tests, release compilation, command-line startup, and packaging metadata on pushes and pull requests. native builds cover all four operating system/architecture pairs. these checks do not establish interactive gui parity.

`.github/workflows/release.yml` runs on pushed `v*` tags. it reruns the checks, verifies the tag against the cargo version, builds each native package, and uploads temporary workflow artifacts. publication waits for every check and package job. the final job rejects missing, duplicate, empty, or unexpected package files and creates sha256 checksums. it uploads assets to a draft release before making that release public. reruns can complete a draft, but refuse to replace an already published release.

only the publish job has `contents: write`. all other jobs have read-only repository access and checkouts do not retain credentials. no separate release token is needed; publishing uses the job's `GITHUB_TOKEN` through `gh`.

actions are pinned to the verified commits for [checkout v7.0.1](https://github.com/actions/checkout/commit/3d3c42e5aac5ba805825da76410c181273ba90b1), [upload-artifact v7.0.2](https://github.com/actions/upload-artifact/commit/cf430e030ddbb5b0abf93d22962f4752f3646cd9), and [download-artifact v8.0.2](https://github.com/actions/download-artifact/commit/9000827ccba6bdab643e8b6fd33ac0654aef8333). runner architectures follow the [github runner reference](https://docs.github.com/en/actions/how-tos/write-workflows/choose-where-workflows-run).

## optional signing

no signing secrets are required for unsigned releases. provide the following repository secrets to enable signing:

| secret | purpose |
| --- | --- |
| `WINDOWS_CERTIFICATE_BASE64` | base64-encoded code-signing pfx |
| `WINDOWS_CERTIFICATE_PASSWORD` | pfx password, if protected |
| `MACOS_CERTIFICATE_BASE64` | base64-encoded developer id p12 |
| `MACOS_CERTIFICATE_PASSWORD` | p12 password, if protected |
| `MACOS_SIGNING_IDENTITY` | exact developer id application identity |
| `APPLE_ID` | notarization account |
| `APPLE_APP_PASSWORD` | app-specific notarization password |
| `APPLE_TEAM_ID` | developer team used for notarization |

windows signing signs and verifies the application and installer with the windows sdk signtool and timestamps them. macos signing imports a temporary keychain, signs the app and disk image, and removes the keychain afterward. the three apple account secrets enable notarization of the disk image and stapling of its ticket. incomplete signing/notarization configuration fails instead of silently producing a differently labeled artifact. signing key material is removed from staging even when a command fails.

hosted builds, installation on windows/macos, optional signing, and notarization need verification on their actual hosts. creating these workflow files does not publish a release or demonstrate a successful hosted run.

## local validation record

the initial linux release executable and `.deb` were built locally. the deb was extracted without installing it, and the extracted executable returned the expected `--version` and `--help` output. `dist/SHA256SUMS` covers that actual package. its local build requires glibc 2.39 or newer; packages built by the ubuntu 22.04 workflow will derive their own minimum.

rpm and AppImage packages were not produced locally because their build tools/downloads are unavailable. windows and macos packages require their native hosts. all seven artifact paths are defined by the release workflow, but only the deb exists in the local `dist/` directory.
