#!/usr/bin/env python3
"""Package the PyO3 extension as an importable Python module."""

from __future__ import annotations

import os
import platform
import shutil
from pathlib import Path


def default_target() -> str:
    system = platform.system().lower()
    machine = platform.machine().lower()
    if system == "windows":
        return "x86_64-pc-windows-msvc"
    if system == "darwin":
        return "aarch64-apple-darwin" if machine in {"arm64", "aarch64"} else "x86_64-apple-darwin"
    return "x86_64-unknown-linux-gnu"


def source_name(target: str) -> str:
    if "windows" in target:
        return "umbrella_maya_plugin.dll"
    if "apple" in target:
        return "libumbrella_maya_plugin.dylib"
    return "libumbrella_maya_plugin.so"


def extension_suffix(target: str) -> str:
    if "windows" in target:
        return ".pyd"
    return ".so"


def main() -> None:
    root = Path(__file__).resolve().parents[1]
    explicit_target = os.environ.get("CARGO_BUILD_TARGET")
    target = explicit_target or default_target()
    release_dirs = []
    if explicit_target:
        release_dirs.append(root / "target" / target / "release")
        release_dirs.append(root / "target" / "release")
    else:
        release_dirs.append(root / "target" / "release")
        release_dirs.append(root / "target" / target / "release")

    source = release_dirs[0] / source_name(target)
    for release_dir in release_dirs:
        candidate = release_dir / source_name(target)
        if candidate.exists():
            source = candidate
            break
    if not source.exists():
        raise SystemExit(f"Python extension build output not found: {source}")

    output_dir = root / "dist" / "python" / target
    output_dir.mkdir(parents=True, exist_ok=True)
    dest = output_dir / f"umbrella_maya{extension_suffix(target)}"
    shutil.copy2(source, dest)

    flat_dir = root / "dist" / "python"
    flat_dir.mkdir(parents=True, exist_ok=True)
    shutil.copy2(dest, flat_dir / dest.name)
    print(dest)


if __name__ == "__main__":
    main()
