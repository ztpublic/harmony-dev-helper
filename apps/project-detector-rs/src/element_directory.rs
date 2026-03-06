use crate::error::Result;
use crate::resource_directory::ResourceDirectory;
use crate::utils::path::path_is_dir;
use crate::utils::uri::Uri;
use std::path::Path;
use std::sync::Arc;

pub struct ElementDirectory {
    uri: Uri,
    resource_directory: Arc<ResourceDirectory>,
}

impl ElementDirectory {
    pub fn from(
        resource_directory: &Arc<ResourceDirectory>,
    ) -> Result<Option<Arc<ElementDirectory>>> {
        let element_directory_path =
            Path::new(&resource_directory.get_uri().fs_path()).join("element");
        if !path_is_dir(&element_directory_path)? {
            return Ok(None);
        }

        Ok(Some(Arc::new(Self {
            uri: Uri::file(&element_directory_path)?,
            resource_directory: Arc::clone(resource_directory),
        })))
    }

    pub fn get_uri(&self) -> Uri {
        self.uri.clone()
    }

    pub fn get_resource_directory(&self) -> Arc<ResourceDirectory> {
        Arc::clone(&self.resource_directory)
    }
}
