use crate::error::Result;
use crate::fs_discovery::locate_subdirectory;
use crate::resource_directory::ResourceDirectory;
use crate::utils::uri::Uri;
use std::path::{Path, PathBuf};

pub struct ElementDirectory {
    path: PathBuf,
}

impl ElementDirectory {
    pub fn locate(resource_directory: &ResourceDirectory) -> Result<Option<ElementDirectory>> {
        Ok(locate_subdirectory(resource_directory.path(), "element")?.map(|path| Self { path }))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn uri(&self) -> Uri {
        Uri::from_absolute_path(self.path.clone())
    }
}
