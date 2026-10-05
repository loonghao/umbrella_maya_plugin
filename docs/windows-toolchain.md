# Windows build contract

The build CLI and CMake both verify `MAYA_API_VERSION` in `MTypes.h` against the
requested Maya year. Updates within a year are accepted, while another year's
SDK or an unverifiable header fails before compilation. An explicit
`MAYA_ROOT_DIR` or `MAYA_LOCATION` must match the requested year.

The C++ plugin uses the DLL CRT, matching Rust's default Windows `cdylib` policy.
The build CLI rejects `+crt-static` in Rust flag environment variables. Compiler
and SDK compatibility constraints remain owned by the target Maya project.

## Optional msvc-kit and Ninja

Normal Visual Studio builds continue to use the existing generator. A compatible
msvc-kit CLI can select an existing portable toolchain without global environment
or registry changes. This opt-in path requires the new `query --host-arch` and
`doctor --compile` commands; no unreleased dependency is installed automatically.

```powershell
vx cargo maya-build --current-only --maya-version 2024 --msvc-kit C:/tools/msvc-kit.exe --msvc-dir C:/toolchains/msvc --msvc-version 14.44.35207 --sdk-version 10.0.26100.0 --host-arch x64
```

Replace these example versions with the exact installed versions approved for
the target Maya version. This validates the toolchain, chooses Ninja, and applies
one query environment to Cargo and both CMake subprocesses. Cargo's linker and
CMake's compilers are selected explicitly. The query is saved to
`build/toolchain.json` for the build record. CMake's nested Cargo target inherits
the same environment. CMake and Ninja must be available in the invoking process.

Without msvc-kit, `--cmake-generator Ninja` selects Ninja in an already configured
developer shell. Maya Windows builds target x64.

## Artifact and host checks

```powershell
scripts/test-maya-artifacts.ps1 -MayaVersion 2024 -Platform windows -DumpbinPath C:/toolchains/msvc/VC/Tools/MSVC/14.44.35207/bin/Hostx64/x64/dumpbin.exe
scripts/run-maya-standalone-smoke.ps1 -MayaVersion 2024 -CleanEnvironment
```

The optional dependency check verifies both packaged PE binaries, the plugin's
Rust DLL import and the absence of debug CRT imports, then saves the dependency
list. When a portable query record exists, artifact validation also saves the
selected `cl.exe` SHA256 to `build/compiler-provenance.json`; the selection
fingerprint alone describes metadata rather than compiler bytes.
It does not claim that every dependency is deployable to another machine.
The standalone test uses a temporary Maya user profile and removes inherited
Python/module/toolchain paths. It loads the packaged plugin and exercises its
commands in mayapy without opening the desktop application. Artifact checks,
standalone host acceptance, and interactive host acceptance are separate gates.
