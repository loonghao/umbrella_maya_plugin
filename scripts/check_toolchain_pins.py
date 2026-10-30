#!/usr/bin/env python3
"""Verify that the pinned Rust/toolchain versions agree across the repository.

This project builds a Windows ``cdylib`` and a PyO3 extension whose dependency
graph uses ``raw-dylib`` imports. Those imports require the MSVC host toolchain;
the ``x86_64-pc-windows-gnu`` host cannot create the import library (its
``dlltool.exe`` fails), which produces a cryptic ``CreateProcess`` error. Local
and CI must therefore run on one pinned toolchain, and any drift must fail
loudly instead of being hidden behind an ad-hoc ``RUSTUP_TOOLCHAIN`` override.

Checks performed:

* ``rust-toolchain.toml`` ``channel`` is a single, parseable version.
* Every ``dtolnay/rust-toolchain@X`` pin in ``.github/workflows`` equals it.
* ``Cargo.toml`` ``rust-version`` is a prefix of the channel.
* ``vx.toml`` pins ``just`` to a concrete version and does NOT declare a
  ``rust`` tool pin (vx's Rust provider has a channel-switching defect; Rust
  stays on rustup via ``rust-toolchain.toml``).
* All ``CBINDGEN_VERSION`` and ``python-version`` values across workflows are
  consistent.
* On Windows, the active ``rustc`` host is NOT the broken ``*-windows-gnu``
  host, and its release matches the pinned channel.

Run with ``--self-test`` to exercise the parsing logic against synthetic
fixtures (no toolchain or network required).
"""

from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent


class CheckError(Exception):
    """A pin-consistency or host defect that must fail preflight."""


class Fixture:
    """In-memory view of the config files, so checks are unit-testable."""

    def __init__(
        self,
        toolchain_toml: str,
        cargo_toml: str,
        workflows: dict[str, str],
        vx_toml: str | None,
    ) -> None:
        self.toolchain_toml = toolchain_toml
        self.cargo_toml = cargo_toml
        self.workflows = workflows
        self.vx_toml = vx_toml


def _find(text: str, pattern: str, what: str) -> str:
    match = re.search(pattern, text)
    if not match:
        raise CheckError(f"could not find {what}")
    return match.group(1)


def pinned_channel(fx: "Fixture") -> str:
    return _find(
        fx.toolchain_toml,
        r'channel\s*=\s*"([^"]+)"',
        "channel in rust-toolchain.toml",
    )


def check_ci_rust_pins(fx: "Fixture", channel: str) -> list[str]:
    pins: list[tuple[str, str]] = []
    for name, body in fx.workflows.items():
        for m in re.finditer(r"dtolnay/rust-toolchain@([^\s]+)", body):
            pins.append((m.group(1), name))
    if not pins:
        raise CheckError("no dtolnay/rust-toolchain@ pins found in workflows")
    for version, where in pins:
        if version != channel:
            raise CheckError(
                f"CI pin dtolnay/rust-toolchain@{version} in {where} "
                f"does not match rust-toolchain.toml channel {channel}"
            )
    return [f"CI Rust pins == {channel} ({len(pins)} pin(s))"]


def check_rust_version(fx: "Fixture", channel: str) -> list[str]:
    rust_version = _find(
        fx.cargo_toml,
        r'rust-version\s*=\s*"([^"]+)"',
        "rust-version in Cargo.toml",
    )
    if not channel.startswith(rust_version):
        raise CheckError(
            f"Cargo.toml rust-version {rust_version} is not a prefix of "
            f"rust-toolchain.toml channel {channel}"
        )
    return [f"Cargo.toml rust-version {rust_version} prefixes {channel}"]


def check_vx_pins(fx: "Fixture", channel: str) -> list[str]:
    if not fx.vx_toml:
        return ["vx.toml absent (no VX-managed tool pins to reconcile)"]
    # vx's Rust provider has a channel-switching defect: it can ignore the
    # pinned channel and switch the rustup default to `stable`. A `rust` pin
    # here would silently reintroduce that defect into `vx just` recipes, so it
    # is forbidden; Rust stays on rustup via rust-toolchain.toml.
    rust = re.search(r'^\s*rust\s*=\s*"([^"]+)"', fx.vx_toml, re.MULTILINE)
    if rust:
        raise CheckError(
            "vx.toml declares a [tools] rust pin; vx's Rust provider ignores "
            "the pinned channel and can switch the rustup default. Remove the "
            "rust pin and keep Rust on rustup via rust-toolchain.toml / "
            "dtolnay/rust-toolchain@" + channel + "."
        )
    # The tool VX does manage (just) must be pinned to a concrete version, not
    # a floating "latest"/"*", so local and CI resolve the same task runner.
    just = re.search(r'^\s*just\s*=\s*"([^"]+)"', fx.vx_toml, re.MULTILINE)
    if not just:
        raise CheckError("vx.toml must pin [tools] just to a concrete version")
    if not re.fullmatch(r"\d+\.\d+\.\d+", just.group(1)):
        raise CheckError(
            f'vx.toml [tools] just="{just.group(1)}" is not a concrete X.Y.Z version'
        )
    return [f"vx.toml pins just=={just.group(1)}; Rust stays on rustup (VX defect avoided)"]


