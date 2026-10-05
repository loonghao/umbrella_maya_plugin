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
packaged module, the plug-ins directory is prepended to the platform loader path,
and `MAYA_APP_DIR` is redirected to a temporary profile that is deleted on exit.

## Continuous integration

`ci.yml` runs this script in the `tahv/mayapy:2024` container against the Linux
package produced by the `maya-plugin` lane, so every pull request proves that a
real Maya 2024 loads the plugin. That container is Linux-only, so it validates the
Linux artifact; Windows artifacts are covered by the `dumpbin` checks in
`scripts/test-maya-artifacts.ps1` and by pinning the Windows lanes to
`windows-2022` (see [Windows toolchain contract](windows-toolchain.md)).

`scripts/run-maya-standalone-smoke.ps1` remains the Windows desktop-host gate.

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
