use crate::error::Result;
use crate::fs_discovery::{find_recursive_files, locate_subdirectory};
use crate::{resource::Resource, utils::uri::Uri};

pub struct ResfileDirectory {
    uri: Uri,
}

impl ResfileDirectory {
    pub fn locate(resource: &Resource) -> Result<Option<ResfileDirectory>> {
        Ok(locate_subdirectory(resource.uri().as_path(), "resfile")?.map(|uri| Self { uri }))
    }

    pub fn uri(&self) -> &Uri {
        &self.uri
    }

    pub fn find_all(&self) -> Result<Vec<Uri>> {
        find_recursive_files(self.uri().as_path())
    }
}