def check_cbindgen_consistency(fx: "Fixture") -> list[str]:
    versions: dict[str, str] = {}
    for name, body in fx.workflows.items():
        for m in re.finditer(r'CBINDGEN_VERSION:\s*"?([^"\s]+)"?', body):
            versions.setdefault(m.group(1), name)
    if not versions:
        return ["no CBINDGEN_VERSION pin (nothing to reconcile)"]
    if len(versions) > 1:
        raise CheckError(f"inconsistent CBINDGEN_VERSION values: {versions}")
    (version, where) = next(iter(versions.items()))
    return [f"CBINDGEN_VERSION consistent == {version} ({where})"]


def check_ci_python_pins(fx: "Fixture") -> list[str]:
    versions: dict[str, str] = {}
    for name, body in fx.workflows.items():
        for m in re.finditer(r'python-version:\s*"?([^"\s]+)"?', body):
            versions.setdefault(m.group(1), name)
    if not versions:
        return ["no python-version pin (nothing to reconcile)"]
    if len(versions) > 1:
        raise CheckError(f"inconsistent python-version values: {versions}")
    (version, where) = next(iter(versions.items()))
    return [f"CI python-version consistent == {version} ({where})"]


def _active_rustc() -> tuple[str, str] | None:
    try:
        out = subprocess.run(
            ["rustc", "-vV"], capture_output=True, text=True, check=False
        ).stdout
    except OSError:
        return None
    host = re.search(r"^host:\s*(\S+)", out, re.MULTILINE)
    release = re.search(r"^release:\s*(\S+)", out, re.MULTILINE)
    if not host or not release:
        return None
    return host.group(1), release.group(1)


def _version_prefix_matches(release: str, channel: str) -> bool:
    """Compare the overlapping numeric components (channel may omit patch)."""
    rel = release.split(".")
    pin = channel.split(".")
    overlap = min(len(rel), len(pin))
    return overlap > 0 and rel[:overlap] == pin[:overlap]


def check_windows_host(channel: str, host: str, release: str) -> list[str]:
    """Fail loudly on the GNU/dlltool host; this is the VX/raw-dylib defect."""
    if host.endswith("-pc-windows-gnu"):
        raise CheckError(
            "active Rust host is 'x86_64-pc-windows-gnu'.\n"
            "  This project's cdylib/PyO3 artifacts use raw-dylib imports that\n"
            "  require the MSVC host toolchain. The GNU host's dlltool.exe cannot\n"
            "  create the import library (cryptic CreateProcess failure).\n"
            "  Fix (scoped to this checkout, reversible):\n"
            "    rustup override set " + channel + "-x86_64-pc-windows-msvc\n"
            "  or per-shell: set RUSTUP_TOOLCHAIN=" + channel
            + "-x86_64-pc-windows-msvc\n"
            "  CI is unaffected: dtolnay/rust-toolchain@" + channel
            + " defaults to the MSVC host on windows-2022 runners."
        )
    if not _version_prefix_matches(release, channel):
        raise CheckError(
            f"active rustc release {release} does not match pinned channel {channel}"
        )
    return [f"active rustc host {host} / release {release} OK for pinned {channel}"]


def run_checks(fx: "Fixture", *, is_windows: bool, rustc: tuple[str, str] | None) -> list[str]:
    channel = pinned_channel(fx)
    lines = [f"pinned Rust channel: {channel}"]
    lines += check_ci_rust_pins(fx, channel)
    lines += check_rust_version(fx, channel)
    lines += check_vx_pins(fx, channel)
    lines += check_cbindgen_consistency(fx)
    lines += check_ci_python_pins(fx)
    if is_windows:
        if rustc is None:
            raise CheckError("on Windows, rustc -vV must be available to verify the host")
        lines += check_windows_host(channel, rustc[0], rustc[1])
    else:
        lines.append("non-Windows host: MSVC/raw-dylib host check skipped")
    return lines


def load_fixture(root: Path) -> "Fixture":
    workflows = {}
    wf_dir = root / ".github" / "workflows"
    if wf_dir.is_dir():
        for p in sorted(wf_dir.glob("*.yml")) + sorted(wf_dir.glob("*.yaml")):
            workflows[p.name] = p.read_text(encoding="utf-8")
    vx_path = root / "vx.toml"
    return Fixture(
        toolchain_toml=(root / "rust-toolchain.toml").read_text(encoding="utf-8"),
        cargo_toml=(root / "Cargo.toml").read_text(encoding="utf-8"),
        workflows=workflows,
        vx_toml=vx_path.read_text(encoding="utf-8") if vx_path.is_file() else None,
    )


def _must_fail(fx: "Fixture", *, is_windows: bool, rustc: tuple[str, str] | None,
               label: str, contains: str | None = None) -> None:
    try:
        run_checks(fx, is_windows=is_windows, rustc=rustc)
    except CheckError as err:
        if contains and contains not in str(err):
            raise AssertionError(f"{label}: wrong error message: {err}")
    else:
        raise AssertionError(f"{label} was NOT detected")


