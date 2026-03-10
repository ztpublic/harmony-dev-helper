use crate::error::Result;
use crate::fs_discovery::locate_subdirectory;
use crate::resource_directory::ResourceDirectory;
use crate::utils::uri::Uri;

pub struct ElementDirectory {
    uri: Uri,
}

impl ElementDirectory {
    pub fn locate(resource_directory: &ResourceDirectory) -> Result<Option<ElementDirectory>> {
        Ok(
            locate_subdirectory(resource_directory.uri().as_path(), "element")?
                .map(|uri| Self { uri }),
        )
    }

    pub fn uri(&self) -> &Uri {
        &self.uri
    }
}
