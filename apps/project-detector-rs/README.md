# project-detector-rs

`project-detector-rs` is an internal Rust crate for discovering Harmony projects, modules, targets, and resource files from a workspace on disk.

This crate was copied from the Rust implementation inside the external `project-detector` source tree and converted into a plain Rust library. It is not a Node binding, and it is not wired into any runtime application yet. Its current role is to provide a reusable internal detector that other Rust code can call directly later.

## Status

- Internal crate, not published.
- API is usable, but not yet stabilized for external consumers.
- Focused on filesystem discovery and light parsing, not build execution.
- Tested against local fixture projects in [`tests/fixtures/mock`](tests/fixtures/mock).

## What It Detects

Given a workspace folder, the crate can:

- Walk the workspace and find Harmony projects by locating `build-profile.json5`.
- Load project modules from the project `modules` array.
- Load products from a module `targets` array.
- Resolve source directories and resource directories for each product.
- Discover resource qualifier directories such as `base` and `dark`.
- Discover `element`, `media`, `profile`, `rawfile`, and `resfile` subdirectories where applicable.
- Parse `element/*.json` files and extract `{ name, value }` references with source offsets.

## Current Model

The crate models the workspace as a hierarchy:

`ProjectDetector -> Project -> Module -> Product -> Resource -> ResourceDirectory`

Additional helper types hang off resource directories:

- `ElementDirectory`
- `MediaDirectory`
- `ProfileDirectory`
- `RawfileDirectory`
- `ResfileDirectory`
- `ElementJsonFile`
- `ElementJsonFileReference`

Objects are plain values and are traversed by borrowing rather than keeping parent-link `Arc` chains.

## Public API

The main entry points are:

```rust
use project_detector_rs::{Project, ProjectDetector};
use std::path::Path;

fn load_projects(workspace: &Path) -> project_detector_rs::Result<()> {
    let detector = ProjectDetector::new(workspace)?;
    let projects = Project::find_all(&detector)?;

    for project in projects {
        println!("{}", project.path().display());
    }

    Ok(())
}
```

Typical traversal looks like this:

```rust
use project_detector_rs::{
    ElementDirectory, ElementJsonFile, ElementJsonFileReference, Module, Product, Project,
    ProjectDetector, Resource, ResourceDirectory,
};
use std::path::Path;

fn scan(workspace: &Path) -> project_detector_rs::Result<()> {
    let detector = ProjectDetector::new(workspace)?;

    for project in Project::find_all(&detector)? {
        for module in Module::find_all(&project)? {
            for product in Product::find_all(&module) {
                for resource in Resource::find_all(&product)? {
                    for resource_dir in ResourceDirectory::find_all(&resource)? {
                        if let Some(element_dir) = ElementDirectory::locate(&resource_dir)? {
                            for file in ElementJsonFile::find_all(&element_dir)? {
                                let references = ElementJsonFileReference::find_all(&file)?;
                                println!(
                                    "{} -> {} references",
                                    file.path().display(),
                                    references.len()
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
```

## Error Handling

The crate uses [`DetectorError`](src/error.rs) for all fallible operations.

Important error cases:

- invalid or unsupported URIs
- filesystem read failures
- invalid `build-profile.json5`
- invalid `element/*.json`
- Tree-sitter parser failures
- config paths that escape the owning project or module root

The detector now treats malformed paths and parse failures as real errors instead of silently defaulting to empty paths or empty parse results.

## Filesystem Rules

The detector expects real files on disk and primarily uses native filesystem paths internally. `file://` URIs are accepted only at explicit URI boundaries such as `ProjectDetector::from_uri(...)`.

Notable behavior:

- Non-`file` URIs are rejected.
- Relative workspace paths are normalized to absolute paths.
- Project scanning skips hidden directories, `node_modules`, and `oh_modules`.
- Config-derived paths such as `srcPath`, `source.sourceRoots`, and `resource.directories` are constrained to remain under the owning project or module root.

## Resource Discovery

`ResourceDirectory::find_all` recognizes:

- `base`
- `rawfile`
- `resfile`
- qualifier directories accepted by `QualifierUtils`

From there, callers can opt into narrower views:

- `ElementDirectory::locate(...)` for `element/`
- `MediaDirectory::locate(...)` for `media/`
- `ProfileDirectory::locate(...)` for `profile/`
- `RawfileDirectory::locate(...)` for `rawfile/`
- `ResfileDirectory::locate(...)` for `resfile/`

`ElementJsonFileReference::find_all(...)` returns extracted references with:

- element type
- name/value text
- quoted full text
- character offsets
- convenience conversions to ETS and JSON placeholder formats

## Implementation Notes

- Project and module build profiles are parsed once into typed internal config structs before domain objects are built.
- Filesystem-native types now store normalized `PathBuf`s and expose borrowed `path()` accessors; `Uri` values are derived only when callers explicitly need them.
- The parser logic for `element/*.json` is low-level but now isolated from the rest of the path-handling code.

## Known Rough Edges

- The API surface is broader than the currently tested integration path.
- This crate should be treated as an internal dependency under active cleanup, not a polished public library yet.

## Development

Run the crate tests from the repository root:

```sh
cargo test --manifest-path apps/project-detector-rs/Cargo.toml
```

Lint the crate:

```sh
cargo clippy --manifest-path apps/project-detector-rs/Cargo.toml --all-targets -- -D warnings
```

Format the crate:

```sh
cargo fmt --manifest-path apps/project-detector-rs/Cargo.toml
```

## Repository Layout

- [`src/project_detector.rs`](src/project_detector.rs): workspace entry point
- [`src/project.rs`](src/project.rs): project discovery and loading
- [`src/module.rs`](src/module.rs): module loading
- [`src/product.rs`](src/product.rs): target/product resolution
- [`src/resource.rs`](src/resource.rs): resource roots
- [`src/resource_directory.rs`](src/resource_directory.rs): qualifier directory discovery
- [`src/files/element_json_file.rs`](src/files/element_json_file.rs): element file loading and parsing
- [`src/references/element_json_file_reference.rs`](src/references/element_json_file_reference.rs): element reference extraction
- [`src/error.rs`](src/error.rs): shared error type
