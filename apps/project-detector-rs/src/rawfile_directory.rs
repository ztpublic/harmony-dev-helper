use crate::error::Result;
use crate::fs_discovery::{find_recursive_files, locate_subdirectory};
use crate::{resource::Resource, utils::uri::Uri};
use std::path::{Path, PathBuf};

pub struct RawfileDirectory {
    path: PathBuf,
}

impl RawfileDirectory {
    pub fn locate(resource: &Resource) -> Result<Option<RawfileDirectory>> {
        Ok(locate_subdirectory(resource.path(), "rawfile")?.map(|path| Self { path }))
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
