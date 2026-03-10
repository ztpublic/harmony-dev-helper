use crate::error::Result;
use crate::fs_discovery::{find_recursive_files, locate_subdirectory};
use crate::{resource::Resource, utils::uri::Uri};
use std::path::{Path, PathBuf};

pub struct ResfileDirectory {
    path: PathBuf,
}

impl ResfileDirectory {
    pub fn locate(resource: &Resource) -> Result<Option<ResfileDirectory>> {
        Ok(locate_subdirectory(resource.path(), "resfile")?.map(|path| Self { path }))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn uri(&self) -> Uri {
        Uri::from_absolute_path(self.path.clone())
    }

    pub fn find_all(&self) -> Result<Vec<PathBuf>> {
        find_recursive_files(self.path())
    }
}
