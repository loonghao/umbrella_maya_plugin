# Maya standalone smoke

`scripts/maya_standalone_smoke.py` loads the packaged plugin inside a real Maya
standalone session. It runs under `mayapy`, never under a plain CPython
interpreter, because Maya's Python interpreter is the only one that can
initialize `maya.standalone`.

```bash
mayapy scripts/maya_standalone_smoke.py \
  --maya-version 2024 \
  --package-root dist/modules \
  --scene tests/virus/uifiguration.ma
```

## What it verifies

1. `maya.standalone.initialize()` succeeds.
2. `cmds.about(version=True)` is the expected Maya year. An update suffix is
   accepted (`2024`, `2024.2`), another year is a failure.
3. `cmds.loadPlugin()` loads `umbrella_maya.<mll|so|bundle>` from the newest
   `UmbrellaMayaPlugin-<version>-maya<year>-<platform>` package, and
   `pluginInfo(loaded=True)` confirms it.
4. The plugin's commands respond: `umbrellaInfo`, `umbrellaEnable`,
   `umbrellaScanScene`, `umbrellaDisable`. With `--scene`, the scene is opened
   with `executeScriptNodes=False` before the scan.

The session is isolated: `MAYA_MODULE_PATH` and `MAYA_PLUG_IN_PATH` point at the
packaged module, and `MAYA_APP_DIR` is redirected to a temporary profile that
is deleted on exit.

## What this gate does not prove

Read the `[ok]` lines as ABI and liveness evidence only.

- **`umbrellaScanScene` is not a detection test.** The script asserts that the
  command was dispatched and did not raise. `Threats found: 0` is therefore not
  evidence that detection works: the sample scenes under `tests/virus/` are
  malformed on purpose, so Maya opens them with warnings such as
  `Unterminated string` or `missing a 'requires' statement`, and the scan can end
  up running against a scene that never finished loading. Detection accuracy is a
  separate concern, covered by the Rust tests.
- **The loader path is not set at runtime on Linux and macOS.** Both loaders
  snapshot the environment when the process starts, so assigning
  `LD_LIBRARY_PATH` or `DYLD_LIBRARY_PATH` after interpreter start cannot affect
  the `dlopen()` that loads the plugin. The plugin finds its packaged Rust library
  through the `INSTALL_RPATH` set in `CMakeLists.txt` (`$ORIGIN` on Linux,
  `@loader_path` on macOS). Windows has no rpath equivalent and resolves imports
  through `PATH` at load time, so the script does prepend the plug-ins directory
  to `PATH` there.

## Continuous integration

`ci.yml` runs this script in the `tahv/mayapy:2024` container against the Linux
package produced by the `maya-plugin` lane, so every pull request proves that a
real Maya 2024 loads the plugin. That container is Linux-only, so it validates the
Linux artifact; Windows artifacts are covered by the `dumpbin` checks in
`scripts/test-maya-artifacts.ps1` and by pinning the Windows lanes to
`windows-2022` (see [Windows toolchain contract](windows-toolchain.md)).

`scripts/run-maya-standalone-smoke.ps1` remains the Windows desktop-host gate.

## macOS coverage

There is no GitHub-hosted macOS Maya runner, so no CI job loads the macOS bundle.
The macOS lane instead verifies the Mach-O export table, which is the part that
actually broke on Linux: it asserts `umbrella_maya.bundle` exports
`_initializePlugin` and `_uninitializePlugin` as global symbols and carries an
`@loader_path` rpath so it can find the packaged Rust library. A bundle that fails
those checks fails the lane before it is ever shipped.

**Known unverified surface: the macOS bundle has never been loaded by a real
Maya.** Build, packaging, and symbol exports are verified; the load itself is not.
Owner: **hallong**. Until a real load is recorded, treat macOS artifacts as
experimental.

To close that gap, run the smoke on a Mac with Maya 2024 installed:

```bash
# 1. Build and package the macOS module for your Maya version.
just package 2024

# 2. Load it in a real Maya standalone session and keep the output as evidence.
/Applications/Autodesk/maya2024/Maya.app/Contents/bin/mayapy \
  scripts/maya_standalone_smoke.py \
  --maya-version 2024 \
  --package-root dist/modules \
  --scene tests/virus/uifiguration.ma
```

A successful run prints `Maya 2024 standalone initialized`, `Plugin loaded from
...`, and `Plugin commands responded: ...`. Attach that transcript to the pull
request or issue that records the verification, and update this section with the
Maya version and host used.

## Host baseline

The `tahv/mayapy:2024` image reports Ubuntu 22.04.4 LTS with glibc 2.35, which is
inside the range Autodesk certifies for Maya 2024 on Linux (glibc 2.28-2.35 with
libstdc++ up to `GLIBCXX_3.4.29`). The Linux build lanes are therefore pinned to
`ubuntu-22.04`: the newest Ubuntu image ships glibc 2.39 with GCC 13, and the
plugin it builds requires `GLIBC_2.38` and `GLIBCXX_3.4.32`, so it cannot be
loaded by the container even though it compiles.

## What this gate caught

The smoke is the only check that resolves the plugin the way Maya does, so it
surfaces failure modes the packaging checks cannot:

- A plugin whose entry points are emitted with C++ decoration. Maya looks up
  `initializePlugin` and `uninitializePlugin` by their undecorated names, and the
  Maya headers declare them with C++ linkage because `PLUGIN_EXPORT` only sets
  visibility. On Linux and macOS the entry points must therefore be emitted
  undecorated through assembler labels; on Windows the undecorated export comes
  from `/export:`. A build can succeed, pass every packaging check, and still
  fail here with `initializePlugin function failed`.
- A plugin whose runtime requirements exceed the host baseline, which fails as
  `version 'GLIBCXX_3.4.32' not found` (or the `GLIBC_2.38` equivalent) at load
  time.
