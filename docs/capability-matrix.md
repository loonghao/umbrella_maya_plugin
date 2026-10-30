# Capability compatibility matrix: maya_umbrella → umbrella_maya_plugin

This matrix compares the **real, implemented** capabilities of the original
Python project against this Rust rewrite. It is evidence-based: a feature is
only "covered" when there is code (and, where claimed, a test) for it. README
marketing claims (e.g. "10x faster", "25+ threat patterns", "Maya 2018-2026",
"real-time protection") are **not** treated as tested facts here; they are
listed separately under [Unverified README claims](#unverified-readme-claims).

Locked commits this comparison was made against:

| Repository | Commit | Role |
|---|---|---|
| `loonghao/umbrella_maya_plugin` (this repo) | `754fec5adffce3e97ccd0be777c5c07ef00f54ab` (`main`, v0.3.2) | Rust rewrite, pre-change baseline |
| `loonghao/maya_umbrella` | `cb779f732536069a02e72dba293c0be711d10ba5` | Original Python tool |

Status legend: **已覆盖** covered · **部分** partial · **缺失** missing · **不适用** N/A.

> The base64-encoded scriptNode payload gap (row 3) is the vertical slice fixed
> by this change: see [Verification](#verification) at the end. Rows describe
> the shipped `main` baseline and are annotated where this change closes a gap.

## Detection

| # | maya_umbrella capability | Status | Evidence in Rust rewrite |
|---|---|---|---|
| 1 | Virus signatures: `JOB_SCRIPT`, `FILE`, `maya_secure_system` script nodes/files | 已覆盖 | `crates/umbrella-core/src/antivirus/signatures.rs` ports the upstream constants; `scanner_signatures()` dedups JOB+FILE by pattern; `PatternDetector::from_signatures` compiles them as `regex::bytes` |
| 2 | Offline file/directory scanning (`.ma`, `.mb`, `userSetup.py`, `.mel`) | 已覆盖 | `scanner.rs` `FileSystemScanner::scan_with_detector`, `ScanOptions::maya_scene_files()`; CLI `src/bin/umbrella-maya.rs`; FFI `umbrella_scan_file` in `src/ffi/c_api.rs` |
| 3 | ScriptNode string attributes whose payload is **base64-encoded** (`setAttr ".nts"`) | **缺失 → 已覆盖 (this change)** | Baseline `detect_bytes` scanned literal bytes only, so `tests/virus/2024-4-30.ma` and `uifiguration.ma` were reported clean. This change adds `antivirus/encoded.rs` (valid-UTF-8 text filter, no new crate dependency) + `detect_bytes` decode pass; regression `crates/umbrella-core/tests/encoded_payload_test.rs`. Also flows into the C++ live scan via `umbrella_scan_content`. See [Known limitations](#known-limitations-this-change) for the decode budget and `encodebytes` caveats |
| 4 | Live scriptNode `before`/`after`/`notes` inspection and named-node heuristics | 部分 | `src/maya/umbrella_maya_plugin.cpp` `fixInfectedScriptNodes()` (checks `uifiguration`, `maya_secure_system_scriptNode`, `_gene`, `codeExtractor`, … and calls `umbrella_scan_content`). Requires a real Maya host; no automated coverage in this repo |
| 5 | scriptJob / scriptNode detection from live scene graph | 部分 | `signatures.rs` `scanner_signatures()` includes JOB_SCRIPT patterns; C++ `killInfectedScriptJobs()`. Offline scanner cannot enumerate live jobs — host-only |
| 6 | Vaccine families: PuTianTongQi, ZeiJianKang, Virus2024429/uifiguration, maya_secure_system | 部分 | Detection via ported signatures (`uifiguration`, `maya_secure_system*`, `leukocyte_eval_deferred`, `vaccine`); the upstream **vaccine fix/hook scripts** (`vaccines/vaccine1-4.py`) are not ported as executable remediation |

## Cleaning & remediation

| # | maya_umbrella capability | Status | Evidence in Rust rewrite |
|---|---|---|---|
| 7 | Back up before cleaning; atomic writes | 已覆盖 | `antivirus/cleaner.rs` `BackupCleaner` uses `tempfile` for backups/cleaned outputs |
| 8 | Remove malicious lines from text scripts (`userSetup.py`, `.mel`, `.ma`) | 已覆盖 | `cleaner.rs` signature-removal path, guarded by `detect_bytes` before/after; CLI `umbrellaCleanFile/Directory` |
| 9 | Fail closed when a scene cannot be safely rewritten | 已覆盖 (Rust-specific safety) | `cleaner.rs` never rewrites binary `.mb`; on residual signatures returns `Failed` and preserves the original. Verified on a real `2024-4-30.ma` copy (byte-identical, non-zero exit). See `docs/binary-scene-safety.md` |
| 10 | Clean `.mb` binary scenes | 部分 / host-gated | Upstream cleans via live Maya API. Rust offline cleaner deliberately refuses `.mb`; native cleaning requires an environment profile (`--environment-profile`/`--output-dir`) and a verified Maya standalone path — not proven in this repo without a host |
| 11 | Fix infected scene references | 部分 | C++ plugin clears infected referenced script nodes (`isReferencedNode`/`clearScriptNode`); offline scanner flags infected reference files. Full reference repair is host-dependent (see `docs/binary-scene-safety.md` gates) |
| 12 | Hooks: `delete_turtle`, `delete_unknown_plugin_node`, `fix_model_panel`, `fix_no_scene_name`, `fix_on_model_change_3dc` | 缺失 | No equivalent in the Rust rewrite; upstream `hooks/` not ported |

## Real-time protection

| # | maya_umbrella capability | Status | Evidence in Rust rewrite |
|---|---|---|---|
| 13 | `MSceneMessage` callbacks (before-open check, after-open, after-save) and auto-fix | 部分 | `src/maya/umbrella_maya_plugin.cpp` registers `kBeforeOpenCheck`/`kAfterOpen`/`kAfterSave` (`onSceneBeforeOpenCheck` scans → cleans → re-verifies, else blocks open). Semantics differ from upstream `defender.py` (no `context_defender`, no global `auto_fix`). **Behavior change (this change):** a text `.ma` whose encoded scriptNode payload now matches is blocked from opening (previously it opened and the payload could execute); the offline cleaner only removes literal signatures, so it returns `Failed` and preserves the original — there is no in-Maya repair for encoded payloads. See [Known limitations](#known-limitations-this-change) for offline disposal |
| 14 | Python `MayaVirusDefender` real-time facade (`start`/`stop`/`collect`/`fix`/`report`) | 部分 / non-functional | `src/python.rs` `MayaVirusDefender` methods are **no-ops** (compatibility facade only). Real protection is the C++ plugin, not this class |

## Platform / compatibility

| # | maya_umbrella capability | Status | Evidence in Rust rewrite |
|---|---|---|---|
| 15 | Maya install-root discovery, locale script paths, per-year DevKit | 缺失 / different model | Upstream `filesystem.py` auto-discovers Maya root and locale script dirs. Rust rewrite uses explicit `MAYA_DEVKIT_PATH`/`MAYA_LOCATION` and per-year `maya_bindings` features (`Cargo.toml`) |
| 16 | i18n (en/zh) and rotating log handler | 缺失 | Upstream `i18n.py`/`log.py` not ported; Rust/C++ use plain `MGlobal` display + stderr logging |
| 17 | CLI, Python module, C ABI, native Maya `.mll` plugin | 已覆盖 (Rust-added surface) | `src/bin/umbrella-maya.rs`, `src/python.rs` (PyO3), `src/ffi/c_api.rs` (C ABI + generated header), `src/maya/umbrella_maya_plugin.cpp` (`.mll`). Upstream is Python-only |
| 18 | Build / CI / packaging / pinned toolchain | 已覆盖 | `Justfile` (`vx just preflight` = single local/CI entry), `vx.toml` (pinned `just`, Rust on rustup via `rust-toolchain.toml`), `scripts/check_toolchain_pins.py` (pin reconciliation + Windows MSVC-host guard), `.github/workflows/*.yml`, `scripts/package-python-extension.py` |

## Test & acceptance coverage

| # | Aspect | Status | Evidence |
|---|---|---|---|
| 19 | Core signature detection, cleaner safety, scanner, encoded payloads | 已覆盖 | `crates/umbrella-core` unit tests + `tests/encoded_payload_test.rs` + `cleaner` safety tests; `cargo test --workspace` (run with the pinned MSVC toolchain) |
| 20 | Native cleanup guard contract (profile/output-dir flags) | 已覆盖 | `tests/test_native_clean_guards.py`; run in CI via `python -m unittest` |
| 21 | Real Maya host: module load / command ABI / liveness | 部分 | `scripts/maya_standalone_smoke.py` — an **ABI/liveness gate, not a detection gate**; runs in CI Docker `tahv/mayapy:2024`. `scripts/maya_binary_safety_smoke.py` covers binary-scene callback regression |
| 22 | Real Maya host: functional detection/cleaning on live scene | 缺失 (host-gated) | No automated functional pass on a live Maya desktop in this repo; requires an owned host instance with provider/PID/HWND binding. Mock/CI ABI smoke must not be reported as functional acceptance |

## Unverified README claims

The following appear in `README.md` but are **marketing statements, not test
results**. Do not cite them as acceptance evidence:

- "10x faster scanning / 60x faster startup / 90% memory reduction" — no
  benchmark harness or results in the repo.
- "25+ threat patterns" — `signatures.rs` exposes a fixed signature set; the
  "25+" number is not derived from a test.
- "Maya 2018-2026" — `Cargo.toml` defines per-year `maya_bindings` features,
  but CI only builds/smoke-tests Maya 2024 (`.github/workflows/ci.yml`,
  `scripts/maya_standalone_smoke.py`). Other years are not verified here.
- "Real-time Protection / background monitoring" — only the C++ scene-message
  callbacks (row 13) back this; the Python `MayaVirusDefender` is a no-op
  (row 14).
- "bypasses Python security restrictions" — design intent, not a measured
  security property.

## Verification (this change)

The vertical slice "old candidate fails → minimal fix → regression":

1. **RED (old candidate fails).** Before the fix, a scan of the real
   `tests/virus` fixtures reported only the two `.mb` files as infected; the
   three `.ma` scenes — including `2024-4-30.ma` and `uifiguration.ma` whose
   scriptNode payloads are base64-encoded — were reported **clean**. Raw `.ma`
   fixtures contain zero literal signature hits.
2. **GREEN (minimal fix).** `antivirus/encoded.rs` recovers base64 runs
   (bounded, valid-UTF-8-text-only via `Utf8Chunks`, skips NUL/binary content,
   no new crate dependency) and `PatternDetector::detect_bytes` scans decoded
   payloads, prefixing matches with `encoded:`. There is no per-run *count* cap;
   if the 8 MiB decoded-byte budget is exhausted the scan is marked incomplete
   and fails closed (a clearly-named `encoded:scan-incomplete` finding) instead
   of silently declaring the scene clean. Because `umbrella_scan_content` (C
   ABI) calls the same `detect_bytes`, the fix also applies to live C++
   scriptNode inspection.
3. **REGRESSION.** `tests/encoded_payload_test.rs` covers: a synthetic encoded
   scriptNode, the two real legacy `.ma` fixtures, a benign base64 control, a
   binary/NUL control, the file-based `Detector::detect` path, and a padded
   scene (a huge benign run before the real payload) that must fail closed
   rather than report clean. `encoded.rs` unit tests pin the valid-UTF-8 filter
   (`'A'*128` and random bytes are rejected) and the truncated-budget flag.
   After the fix a real-fixture scan reports `2024-4-30.ma` and
   `uifiguration.ma` infected (`encoded:userSetup`, `encoded:vaccine`,
   `encoded:leukocyte_eval_deferred`, …). The cleaner remains fail-closed:
   re-scanning a cleaned-but-still-encoded real scene preserves the original
   bytes and returns `Failed` with a non-zero exit.

Reproduce:

```bash
rustup override set 1.95.0-x86_64-pc-windows-msvc   # Windows host only
vx just preflight                                   # unified local/CI entry
python scripts/check_toolchain_pins.py --self-test    # pin-guard self-test
cargo test -p umbrella-core --test encoded_payload_test
```

## Known limitations (this change)

The base64 payload recovery is deliberately conservative. These are honest
bounds, **not** claims of full coverage:

- **Decode byte budget / fail-closed.** Decoding is bounded to 8 MiB of decoded
  bytes per file; there is no per-run *count* cap (a count cap could hide a
  trailing scriptNode payload behind a wall of benign base64). A scene whose
  candidate runs exceed the budget cannot be fully verified, so the scan is
  marked incomplete and fails closed (`encoded:scan-incomplete`) — it is
  reported **unsafe, not clean**. A very large or deliberately padded scene may
  therefore need offline triage; the two real legacy fixtures (76 KB / 64 KB)
  and the 20 MB benign `jiankang_sample.ma` (no long base64 runs) are unaffected.
- **Single-line base64 only.** The run finder treats line breaks as delimiters,
  so base64 produced with line wrapping (for example Python's
  `base64.encodebytes`, 76 chars per line) is split into multiple short runs
  and a signature spanning a line boundary could be missed. Both real legacy
  samples store the payload as one continuous base64 string, so they are
  unaffected.
- **Before-open behavior / offline disposal.** For a text `.ma` containing an
  encoded scriptNode payload, the offline cleaner can only remove *literal*
  signatures, so it returns `Failed` and preserves the original; the C++
  before-open check then blocks opening the scene. There is **no in-Maya repair
  path** for encoded payloads in this change. The failure message shown by the
  C++ plugin still says "Binary scenes require verified Maya standalone
  cleaning", which is imprecise for a text `.ma`; the packaged `.mll` is not
  rebuilt here (no Maya DevKit), so the wording is documented rather than
  shipped, and the blocking behavior is unchanged. To recover, delete the
  offending `scriptNode` block from the `.ma` in a text editor (or restore
  from a clean backup) and re-scan; a verified Maya standalone live-clean is a
  separate, host-gated capability (rows 10, 22).
