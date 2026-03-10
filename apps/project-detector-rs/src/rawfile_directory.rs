use crate::error::Result;
use crate::fs_discovery::{find_recursive_files, locate_subdirectory};
use crate::{resource::Resource, utils::uri::Uri};

pub struct RawfileDirectory {
    uri: Uri,
}

impl RawfileDirectory {
    pub fn locate(resource: &Resource) -> Result<Option<RawfileDirectory>> {
        Ok(locate_subdirectory(resource.uri().as_path(), "rawfile")?.map(|uri| Self { uri }))
    }

    pub fn uri(&self) -> &Uri {
        &self.uri
    }

    pub fn find_all(&self) -> Result<Vec<Uri>> {
        find_recursive_files(self.uri().as_path())
    }
}
