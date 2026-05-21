# Architecture

Umbrella is split around a small domain crate and thin outer adapters.

## Crates

`crates/umbrella-core` is the domain module. It owns virus signatures, detection, file discovery, cleaning, backup rules, reports, and domain errors. It has no Maya, PyO3, C ABI, CLI, or build-system dependencies.

The root `umbrella_maya_plugin` crate is the adapter layer. It exposes the core through C ABI functions, PyO3 bindings, the CLI binary, and the Maya C++ plugin build. Adapter code converts transport-specific types into core interfaces and translates reports back out.

## Contracts

The core interfaces are the contract surface:

- `PatternDetector::detect` reports upstream signatures with file path, threat names, line numbers, and concrete matches.
- `FileSystemScanner::scan_with_detector` traverses supported Maya/script files and ignores backup directories such as `_virus`.
- `BackupCleaner::clean` creates upstream-style backups, removes only known removable signatures by default, and reports whether files were cleaned, deleted, already clean, or failed.

These contracts are covered by `crates/umbrella-core/tests/domain_contract_test.rs`.

## Dependency Rule

Dependencies point inward:

- Maya C++ plugin, FFI, Python extension, CLI, and GitHub Actions depend on the root adapter crate.
- The root adapter crate depends on `umbrella-core`.
- `umbrella-core` depends only on general-purpose Rust libraries.

The domain crate must not import Maya APIs, PyO3, C ABI types, command-line parsing, or build tooling.

## Maintenance Rules

- Add new virus families first as core signatures and contract tests.
- Add host-specific behavior as adapters around core reports, not inside core detection.
- Keep Maya plugin initialization transactional: if one command registration fails, previously registered commands must be deregistered before returning failure.
- Build outputs must place the Maya plugin and Rust runtime library in the same directory. The plugin loader depends on that contract.
