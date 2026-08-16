#!/usr/bin/env python3
"""Validate the Decky release archive layout before publishing it."""

import json
import sys
from pathlib import Path, PurePosixPath
from zipfile import BadZipFile, ZipFile

REQUIRED_FILES = {
    "LICENSE",
    "bin/io.github.PatrikTheDev.HyperionCapture.flatpak",
    "dist/index.js",
    "main.py",
    "package.json",
    "plugin.json",
    "py_modules/__init__.py",
    "py_modules/command_env.py",
    "py_modules/config.py",
}


def validate_archive(archive: Path) -> None:
    """Raise ``ValueError`` when an archive cannot be installed by Decky."""

    try:
        with ZipFile(archive) as package:
            files = {
                PurePosixPath(name)
                for name in package.namelist()
                if not name.endswith("/")
            }
            roots = {path.parts[0] for path in files if path.parts}
            if len(roots) != 1:
                raise ValueError("archive must contain exactly one top-level directory")

            root = roots.pop()
            if any(".." in path.parts or path.is_absolute() for path in files):
                raise ValueError("archive contains an unsafe path")

            relative_files = {
                str(PurePosixPath(*path.parts[1:])) for path in files
            }
            missing = sorted(REQUIRED_FILES - relative_files)
            if missing:
                raise ValueError(f"archive is missing required files: {', '.join(missing)}")

            plugin = json.loads(package.read(f"{root}/plugin.json"))
            package_json = json.loads(package.read(f"{root}/package.json"))
            if not plugin.get("name"):
                raise ValueError("plugin.json must contain a non-empty name")
            if not package_json.get("version"):
                raise ValueError("package.json must contain a non-empty version")
    except BadZipFile as error:
        raise ValueError("archive is not a valid ZIP file") from error


def main() -> int:
    """Validate the archive supplied on the command line."""

    if len(sys.argv) != 2:
        print(f"usage: {Path(sys.argv[0]).name} ARCHIVE", file=sys.stderr)
        return 2

    try:
        validate_archive(Path(sys.argv[1]))
    except (OSError, ValueError) as error:
        print(f"invalid Decky package: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
