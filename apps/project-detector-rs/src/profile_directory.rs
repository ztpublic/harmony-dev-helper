use crate::error::Result;
use crate::fs_discovery::{find_immediate_files, locate_subdirectory};
use crate::{resource_directory::ResourceDirectory, utils::uri::Uri};
use std::path::{Path, PathBuf};

pub struct ProfileDirectory {
    path: PathBuf,
}

impl ProfileDirectory {
    pub fn locate(resource_directory: &ResourceDirectory) -> Result<Option<ProfileDirectory>> {
        Ok(locate_subdirectory(resource_directory.path(), "profile")?.map(|path| Self { path }))
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
