#!/usr/bin/env python3
"""build native openpaint packages from an already compiled release binary."""

import argparse
import base64
import hashlib
import os
from pathlib import Path
import plistlib
import re
import secrets
import shutil
import struct
import subprocess
import sys
import urllib.request
import xml.etree.ElementTree as ET
import zipfile

from release import ROOT, version
from licenses import collect as collect_licenses


APP_ID = "org.openpaint.OpenPaint"
APPIMAGE_TOOL_URL = "https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-x86_64.AppImage"
APPIMAGE_TOOL_SHA256 = "ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0"


def run(*command, **kwargs):
    return subprocess.run([str(part) for part in command], check=True, **kwargs)


def copy(source, destination):
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, destination)


def write(path, contents):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(contents, encoding="utf-8")


def verify_binary(path, platform, arch):
    with path.open("rb") as stream:
        header = stream.read(64)
        if platform == "linux":
            valid = header[:6] == b"\x7fELF\x02\x01" and struct.unpack_from("<H", header, 18)[0] == 62
        elif platform == "windows":
            if header[:2] != b"MZ":
                raise ValueError("windows payload is not a PE executable")
            stream.seek(struct.unpack_from("<I", header, 60)[0])
            pe = stream.read(6)
            valid = pe == b"PE\0\0\x64\x86"
        else:
            cpu = 0x0100000C if arch == "arm64" else 0x01000007
            valid = header[:4] == b"\xcf\xfa\xed\xfe" and struct.unpack_from("<I", header, 4)[0] == cpu
    if not valid:
        raise ValueError(f"{path} is not a {platform} {arch} executable")


def copy_notices(destination, target):
    for filename in ("LICENSE", "THIRD_PARTY.md"):
        copy(ROOT / filename, destination / filename)
    copy(ROOT / "assets/DejaVuSans-LICENSE.txt", destination / "DejaVuSans-LICENSE.txt")
    copy(ROOT / "assets/ProggyClean-LICENSE.txt", destination / "ProggyClean-LICENSE.txt")
    collect_licenses(destination / "licenses", target)


def glibc_requirement(binary):
    symbols = run("readelf", "--version-info", binary, capture_output=True, text=True).stdout
    versions = re.findall(r"GLIBC_(\d+(?:\.\d+)+)", symbols)
    if not versions:
        raise ValueError("cannot determine the linux binary's glibc requirement")
    return max(versions, key=lambda value: tuple(int(part) for part in value.split(".")))


def linux_tree(stage, binary, release_version):
    copy(binary, stage / "usr/bin/openpaint")
    (stage / "usr/bin/openpaint").chmod(0o755)
    copy_notices(stage / "usr/share/doc/openpaint", "x86_64-unknown-linux-gnu")
    copy(ROOT / "assets/openpaint.svg", stage / "usr/share/icons/hicolor/scalable/apps/openpaint.svg")
    copy(ROOT / "assets/openpaint.png", stage / "usr/share/icons/hicolor/1024x1024/apps/openpaint.png")
    copy(ROOT / f"packaging/{APP_ID}.desktop", stage / f"usr/share/applications/{APP_ID}.desktop")
    copy(ROOT / "packaging/openpaint-mime.xml", stage / "usr/share/mime/packages/openpaint.xml")
    metadata = ET.parse(ROOT / f"packaging/{APP_ID}.metainfo.xml")
    component = metadata.getroot()
    repository = os.environ.get("GITHUB_REPOSITORY", "")
    if re.fullmatch(r"[\w.-]+/[\w.-]+", repository):
        ET.SubElement(component, "url", type="homepage").text = f"https://github.com/{repository}"
        ET.SubElement(component, "url", type="bugtracker").text = f"https://github.com/{repository}/issues"
    metadata_path = stage / f"usr/share/metainfo/{APP_ID}.metainfo.xml"
    metadata_path.parent.mkdir(parents=True)
    metadata.write(metadata_path, encoding="utf-8", xml_declaration=True)


