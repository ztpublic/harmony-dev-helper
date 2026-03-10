use crate::error::Result;
use crate::fs_discovery::{find_immediate_files, locate_subdirectory};
use crate::{resource_directory::ResourceDirectory, utils::uri::Uri};
use std::path::{Path, PathBuf};

pub struct MediaDirectory {
    path: PathBuf,
}

impl MediaDirectory {
    pub fn locate(resource_directory: &ResourceDirectory) -> Result<Option<MediaDirectory>> {
        Ok(locate_subdirectory(resource_directory.path(), "media")?.map(|path| Self { path }))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn uri(&self) -> Uri {
        Uri::from_absolute_path(self.path.clone())
    }

    pub fn find_all(&self) -> Result<Vec<PathBuf>> {
        find_immediate_files(self.path())
    }
}
