use crate::error::Result;
use crate::resource_directory::ResourceDirectory;
use crate::utils::path::path_is_dir;
use crate::utils::uri::Uri;

pub struct ElementDirectory {
    uri: Uri,
}

impl ElementDirectory {
    pub fn locate(resource_directory: &ResourceDirectory) -> Result<Option<ElementDirectory>> {
        let element_directory_path = resource_directory.uri().as_path().join("element");
        if !path_is_dir(&element_directory_path)? {
            return Ok(None);
        }

        Ok(Some(Self {
            uri: Uri::file(&element_directory_path)?,
        }))
    }

    pub fn uri(&self) -> &Uri {
        &self.uri
    }
}
