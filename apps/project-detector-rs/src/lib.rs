//! Internal architecture for `project-detector-rs`.
//!
//! Supported entrypoints are re-exported from the crate root:
//! `ProjectDetector`, `Project`, `Module`, `Product`, `Resource`,
//! `ResourceDirectory`, the resource subdirectory wrappers, `ElementJsonFile`,
//! `ElementJsonFileReference`, `Uri`, and the shared error types.
//!
//! The source-tree module layout is intentionally private. Internal layering is:
//! typed build-profile loading, filesystem discovery helpers, traversal/domain
//! types, then element-file parsing/reference extraction on top.
//!
//! `utils::byteorder` is kept as a private compatibility shim because some
//! upstream-linked Linux environments may still expect libbsd-style byteorder
//! symbols at runtime. It is not part of the supported crate API.

mod build_profile;
mod element_directory;
mod error;
mod files;
mod fs_discovery;
mod media_directory;
mod module;
mod product;
mod profile_directory;
mod project;
mod project_detector;
mod rawfile_directory;
mod references;
mod resfile_directory;
mod resource;
mod resource_directory;
mod utils;

pub use element_directory::ElementDirectory;
pub use error::{DetectorError, Result};
pub use files::element_json_file::ElementJsonFile;
pub use media_directory::MediaDirectory;
pub use module::Module;
pub use product::Product;
pub use profile_directory::ProfileDirectory;
pub use project::Project;
pub use project_detector::ProjectDetector;
pub use rawfile_directory::RawfileDirectory;
pub use references::element_json_file_reference::ElementJsonFileReference;
pub use resfile_directory::ResfileDirectory;
pub use resource::Resource;
pub use resource_directory::ResourceDirectory;
pub use utils::uri::Uri;
