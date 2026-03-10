use crate::error::Result;
use crate::fs_discovery::{find_immediate_files, locate_subdirectory};
use crate::{resource_directory::ResourceDirectory, utils::uri::Uri};

pub struct ProfileDirectory {
    uri: Uri,
}

impl ProfileDirectory {
    pub fn locate(resource_directory: &ResourceDirectory) -> Result<Option<ProfileDirectory>> {
        Ok(
            locate_subdirectory(resource_directory.uri().as_path(), "profile")?
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
