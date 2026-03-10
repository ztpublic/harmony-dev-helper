use crate::error::Result;
use crate::fs_discovery::{find_immediate_files, locate_subdirectory};
use crate::{resource_directory::ResourceDirectory, utils::uri::Uri};

pub struct MediaDirectory {
    uri: Uri,
}

impl MediaDirectory {
    pub fn locate(resource_directory: &ResourceDirectory) -> Result<Option<MediaDirectory>> {
        Ok(
            locate_subdirectory(resource_directory.uri().as_path(), "media")?
                .map(|uri| Self { uri }),
        )
    }

    pub fn uri(&self) -> &Uri {
        &self.uri
    }

    pub fn find_all(&self) -> Result<Vec<Uri>> {
        find_immediate_files(self.uri().as_path())
    }
}
