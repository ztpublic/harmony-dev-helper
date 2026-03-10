# `project-detector-rs` Refactor TODO

## Summary

`apps/project-detector-rs` is functional, but it does not match the style or maintainability level of the newer Rust crates in this repo, especially `apps/hdckit-rs` and `apps/hdc-bridge-rs`.

Current state from local verification:

- `cargo test --manifest-path apps/project-detector-rs/Cargo.toml` passes.
- `cargo clippy --manifest-path apps/project-detector-rs/Cargo.toml --all-targets -- -D warnings` passes.
- The main issues are API shape, ownership patterns, duplication, stringly-typed parsing, and copied-upstream style rather than obvious red test failures.

This document is a decision-ready TODO list for bringing the crate closer to the repo's Rust style without losing the current behavior.

## Main Findings

### 1. The public API is Java-style and clone-heavy

Examples:

- `Project`, `Module`, `Product`, `Resource`, `ResourceDirectory`, `ElementJsonFile`, and `ElementJsonFileReference` expose many `get_*` methods.
- Many getters clone owned data (`String`, `Uri`, `serde_json::Value`) even when callers only need borrowed access.
- The crate leans on `Arc<T>` throughout the entire object graph, even for simple parent links and short-lived values.

This is notably different from the other Rust crates here, which prefer:

- smaller value types
- narrower public surfaces
- explicit constructors like `new(...)` / `from_env(...)`
- focused methods with clear input/output ownership

### 2. Build-profile parsing and validation are duplicated

Examples:

- `src/project.rs` and `src/module.rs` both read `build-profile.json5`, parse `serde_json::Value`, validate shape, store raw text, and implement similar `reload` logic.
- Validation rules are embedded in the loader methods instead of being centralized in typed config parsing.

This makes future changes to build-profile shape harder and increases drift risk.

### 3. Config handling is too stringly typed

Examples:

- `Project`, `Module`, and `Product` navigate `serde_json::Value` directly.
- `Product::get_current_target_config`, `get_source_directories`, and `get_resource_directories` repeatedly dig through ad hoc JSON access chains.
- Missing or malformed values often degrade into empty strings or `Value::Null` instead of clearer typed behavior.

Compared with the other Rust crates in this repo, this is the largest design gap.

### 4. Filesystem traversal logic is repetitive and inconsistent

Examples:

- `media_directory.rs`, `profile_directory.rs`, `rawfile_directory.rs`, and `resfile_directory.rs` all repeat similar "locate subdirectory, iterate entries, map to `Uri`" logic.
- `ResourceDirectory::find_all` uses `dirs.flatten()`, which silently drops read errors.
- Path/URI conversion bounces between `String`, `Path`, `PathBuf`, `Uri`, and `to_string_lossy()` more than necessary.

The newer crates are more explicit about path ownership and error propagation.

### 5. The parser layer is more complex than it needs to be

Examples:

- Each `ElementJsonFile` stores an `Arc<Mutex<tree_sitter::Parser>>`.
- `ElementJsonFileReference::find_all` is a large nested traversal with several mutable `Option` accumulators.
- The parse API mixes "load file", "keep source text", "own parser", and "extract references" responsibilities in one cluster.

The code works, but it is difficult to reason about and expensive to modify safely.

### 6. Some code still reads like copied upstream internals instead of a maintained internal crate

Examples:

- `src/utils/byteorder.rs` contains non-English comments and low-level exported C shims without crate-level explanation.
- `src/lib.rs` re-exports the internal module layout almost verbatim instead of presenting a deliberate public API.
- Several types exist mostly as wrappers around a parent pointer and a `Uri`.

This is a style mismatch with the rest of the repo, where internal crates are more intentional about public surface and local conventions.

## Target Style

Use `apps/hdckit-rs` and `apps/hdc-bridge-rs` as the style baseline for the Rust side of this repo.

Adopt these rules:

- Prefer small structs with clear ownership over deep object graphs with pervasive `Arc`.
- Use idiomatic method names. Prefer `new`, `load`, `parse`, `find_*`, `path`, `uri`, `name`, `content`, etc. Avoid blanket `get_*` / `update_*` naming for internal APIs.
- Return borrowed data where possible. Clone only at clear ownership boundaries.
- Prefer typed config structs over repeated `serde_json::Value` traversal.
- Keep loaders, pure value types, and parser/extractor logic separated.
- Keep error propagation explicit. Do not silently ignore directory iteration errors.
- Keep comments sparse, technical, and in English.
- Keep `lib.rs` intentional: export supported entrypoints, not every implementation detail by default.