def deb(stage, output, release_version):
    installed_size = sum(p.stat().st_size for p in stage.rglob("*") if p.is_file()) // 1024 + 1
    glibc = glibc_requirement(stage / "usr/bin/openpaint")
    write(stage / "DEBIAN/control", f"""Package: openpaint
Version: {release_version}
Section: graphics
Priority: optional
Architecture: amd64
Maintainer: openpaint contributors
Installed-Size: {installed_size}
Depends: libc6 (>= {glibc}), libgcc-s1, libx11-6, libxi6, libgl1, libegl1, libxkbcommon0
Recommends: zenity, xclip
Description: native raster image editor
 Edit images with layers, drawing tools, selections, and effects.
""")
    run("dpkg-deb", "--build", "--root-owner-group", stage, output)
    run("dpkg-deb", "--info", output)
    shutil.rmtree(stage / "DEBIAN")


def rpm(stage, output, release_version):
    glibc = glibc_requirement(stage / "usr/bin/openpaint")
    topdir = stage.parent / "rpmbuild"
    for directory in ("BUILD", "BUILDROOT", "RPMS", "SOURCES", "SPECS", "SRPMS"):
        (topdir / directory).mkdir(parents=True, exist_ok=True)
    spec = topdir / "SPECS/openpaint.spec"
    write(spec, f"""Name: openpaint
Version: {release_version}
Release: 1
Summary: native raster image editor
License: MIT
BuildArch: x86_64
Requires: glibc >= {glibc}, libgcc_s.so.1()(64bit), libX11.so.6()(64bit), libXi.so.6()(64bit), libGL.so.1()(64bit), libEGL.so.1()(64bit), libxkbcommon.so.0()(64bit)
Recommends: zenity, xclip
AutoReqProv: no
%global debug_package %{{nil}}
%global __os_install_post %{{nil}}

%description
Edit images with layers, drawing tools, selections, and effects.

%install
mkdir -p "%{{buildroot}}"
cp -a "{stage}/usr" "%{{buildroot}}/"

%files
/usr/bin/openpaint
/usr/share/applications/{APP_ID}.desktop
/usr/share/metainfo/{APP_ID}.metainfo.xml
/usr/share/icons/hicolor/scalable/apps/openpaint.svg
/usr/share/icons/hicolor/1024x1024/apps/openpaint.png
/usr/share/mime/packages/openpaint.xml
%doc /usr/share/doc/openpaint/THIRD_PARTY.md
%license /usr/share/doc/openpaint/LICENSE
%license /usr/share/doc/openpaint/DejaVuSans-LICENSE.txt
%license /usr/share/doc/openpaint/ProggyClean-LICENSE.txt
%license /usr/share/doc/openpaint/licenses
""")
    run("rpmbuild", "--define", f"_topdir {topdir}", "-bb", spec)
    packages = list((topdir / "RPMS/x86_64").glob("*.rpm"))
    if len(packages) != 1:
        raise ValueError("rpmbuild did not produce exactly one package")
    copy(packages[0], output)
    run("rpm", "-qip", output)


def bundle_x11_libraries(stage):
    ldconfig = shutil.which("ldconfig") or "/sbin/ldconfig"
    cache = run(ldconfig, "-p", capture_output=True, text=True).stdout
    available = {}
    for line in cache.splitlines():
        match = re.match(r"\s*(\S+)\s+\([^)]*x86-64[^)]*\)\s+=>\s+(\S+)", line)
        if match:
            available.setdefault(match[1], Path(match[2]))
    queue = [available[name] for name in ("libX11.so.6", "libXi.so.6", "libxkbcommon.so.0")]
    excluded = re.compile(r"^(lib(c|m|dl|pthread|rt|gcc_s|GL|EGL|OpenGL|GLX|GLdispatch)\b|ld-linux)")
    seen = set()
    while queue:
        library = queue.pop()
        if library.name in seen or excluded.match(library.name):
            continue
        seen.add(library.name)
        copy(library, stage / "usr/lib" / library.name)
        linked = run("ldd", library, capture_output=True, text=True).stdout
        for dependency in re.findall(r"=>\s+(/\S+)", linked):
            queue.append(Path(dependency))
        # bundled libraries retain their distribution's copyright and license notices.
        owner = None
        paths = (str(library), str(library.resolve()), str(library).removeprefix("/usr"), "/usr" + str(library))
        for path in dict.fromkeys(paths):
            result = subprocess.run(["dpkg-query", "-S", path], capture_output=True, text=True)
            if result.returncode == 0:
                owner = result.stdout
                break
        if owner is None:
            raise ValueError(f"cannot find license owner for bundled library {library}")
        package = owner.split(":", 1)[0]
        copyright_file = Path("/usr/share/doc") / package / "copyright"
        copy(copyright_file, stage / "usr/share/doc/openpaint/bundled" / f"{package}.copyright")