def self_test() -> int:
    def wf(rust: str, cbindgen: str, python: str) -> dict[str, str]:
        return {
            "ci.yml": (
                f"uses: dtolnay/rust-toolchain@{rust}\n"
                f'  CBINDGEN_VERSION: "{cbindgen}"\n'
                f'          python-version: "{python}"\n'
                "uses: loonghao/vx@v0.8.36\n"
            ),
            "release.yml": (
                f"uses: dtolnay/rust-toolchain@{rust}\n"
                f'  CBINDGEN_VERSION: "{cbindgen}"\n'
                f'          python-version: "{python}"\n'
            ),
        }

    base = Fixture(
        toolchain_toml='[toolchain]\nchannel = "1.95.0"\ncomponents = ["clippy"]\n',
        cargo_toml='[package]\nrust-version = "1.95"\n',
        workflows=wf("1.95.0", "0.29.2", "3.11"),
        vx_toml='[tools]\njust = "1.50.0"\n',
    )

    def variant(**overrides: object) -> "Fixture":
        return Fixture(**{**base.__dict__, **overrides})

    # Happy path (non-Windows): all pins consistent.
    run_checks(base, is_windows=False, rustc=None)

    # CI Rust pin drift is caught.
    _must_fail(variant(workflows={
        "ci.yml": 'uses: dtolnay/rust-toolchain@1.94.0\n  CBINDGEN_VERSION: "0.29.2"\n          python-version: "3.11"\n',
        "release.yml": 'uses: dtolnay/rust-toolchain@1.95.0\n  CBINDGEN_VERSION: "0.29.2"\n          python-version: "3.11"\n',
    }), is_windows=False, rustc=None, label="CI Rust pin drift")

    # Cargo rust-version mismatch is caught.
    _must_fail(variant(cargo_toml='[package]\nrust-version = "1.90"\n'),
               is_windows=False, rustc=None, label="Cargo rust-version mismatch")

    # A vx.toml `rust` pin is forbidden (VX channel-switching defect).
    _must_fail(variant(vx_toml='[tools]\njust = "1.50.0"\nrust = "1.95.0"\n'),
               is_windows=False, rustc=None, label="vx.toml rust pin",
               contains="channel")

    # A floating `just` pin is rejected.
    _must_fail(variant(vx_toml='[tools]\njust = "latest"\n'),
               is_windows=False, rustc=None, label="floating just pin")

    # Windows GNU host is rejected loudly with actionable remediation.
    _must_fail(base, is_windows=True,
               rustc=("x86_64-pc-windows-gnu", "1.95.0"),
               label="Windows GNU host", contains="dlltool")

    # Windows MSVC host passes.
    run_checks(base, is_windows=True,
               rustc=("x86_64-pc-windows-msvc", "1.95.0"))

    # Windows MSVC host but wrong release is caught.
    _must_fail(base, is_windows=True,
               rustc=("x86_64-pc-windows-msvc", "1.94.0"),
               label="rustc release drift")

    # Inconsistent cbindgen pins are caught.
    _must_fail(variant(workflows={
        "ci.yml": 'uses: dtolnay/rust-toolchain@1.95.0\n  CBINDGEN_VERSION: "0.29.2"\n          python-version: "3.11"\n',
        "release.yml": 'uses: dtolnay/rust-toolchain@1.95.0\n  CBINDGEN_VERSION: "0.28.0"\n          python-version: "3.11"\n',
    }), is_windows=False, rustc=None, label="inconsistent cbindgen pins")

    # Inconsistent CI python-version pins are caught.
    _must_fail(variant(workflows={
        "ci.yml": 'uses: dtolnay/rust-toolchain@1.95.0\n  CBINDGEN_VERSION: "0.29.2"\n          python-version: "3.11"\n',
        "release.yml": 'uses: dtolnay/rust-toolchain@1.95.0\n  CBINDGEN_VERSION: "0.29.2"\n          python-version: "3.12"\n',
    }), is_windows=False, rustc=None, label="inconsistent python-version pins")

    print("self-test: all 10 toolchain-pin cases passed")
    return 0


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true",
                        help="test the pin checks against synthetic fixtures")
    parser.add_argument("--root", default=str(REPO_ROOT),
                        help="repository root (default: parent of scripts/)")
    args = parser.parse_args(argv)

    if args.self_test:
        return self_test()

    is_windows = sys.platform == "win32" or os.name == "nt"
    rustc = _active_rustc() if is_windows else None
    try:
        lines = run_checks(load_fixture(Path(args.root)),
                           is_windows=is_windows, rustc=rustc)
    except CheckError as err:
        print(f"toolchain pin check FAILED:\n{err}", file=sys.stderr)
        return 1
    except FileNotFoundError as err:
        print(f"toolchain pin check could not read a config file: {err}",
              file=sys.stderr)
        return 1

    for line in lines:
        print(f"  ok: {line}")
    print("toolchain pin check passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
