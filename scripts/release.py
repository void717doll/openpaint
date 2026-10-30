#!/usr/bin/env python3
"""validate release versions and package sets before publication."""

import argparse
import hashlib
from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parent.parent


def version():
    manifest = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    package = re.search(r"(?ms)^\[package\]\s*(.*?)(?=^\[|\Z)", manifest)
    match = re.search(r'^version\s*=\s*"(\d+\.\d+\.\d+)"\s*$', package[1], re.M) if package else None
    if not match:
        raise ValueError("Cargo.toml must contain a stable major.minor.patch package version")
    return match[1]


def checksum_packages(directory, require_complete=False):
    release_version = version()
    prefix = f"openpaint-{release_version}"
    expected = [
        f"{prefix}-linux-x86_64.deb",
        f"{prefix}-linux-x86_64.rpm",
        f"{prefix}-linux-x86_64.AppImage",
        f"{prefix}-windows-x86_64-*-setup.exe",
        f"{prefix}-windows-x86_64-*-portable.zip",
        f"{prefix}-macos-arm64-*.dmg",
        f"{prefix}-macos-x86_64-*.dmg",
    ]
    packages = []
    for pattern in expected:
        matches = sorted(directory.glob(pattern))
        if len(matches) > 1 or (require_complete and len(matches) != 1):
            raise ValueError(f"expected exactly one package matching {pattern}, found {len(matches)}")
        packages.extend(matches)
    if not packages:
        raise ValueError("no release packages found")
    unknown = [p.name for p in directory.iterdir() if p.is_file() and p.name != "SHA256SUMS" and p not in packages]
    if unknown:
        raise ValueError(f"unexpected release files: {', '.join(unknown)}")
    lines = []
    for package in sorted(packages):
        if package.stat().st_size == 0:
            raise ValueError(f"empty package: {package.name}")
        digest = hashlib.sha256()
        with package.open("rb") as stream:
            for block in iter(lambda: stream.read(1024 * 1024), b""):
                digest.update(block)
        lines.append(f"{digest.hexdigest()}  {package.name}\n")
    (directory / "SHA256SUMS").write_text("".join(lines), encoding="utf-8")
    print(f"checksummed {len(packages)} packages")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["version", "verify-tag", "checksums"])
    parser.add_argument("value", nargs="?")
    parser.add_argument("--complete", action="store_true")
    args = parser.parse_args()
    if args.command == "version":
        print(version())
    elif args.command == "verify-tag":
        expected = "v" + version()
        if args.value != expected:
            raise ValueError(f"release tag must match Cargo.toml: expected {expected}")
        print(expected)
    else:
        checksum_packages(Path(args.value or ROOT / "dist"), args.complete)


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError) as error:
        sys.exit(str(error))