def appimage(stage, output, release_version):
    tool = ROOT / "target/package/tools/appimagetool-1.9.1-x86_64.AppImage"
    tool.parent.mkdir(parents=True, exist_ok=True)
    if not tool.exists():
        with urllib.request.urlopen(APPIMAGE_TOOL_URL, timeout=120) as response:
            temporary = tool.with_suffix(".download")
            with temporary.open("wb") as stream:
                shutil.copyfileobj(response, stream)
            temporary.replace(tool)
    if hashlib.sha256(tool.read_bytes()).hexdigest() != APPIMAGE_TOOL_SHA256:
        raise ValueError("appimagetool checksum mismatch; remove the cached download before retrying")
    tool.chmod(0o755)
    # reuse the pinned tool's runtime instead of downloading a mutable runtime release.
    offset = int(run(tool, "--appimage-offset", capture_output=True, text=True).stdout.strip())
    runtime = tool.parent / "runtime-x86_64"
    runtime.write_bytes(tool.read_bytes()[:offset])
    runtime_help = run(tool, "--appimage-help", capture_output=True, text=True)
    runtime_version = run(tool, "--appimage-version", capture_output=True, text=True)
    write(stage / "usr/share/doc/openpaint/AppImage-runtime.txt", runtime_version.stdout + runtime_version.stderr + "\n" + runtime_help.stdout + runtime_help.stderr)
    copy(ROOT / "packaging/AppRun", stage / "AppRun")
    (stage / "AppRun").chmod(0o755)
    copy(ROOT / f"packaging/{APP_ID}.desktop", stage / f"{APP_ID}.desktop")
    copy(ROOT / "assets/openpaint.png", stage / "openpaint.png")
    copy(ROOT / "assets/openpaint.png", stage / ".DirIcon")
    bundle_x11_libraries(stage)
    run("appstreamcli", "validate", "--no-net", "--override=url-homepage-missing=info", stage / f"usr/share/metainfo/{APP_ID}.metainfo.xml")
    environment = dict(os.environ, ARCH="x86_64", VERSION=release_version)
    run(tool, "--appimage-extract-and-run", "--no-appstream", "--runtime-file", runtime, stage, output, env=environment)
    output.chmod(0o755)
    run(output, "--appimage-extract-and-run", "--version")


def linux(args, stage, output, release_version):
    if sys.platform != "linux" or args.arch != "x86_64":
        raise ValueError("linux packages must be built on x86_64 linux")
    linux_tree(stage, args.binary, release_version)
    formats = ("deb", "rpm", "appimage") if args.format == "all" else (args.format,)
    functions = {"deb": deb, "rpm": rpm, "appimage": appimage}
    for package_format in formats:
        suffix = "AppImage" if package_format == "appimage" else package_format
        functions[package_format](stage, output / f"openpaint-{release_version}-linux-x86_64.{suffix}", release_version)