## Prioritized TODOs

### [x] P0: Fix design choices that hide bugs or make future changes unsafe

Completed:

- Replaced `dirs.flatten()` in `src/resource_directory.rs` with explicit iteration and error propagation, and stopped swallowing entry metadata errors on the directory scan path.
- Centralized project/module build-profile loading and validation in `src/build_profile.rs`, with shared typed config structs used by `Project`, `Module`, and `Product`.
- Preserved and expanded `resolve_within(...)`-style path safety by validating config-derived `srcPath`, `sourceRoots`, and `resource.directories` inputs before traversal.
- Removed the `Value::Null` / empty-string fallback behavior in `Product` by storing validated typed target config instead of re-reading ad hoc JSON and silently defaulting malformed values.

Verification:

- `cargo fmt --manifest-path apps/project-detector-rs/Cargo.toml`
- `cargo test --manifest-path apps/project-detector-rs/Cargo.toml`
- `cargo clippy --manifest-path apps/project-detector-rs/Cargo.toml --all-targets -- -D warnings`

### [x] P1: Make the public and internal APIs idiomatic

Completed:

- Renamed the main constructor/accessor surface to idiomatic Rust names such as `new`, `load`, `locate`, `uri`, `name`, `workspace_folder`, `content`, and `qualifiers`.
- Switched the crate from clone-heavy `get_*` accessors to borrowed accessors by default, especially for `Uri`, `String`, and build-profile content.
- Removed the parent-link `Arc<T>` object graph across `Project`, `Module`, `Product`, `Resource`, `ResourceDirectory`, and the directory wrapper types; traversal now uses plain values and borrows.
- Collapsed several trivial wrappers down to `uri`-centric value objects instead of `uri + parent Arc` holders.
- Updated tests and README examples to use the new plain-value traversal API rather than `Arc` chaining.

### [x] P1: Replace stringly config access with typed structures

Completed:

- Introduced typed project/module/target/source/resource build-profile structs in `src/build_profile.rs`.
- Kept JSON5 parsing centralized and one-shot, then built `Project`, `Module`, and `Product` from that typed layer.
- Removed repeated ad hoc `serde_json::Value` target lookups in `Product`.
- Tightened invalid-config handling so malformed names and path lists fail at the build-profile layer instead of leaking through string defaults.

Verification:

- `cargo fmt --manifest-path apps/project-detector-rs/Cargo.toml`
- `cargo test --manifest-path apps/project-detector-rs/Cargo.toml`
- `cargo clippy --manifest-path apps/project-detector-rs/Cargo.toml --all-targets -- -D warnings`

### [x] P2: Simplify parser and resource discovery code

Completed:

- Split `src/files/element_json_file.rs` into smaller loading and parsing helpers so file I/O, JSON5 parsing, and tree-sitter parsing each have their own entrypoint.
- Removed the per-instance parser ownership from `ElementJsonFile`; tree-sitter parsers are now created locally for parse work instead of being stored behind a mutex on every file value.
- Reworked `src/references/element_json_file_reference.rs` into helper-based traversal using explicit pair/object parsing invariants rather than one large nested walk.
- Added focused `ElementJsonFileReference` tests for missing `name`, missing `value`, non-string fields, and nested unrelated JSON nodes.
- Deduplicated resource subdirectory discovery and file collection in a shared internal filesystem helper used by `ElementDirectory`, `MediaDirectory`, `ProfileDirectory`, `RawfileDirectory`, `ResfileDirectory`, and `ResourceDirectory`.

Verification:

- `cargo fmt --manifest-path apps/project-detector-rs/Cargo.toml`
- `cargo test --manifest-path apps/project-detector-rs/Cargo.toml`
- `cargo clippy --manifest-path apps/project-detector-rs/Cargo.toml --all-targets -- -D warnings`

### [x] P2: Normalize path and URI handling

Completed:

- Moved filesystem-native entity storage to normalized `PathBuf`s across `ProjectDetector`, `Project`, `Module`, `Product`, `Resource`, `ResourceDirectory`, the resource subdirectory wrappers, and `ElementJsonFile`.
- Added borrowed `path()` / `workspace_path()` accessors and changed internal traversal to stay on `&Path` / `PathBuf` until a `Uri` is explicitly requested.
- Changed the remaining filesystem-facing constructors and helpers to take `&Path` / `PathBuf` instead of string paths, while keeping explicit URI parsing only at boundary methods such as `ProjectDetector::from_uri(...)`.
- Simplified `src/utils/uri.rs` by removing the unused string-returning helper surface that encouraged path/URI/string round-trips and keeping it focused on file-URI creation and parsing.
- Updated README examples and tests to use path-first access, and added focused tests covering relative-path normalization and `file://` URI acceptance.

