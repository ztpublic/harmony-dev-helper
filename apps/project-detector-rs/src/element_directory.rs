use crate::resource_directory::ResourceDirectory;
use crate::utils::uri::Uri;
use std::fs;
use std::path::Path;
use std::sync::Arc;

pub struct ElementDirectory {
    uri: Uri,
    resource_directory: Arc<ResourceDirectory>,
}

impl ElementDirectory {
    pub fn from(resource_directory: &Arc<ResourceDirectory>) -> Option<Arc<ElementDirectory>> {
        let uri = Uri::file(
            Path::new(&resource_directory.get_uri().fs_path())
                .join("element")
                .to_string_lossy()
                .to_string(),
        );
        if !fs::metadata(uri.fs_path())
            .map(|metadata| metadata.is_dir())
            .unwrap_or(false)
        {
            return None;
        }

        Some(Arc::new(Self {
            uri,
            resource_directory: Arc::clone(resource_directory),
        }))
    }

    pub fn get_uri(&self) -> Uri {
        self.uri.clone()
    }

    pub fn get_resource_directory(&self) -> Arc<ResourceDirectory> {
        Arc::clone(&self.resource_directory)
    }
}