def windows_sign(files, stage):
    certificate = os.environ.get("WINDOWS_CERTIFICATE_BASE64", "")
    password = os.environ.get("WINDOWS_CERTIFICATE_PASSWORD", "")
    if not certificate:
        if password:
            raise ValueError("WINDOWS_CERTIFICATE_BASE64 is required with a signing password")
        return False
    signtool = shutil.which("signtool")
    if not signtool:
        sdk = Path(os.environ.get("ProgramFiles(x86)", "C:/Program Files (x86)")) / "Windows Kits/10/bin"
        matches = sorted(sdk.glob("*/x64/signtool.exe"))
        if not matches:
            raise ValueError("Windows SDK signtool.exe is required for signing")
        signtool = str(matches[-1])
    certificate_path = stage / "signing.pfx"
    try:
        certificate_path.write_bytes(base64.b64decode(certificate, validate=True))
        for file in files:
            run(signtool, "sign", "/fd", "SHA256", "/td", "SHA256", "/tr", "http://timestamp.digicert.com", "/f", certificate_path, "/p", password, file)
            run(signtool, "verify", "/pa", file)
    finally:
        certificate_path.unlink(missing_ok=True)
    return True


def windows(args, stage, output, release_version):
    if sys.platform != "win32" or args.arch != "x86_64":
        raise ValueError("windows packages must be built on x86_64 windows")
    copy(args.binary, stage / "openpaint.exe")
    copy(ROOT / "assets/openpaint.ico", stage / "openpaint.ico")
    copy_notices(stage, "x86_64-pc-windows-msvc")
    signed = windows_sign([stage / "openpaint.exe"], stage)
    signing_label = "signed" if signed else "unsigned"
    prefix = f"openpaint-{release_version}-windows-x86_64-{signing_label}"
    with zipfile.ZipFile(output / f"{prefix}-portable.zip", "w", compression=zipfile.ZIP_DEFLATED) as archive:
        for file in sorted(stage.rglob("*")):
            if file.is_file():
                archive.write(file, "openpaint/" + str(file.relative_to(stage)).replace("\\", "/"))
    makensis = shutil.which("makensis")
    if not makensis:
        candidate = Path(os.environ.get("ProgramFiles(x86)", "C:/Program Files (x86)")) / "NSIS/makensis.exe"
        if not candidate.exists():
            raise ValueError("install NSIS before building the windows installer")
        makensis = str(candidate)
    installer = output / f"{prefix}-setup.exe"
    run(makensis, f"/DVERSION={release_version}", f"/DPAYLOAD={stage}", f"/DOUTPUT={installer}", f"/DICON={ROOT / 'assets/openpaint.ico'}", ROOT / "packaging/windows.nsi")
    if signed:
        windows_sign([installer], stage)


