use crate::{resource::Resource, utils::uri::Uri};
use std::sync::Arc;
use std::{fs, path::Path};
use walkdir::WalkDir;

pub struct ResfileDirectory {
    uri: Uri,
    resource: Arc<Resource>,
}

impl ResfileDirectory {
    pub fn from(resource: &Arc<Resource>) -> Option<Arc<ResfileDirectory>> {
        let uri = Uri::file(
            Path::new(&resource.get_uri().fs_path())
                .join("resfile")
                .to_string_lossy()
                .to_string(),
        );
        if !fs::metadata(uri.fs_path())
            .map(|metadata| metadata.is_dir())
            .unwrap_or(false)
        {
            return None;
        }

        Some(Arc::new(ResfileDirectory {
            uri,
            resource: Arc::clone(resource),
        }))
    }

    pub fn get_uri(&self) -> Uri {
        self.uri.clone()
    }

    pub fn get_resource(&self) -> Arc<Resource> {
        Arc::clone(&self.resource)
    }

    pub fn find_all(&self) -> Vec<Uri> {
        let resfile_directory = self.get_uri();
        WalkDir::new(resfile_directory.fs_path())
            .into_iter()
            .filter_map(|res| res.ok())
            .filter(|entry| entry.file_type().is_file())
            .map(|entry| Uri::file(entry.path().to_string_lossy().to_string()))
            .collect()
    }
}
