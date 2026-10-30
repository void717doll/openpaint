#!/usr/bin/env python3
"""collect dependency notices from the exact resolved cargo source packages."""

import argparse
import json
from pathlib import Path
import shutil
import subprocess

from release import ROOT


def collect(destination, target):
    result = subprocess.run(
        ["cargo", "metadata", "--locked", "--offline", "--format-version", "1", "--filter-platform", target],
        cwd=ROOT, check=True, capture_output=True, text=True,
    )
    metadata = json.loads(result.stdout)
    destination.mkdir(parents=True, exist_ok=True)
    packages = []
    missing = []
    for package in sorted(metadata["packages"], key=lambda item: (item["name"], item["version"])):
        if package["name"] == "openpaint":
            continue
        source = Path(package["manifest_path"]).parent
        identity = f"{package['name']}-{package['version']}"
        directory = destination / identity
        directory.mkdir(exist_ok=True)
        paths = [path for path in source.iterdir() if path.name.upper().startswith(("LICENSE", "LICENCE", "COPYING", "NOTICE", "UNLICENSE"))]
        if package.get("license_file"):
            paths.append(source / package["license_file"])
        for path in dict.fromkeys(paths):
            if path.is_dir():
                shutil.copytree(path, directory / path.name, dirs_exist_ok=True)
            else:
                shutil.copy2(path, directory / path.name)
        if not paths:
            # preserve the upstream declaration; do not invent an absent copyright notice.
            shutil.copy2(source / "Cargo.toml", directory / "Cargo.toml")
            for readme in source.glob("README*"):
                if readme.is_file():
                    shutil.copy2(readme, directory / readme.name)
            note = f"{identity} declares {package.get('license') or 'no SPDX license'} in Cargo.toml, but its published source package contains no standalone license text.\n"
            (directory / "NOTICE.txt").write_text(note, encoding="utf-8")
            missing.append(identity)
            print(f"notice: {note.strip()}")
        packages.append({
            "name": package["name"], "version": package["version"],
            "license": package.get("license"), "authors": package["authors"],
            "repository": package.get("repository"), "source": package.get("source"),
            "license_files": [str(path.relative_to(directory)) for path in sorted(directory.rglob("*")) if path.is_file()],
        })
    sysroot = subprocess.run(["rustc", "--print", "sysroot"], check=True, capture_output=True, text=True).stdout.strip()
    rust_docs = Path(sysroot) / "share/doc/rust"
    rust_destination = destination / "rust-standard-library"
    rust_destination.mkdir(exist_ok=True)
    shutil.copy2(rust_docs / "COPYRIGHT-library.html", rust_destination / "COPYRIGHT-library.html")
    shutil.copytree(rust_docs / "licenses", rust_destination / "licenses", dirs_exist_ok=True)
    rust_version = subprocess.run(["rustc", "--version"], check=True, capture_output=True, text=True).stdout.strip()
    index = {"target": target, "rustc": rust_version, "packages": packages, "missing_standalone_license_text": missing}
    (destination / "INDEX.json").write_text(json.dumps(index, indent=2) + "\n", encoding="utf-8")
    print(f"collected notices for {len(packages)} cargo packages and the rust standard library")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("destination", type=Path)
    parser.add_argument("--target", required=True)
    args = parser.parse_args()
    collect(args.destination, args.target)
