"""Load the packaged Maya plugin inside a real Maya standalone session.

Run this with ``mayapy`` (never a plain CPython interpreter):

    mayapy scripts/maya_standalone_smoke.py --maya-version 2024 \\
        --package-root dist/modules --scene tests/virus/uifiguration.ma

It is the cross-platform entry point used by CI; ``run-maya-standalone-smoke.ps1``
remains the Windows desktop-host gate.

This is an ABI and liveness gate, not a detection gate: it proves a real Maya can
load the plugin and dispatch its commands. It does not prove the scanner finds
anything, and the ``[ok]`` lines must not be read as detection coverage.
"""

from __future__ import annotations

import argparse
import atexit
import os
import pathlib
import re
import shutil
import sys
import tempfile

PLUGIN_EXTENSIONS = {"windows": ".mll", "linux": ".so", "macos": ".bundle"}
PLUGIN_COMMANDS = ("umbrellaInfo", "umbrellaEnable", "umbrellaScanScene", "umbrellaDisable")
SCRIPT_ROOT = pathlib.Path(__file__).resolve().parent.parent


def detect_platform() -> str:
    if sys.platform.startswith("win"):
        return "windows"
    if sys.platform == "darwin":
        return "macos"
    return "linux"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--maya-version", default="2024", help="Expected Maya year, e.g. 2024")
    parser.add_argument(
        "--package-root",
        default="dist/modules",
        help="Directory holding UmbrellaMayaPlugin-<version>-maya<year>-<platform>",
    )
    parser.add_argument(
        "--platform",
        default=detect_platform(),
        choices=sorted(PLUGIN_EXTENSIONS),
        help="Artifact platform to load (defaults to the running platform)",
    )
    parser.add_argument(
        "--scene",
        default=None,
        help="Optional scene opened before umbrellaScanScene; script nodes stay disabled",
    )
    return parser.parse_args()


def resolve_package(package_root: pathlib.Path, maya_version: str, platform_name: str) -> pathlib.Path:
    pattern = f"UmbrellaMayaPlugin-*-maya{maya_version}-{platform_name}"
    candidates = [p for p in package_root.glob(pattern) if p.is_dir()]
    if not candidates:
        # Artifact archives may add a directory level when restored in CI.
        candidates = [p for p in package_root.rglob(pattern) if p.is_dir()]
    if not candidates:
        raise SystemExit(f"No Maya module package matching {pattern!r} under {package_root}")
    return max(candidates, key=version_sort_key)


def version_sort_key(path: pathlib.Path) -> tuple[tuple[int, ...], float]:
    stem = path.name.split("-", 1)[-1].split("-")[0]
    fields = tuple(int(part) if part.isdigit() else 0 for part in stem.split("."))
    return (fields, path.stat().st_mtime)


def resolve_plugin(package: pathlib.Path, platform_name: str) -> pathlib.Path:
    plug_ins = package / "UmbrellaMayaPlugin" / "plug-ins"
    plugin = plug_ins / f"umbrella_maya{PLUGIN_EXTENSIONS[platform_name]}"
    if not plugin.is_file():
        raise SystemExit(f"Plugin binary missing: {plugin}")
    return plugin


def prepend_path(name: str, value: pathlib.Path) -> None:
    existing = os.environ.get(name, "")
    entries = [part for part in existing.split(os.pathsep) if part]
    os.environ[name] = os.pathsep.join([str(value), *entries])


def configure_environment(package: pathlib.Path, plugin: pathlib.Path, platform_name: str) -> pathlib.Path:
    """Point Maya at the packaged module and isolate the session from user prefs."""
    app_dir = pathlib.Path(tempfile.mkdtemp(prefix="umbrella-maya-profile-"))
    atexit.register(shutil.rmtree, app_dir, ignore_errors=True)

    plug_ins = plugin.parent
    os.environ["MAYA_MODULE_PATH"] = str(package)
    os.environ["MAYA_PLUG_IN_PATH"] = str(plug_ins)
    os.environ["MAYA_APP_DIR"] = str(app_dir)
    os.environ["PYTHONNOUSERSITE"] = "1"
    if platform_name == "windows":
        # Windows resolves imports through PATH at load time and has no rpath
        # equivalent, so the plug-ins directory has to be on PATH for the plugin to
        # find its packaged Rust library.
        prepend_path("PATH", plug_ins)
    # On Linux and macOS the plugin resolves that same library through its
    # INSTALL_RPATH ($ORIGIN / @loader_path). Setting LD_LIBRARY_PATH or
    # DYLD_LIBRARY_PATH here would be decorative: both loaders snapshot the
    # environment when the process starts, so a change made after interpreter start
    # cannot affect the dlopen() that loads the plugin.
    return app_dir


def assert_maya_version(expected: str) -> str:
    from maya import cmds

    version = cmds.about(version=True)
    if not re.fullmatch(rf"{re.escape(expected)}(\.\d+)*", version):
        raise SystemExit(f"Maya {expected} required, mayapy reports {version!r}")
    print(f"[ok] Maya {version} standalone initialized")
    return version


def load_plugin(plugin: pathlib.Path) -> None:
    from maya import cmds

    cmds.loadPlugin(str(plugin))
    if not cmds.pluginInfo(str(plugin), query=True, loaded=True):
        raise SystemExit(f"Plugin did not load: {plugin}")
    print(f"[ok] Plugin loaded from {plugin}")


def exercise_plugin(scene: pathlib.Path | None) -> None:
    from maya import cmds

    # Each call only proves the command was dispatched and returned without raising.
    # In particular umbrellaScanScene reporting "Threats found: 0" is not evidence that
    # detection works: the bundled sample scenes are malformed on purpose and Maya opens
    # them with warnings ("Unterminated string", "missing a 'requires' statement"), so
    # the scan often runs against a scene that never finished loading.
    cmds.umbrellaInfo()
    cmds.umbrellaEnable()
    if scene is not None:
        cmds.file(str(scene), open=True, force=True, ignoreVersion=True, executeScriptNodes=False, prompt=False)
        print(f"[ok] Opened {scene} with script nodes disabled")
    cmds.umbrellaScanScene()
    cmds.umbrellaDisable()
    print(f"[ok] Plugin commands responded: {', '.join(PLUGIN_COMMANDS)}")


def main() -> int:
    args = parse_args()

    package_root = pathlib.Path(args.package_root)
    if not package_root.is_absolute():
        package_root = SCRIPT_ROOT / package_root
    if not package_root.is_dir():
        raise SystemExit(f"Package root does not exist: {package_root}")

    scene = None
    if args.scene:
        scene = pathlib.Path(args.scene)
        if not scene.is_absolute():
            scene = SCRIPT_ROOT / scene
        if not scene.is_file():
            raise SystemExit(f"Scene file does not exist: {scene}")

    package = resolve_package(package_root, args.maya_version, args.platform)
    plugin = resolve_plugin(package, args.platform)
    configure_environment(package, plugin, args.platform)

    import maya.standalone

    maya.standalone.initialize(name="umbrella_maya_smoke")
    try:
        assert_maya_version(args.maya_version)
        load_plugin(plugin)
        exercise_plugin(scene)
    finally:
        maya.standalone.uninitialize()

    print("[ok] Maya standalone smoke completed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