Verification:

- `cargo fmt --manifest-path apps/project-detector-rs/Cargo.toml`
- `cargo test --manifest-path apps/project-detector-rs/Cargo.toml`
- `cargo clippy --manifest-path apps/project-detector-rs/Cargo.toml --all-targets -- -D warnings`

### [x] P3: Tighten module layout and crate surface

Completed:

- Replaced the source-tree-shaped public module surface in `src/lib.rs` with an intentional crate-root API that re-exports the supported traversal types, parser/reference types, `Uri`, and shared error types.
- Moved implementation-oriented modules such as `files`, `references`, and `utils` behind a private crate module tree, keeping only crate-internal visibility where the implementation still needs it.
- Updated tests and README examples to consume the crate through root-level re-exports instead of implementation module paths.
- Added crate-level documentation in `src/lib.rs` describing the intended entrypoints, internal layering, and the fact that the source-tree module layout is not the supported API surface.
- Kept `src/utils/byteorder.rs` as a private compatibility shim and documented why it still exists: it provides libbsd-style byteorder symbols for environments that may resolve them dynamically at runtime.

Verification:

- `cargo fmt --manifest-path apps/project-detector-rs/Cargo.toml`
- `cargo test --manifest-path apps/project-detector-rs/Cargo.toml`
- `cargo clippy --manifest-path apps/project-detector-rs/Cargo.toml --all-targets -- -D warnings`

### P3: Bring tests in line with the cleanup

- Keep the current fixture-based integration flow test.
- Add focused unit tests for typed build-profile parsing and validation.
- Add tests for directory read failures and missing files/directories.
- Add tests for malformed `sourceRoots` / `resource.directories` entries.
- Add narrower tests for `ElementJsonFileReference` edge cases such as missing `name`, missing `value`, non-string fields, and nested unrelated JSON nodes.
- Add tests around URI/path normalization so refactors do not weaken path safety.

## Concrete Refactor Targets

These areas should change first because they unlock most of the cleanup:

- `src/project.rs`
  - Extract shared build-profile parsing and validation.
  - Stop storing raw parsed config as generic `serde_json::Value`.

- `src/module.rs`
  - Share the same config parsing path as `Project`.
  - Remove duplicated reload and validation flow.

- `src/product.rs`
  - Replace ad hoc target JSON navigation with typed target config.
  - Replace silent defaulting with explicit result handling.

- `src/resource_directory.rs`
  - Stop dropping iteration errors.
  - Centralize qualifier directory acceptance logic.

- `src/files/element_json_file.rs`
  - Separate file loading from parsing and reference extraction.

- `src/references/element_json_file_reference.rs`
  - Split traversal into helpers and reduce nested mutation.

## Suggested Implementation Order

1. Introduce typed config structs and shared build-profile parsing.
2. Refactor `Project`, `Module`, and `Product` to depend on typed config instead of raw `Value`.
3. Normalize path/URI ownership and remove clone-heavy getters.
4. Simplify resource directory scanning and remove duplicated directory wrapper logic.
5. Refactor element JSON parsing and reference extraction.
6. Tighten `lib.rs` exports and crate docs.
7. Expand targeted tests after each major step.

## Acceptance Criteria For The Future Code Pass

- `cargo test --manifest-path apps/project-detector-rs/Cargo.toml` still passes.
- `cargo clippy --manifest-path apps/project-detector-rs/Cargo.toml --all-targets -- -D warnings` still passes.
- No directory iteration errors are silently ignored.
- Build-profile parsing lives in one shared typed layer.
- The crate no longer exposes clone-heavy `serde_json::Value`-based access as the primary API.
- The number of unconditional `Arc` parent links is meaningfully reduced.
- The public API is smaller, more intentional, and closer to the rest of the repo's Rust style.

## Notes

- This should be treated as a staged refactor, not a single rewrite.
- Preserve existing behavior first, then narrow the API.
- Do not mechanically rename everything before the ownership and config model are improved; otherwise the crate will look more idiomatic while staying structurally hard to maintain.