def macos(args, stage, output, release_version):
    if sys.platform != "darwin":
        raise ValueError("macos packages must be built on macos")
    app = stage / "openpaint.app"
    contents = app / "Contents"
    copy(args.binary, contents / "MacOS/openpaint")
    (contents / "MacOS/openpaint").chmod(0o755)
    target = "aarch64-apple-darwin" if args.arch == "arm64" else "x86_64-apple-darwin"
    copy_notices(contents / "Resources", target)
    iconset = stage.parent / f"openpaint-{args.arch}.iconset"
    iconset.mkdir(exist_ok=True)
    for size in (16, 32, 128, 256, 512):
        for scale in (1, 2):
            name = f"icon_{size}x{size}" + ("@2x" if scale == 2 else "") + ".png"
            run("sips", "-z", size * scale, size * scale, ROOT / "assets/openpaint.png", "--out", iconset / name, stdout=subprocess.DEVNULL)
    run("iconutil", "-c", "icns", iconset, "-o", contents / "Resources/openpaint.icns")
    info = {
        "CFBundleIdentifier": APP_ID,
        "CFBundleName": "openpaint",
        "CFBundleDisplayName": "openpaint",
        "CFBundleExecutable": "openpaint",
        "CFBundleIconFile": "openpaint.icns",
        "CFBundlePackageType": "APPL",
        "CFBundleShortVersionString": release_version,
        "CFBundleVersion": release_version,
        "LSMinimumSystemVersion": "11.0",
        "NSHighResolutionCapable": True,
        "NSHumanReadableCopyright": "openpaint contributors",
    }
    with (contents / "Info.plist").open("wb") as stream:
        plistlib.dump(info, stream)
    run("plutil", "-lint", contents / "Info.plist")
    (stage / "Applications").symlink_to("/Applications", target_is_directory=True)
    certificate = os.environ.get("MACOS_CERTIFICATE_BASE64", "")
    identity = os.environ.get("MACOS_SIGNING_IDENTITY", "")
    certificate_password = os.environ.get("MACOS_CERTIFICATE_PASSWORD", "")
    apple_id = os.environ.get("APPLE_ID", "")
    notarize_password = os.environ.get("APPLE_APP_PASSWORD", "")
    team_id = os.environ.get("APPLE_TEAM_ID", "")
    if bool(certificate) != bool(identity):
        raise ValueError("macos signing requires both MACOS_CERTIFICATE_BASE64 and MACOS_SIGNING_IDENTITY")
    if certificate_password and not certificate:
        raise ValueError("MACOS_CERTIFICATE_BASE64 is required with a signing password")
    if any((apple_id, notarize_password, team_id)) and not all((certificate, apple_id, notarize_password, team_id)):
        raise ValueError("notarization requires a signing certificate, APPLE_ID, APPLE_APP_PASSWORD, and APPLE_TEAM_ID")
    label = "notarized" if apple_id else "signed" if certificate else "unsigned"
    dmg = output / f"openpaint-{release_version}-macos-{args.arch}-{label}.dmg"
    keychain = stage.parent / f"openpaint-{args.arch}.keychain-db"
    cert_path = stage.parent / f"openpaint-{args.arch}.p12"
    try:
        if certificate:
            keychain_password = secrets.token_urlsafe(32)
            cert_path.write_bytes(base64.b64decode(certificate, validate=True))
            run("security", "create-keychain", "-p", keychain_password, keychain)
            run("security", "set-keychain-settings", "-lut", "21600", keychain)
            run("security", "unlock-keychain", "-p", keychain_password, keychain)
            run("security", "import", cert_path, "-k", keychain, "-P", certificate_password, "-T", "/usr/bin/codesign")
            run("security", "set-key-partition-list", "-S", "apple-tool:,apple:", "-s", "-k", keychain_password, keychain)
            run("codesign", "--force", "--deep", "--options", "runtime", "--timestamp", "--keychain", keychain, "--sign", identity, app)
        else:
            run("codesign", "--force", "--deep", "--sign", "-", app)
        run("codesign", "--verify", "--deep", "--strict", app)
        run("hdiutil", "create", "-volname", "openpaint", "-srcfolder", stage, "-ov", "-format", "UDZO", dmg)
        run("hdiutil", "verify", dmg)
        if certificate:
            run("codesign", "--force", "--timestamp", "--keychain", keychain, "--sign", identity, dmg)
        if apple_id:
            run("xcrun", "notarytool", "submit", dmg, "--apple-id", apple_id, "--password", notarize_password, "--team-id", team_id, "--wait")
            run("xcrun", "stapler", "staple", dmg)
            run("xcrun", "stapler", "validate", dmg)
    finally:
        cert_path.unlink(missing_ok=True)
        if keychain.exists():
            subprocess.run(["security", "delete-keychain", str(keychain)], check=False)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("platform", choices=("linux", "windows", "macos"))
    parser.add_argument("--arch", choices=("x86_64", "arm64"), default="x86_64")
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--output", type=Path, default=ROOT / "dist")
    parser.add_argument("--format", choices=("all", "deb", "rpm", "appimage"), default="all", help="linux only")
    args = parser.parse_args()
    args.binary = (args.binary or ROOT / "target/release" / ("openpaint.exe" if args.platform == "windows" else "openpaint")).resolve()
    verify_binary(args.binary, args.platform, args.arch)
    release_version = version()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    stage = ROOT / f"target/package/{args.platform}-{args.arch}"
    if stage.exists():
        shutil.rmtree(stage)
    stage.mkdir(parents=True)
    {"linux": linux, "windows": windows, "macos": macos}[args.platform](args, stage, output, release_version)
    print(f"packages written to {output}")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        # subprocess errors can include signing passwords in their command arguments.
        if isinstance(error, subprocess.CalledProcessError):
            sys.exit(f"packaging command failed with exit status {error.returncode}")
        sys.exit(str(error))
