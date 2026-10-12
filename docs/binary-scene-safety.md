# Binary scene safety and Issue #3

## What is confirmed

At v0.3.1 / main `cdc4d03`, the offline cleaner accepted `.mb`, removed
`regex::bytes` matches, then overwrote the source by default. This can change an
IFF payload length without updating its declared chunk length. The single-file
C ABI also mapped `Ok(CleanResult { status: Failed, .. })` to `files_failed = 0`.
`BeforeOpenCheck` treated scan errors (`threats_found = -1`) as clean and did not
rescan after cleaning.

The regression fixture is synthetic IFF-like bytes, not a valid production Maya
scene. A structural mismatch proves unsafe byte mutation; it is **not** a
reproduction of Maya failing to reopen a real production asset. Running the old
cleaner on the controlled fixture reduced 40 bytes to 26, while its header still
declared a 32-byte payload and the remaining payload was only 18 bytes.

## Safety contract in this change

- Offline `.mb` cleaning always returns `Failed` before creating a backup,
  rewriting, deleting, or creating a cleaned output. Binary magic/NUL content
  disguised with a text extension is also rejected. Read-only scanning remains.
- C ABI failures propagate. The protected open callback rejects failed/skipped
  scans, cleanup failure/deletion, and non-clean post-clean scans.
- Text cleanup scans the same bytes it edits and refuses residual signatures.
  Unique exclusive backups preserve every original snapshot. Writes are staged
  in the destination directory and persisted atomically. Out-of-place cleanup
  never deletes the original or overwrites an existing output.
- A per-source exclusive sidecar serializes cooperating cleaners. Errors release
  it. After a process crash, a stale `.umbrella-clean.lock` fails closed: inspect
  the scene/backups and verify no cleaner is running before removing the lock.
  This is not protection against arbitrary non-cooperating writers or unreliable
  network-filesystem atomicity. Use local disposable copies for acceptance tests.
- `--maya-version` by itself only scans. Native mutation requires `--clean`,
  `--environment-profile`, and `--output-dir`. No offline fallback is permitted.

## Native cleanup: explicit production profile, separate outputs

Use the same Maya build, Umbrella plugin/runtime, required production plugins,
and dependency files as the production scene. An arbitrary mayapy container or
Maya year alone is insufficient. Run from the configured production environment;
never use a workstation with valuable files to inspect an untrusted scene.
Disabling script nodes is not an OS sandbox and does not disable all executable
plugin or scene behavior.

Example invocation:

```sh
umbrella-maya --path /staging/asset.mb --clean --maya-version 2024 \
  --plugin /production/plug-ins/umbrella_maya.so \
  --environment-profile /production/umbrella-environment.json \
  --output-dir /staging/review
```

The JSON profile schema is:

```json
{
  "schema_version": 1,
  "maya_version": "EXACT cmds.about(version=True) VALUE",
  "api_version": 0,
  "platform": "EXACT sys.platform VALUE",
  "mayapy_sha256": "SHA256 OF THE PRODUCTION MAYAPY EXECUTABLE",
  "plugins": [
    {
      "name": "EXACT cmds.pluginInfo NAME",
      "path": "/absolute/production/plugin/path",
      "version": "EXACT cmds.pluginInfo(version=True) VALUE",
      "sha256": "SHA256 OF THIS PLUGIN"
    }
  ],
  "dependencies": [
    {"path": "/absolute/production/runtime/or/dependency", "sha256": "SHA256"}
  ]
}
```

Replace every placeholder and `api_version` with values from the known-good
production environment. Include **all** loaded plugins, including Umbrella,
and runtime/dependency hashes (including the Rust shared library). Empty or
incomplete inventories are rejected. The tool compares the actual Maya version,
API, platform, mayapy hash, loaded plugin names/paths/versions/hashes and listed
files. The profile is a caller-supplied production contract, not automatic
proof that its inventory is complete or that all dynamic-library resolution,
textures, caches and renderer behavior match production. Those require the
acceptance gate below.

Each invocation uses a unique runner/job and output directory. It opens a
snapshot with script nodes disabled, uses Maya's scene-level cleaner, saves a
separate scene through Maya, reopens it, and scans the saved bytes. It does not
replace any source file. Convenience hooks that could delete unknown or Turtle
nodes or modify local files are disabled. Maya read/write errors, version or
plugin mismatches, unknown nodes/plugins and remaining signatures fail closed.

References are currently an **unsupported safety gate**, not a malware verdict.
The runner loads no references and refuses to save referenced scenes rather than
risk altering reference edits or silently dropping data. The same applies to
`nodeGraphEditorInfo` becoming unknown in headless Maya. This stage does not
remove that legitimate node type as a virus or implement a farm-specific hook.

Only a run with `verified-results.json` is accepted by the CLI. A failed run may
leave private snapshots/intermediates for inspection; they are not approved
replacement assets. Even accepted outputs need production review before use.

## Verification and remaining acceptance

Non-Maya checks:

```sh
cargo fmt --all -- --check
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets --features python -- -D warnings
python -m unittest discover -s tests -p 'test_native_clean_guards.py' -v
```

The added CI script `scripts/maya_binary_safety_smoke.py` creates a disposable,
real `.mb` with a demand-only script node, enables protection, and requires that
Maya blocks reopening without changing its bytes. It runs in the existing real
Maya 2024 CI lane. It does not validate the standalone cleaner against production.
No mock mayapy is used as evidence of Maya acceptance.

Before migration or enabling native cleanup on production copies:

1. Obtain reporter fixtures, Maya build/OS, plugin and dependency inventory, and
   steps. Do not test in place on original assets.
2. In that exact environment, create a known-good `.mb` containing geometry,
   animation, custom plugin nodes, namespaces and external dependencies. Test
   malicious-node additions without executing payloads.
3. Compare source hashes, reopen output with the same Maya, and compare scene
   semantics, dependency paths and render behavior. Verify failure leaves sources
   untouched; test missing plugins, unknown nodes, read errors, interrupted saves,
   read-only locations and concurrent invocations on each supported OS/filesystem.
4. Implement and validate reference traversal/cleaning independently before
   allowing referenced scenes. Network payload-node parity, other callbacks,
   local reinfection sources and the optional Renderbus hook remain separate
   Issue #3 work. This PR does not claim complete Python feature parity.

Autodesk command references:
- [file: executeScriptNodes, errorStatus, ignoreVersion, loadReferenceDepth](https://help.autodesk.com/cloudhelp/2018/ENU/Maya-Tech-Docs/CommandsPython/file.html)
- [about: version and apiVersion](https://help.autodesk.com/cloudhelp/2023/ENU/Maya-Tech-Docs/CommandsPython/about.html)
