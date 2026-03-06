use crate::error::{DetectorError, Result};
use crate::utils::path::path_is_dir;
use crate::{resource::Resource, utils::uri::Uri};
use std::path::Path;
use std::sync::Arc;
use walkdir::WalkDir;

pub struct RawfileDirectory {
    uri: Uri,
    resource: Arc<Resource>,
}

impl RawfileDirectory {
    pub fn from(resource: &Arc<Resource>) -> Result<Option<Arc<RawfileDirectory>>> {
        let rawfile_directory_path = Path::new(&resource.get_uri().fs_path()).join("rawfile");
        if !path_is_dir(&rawfile_directory_path)? {
            return Ok(None);
        }

        Ok(Some(Arc::new(RawfileDirectory {
            uri: Uri::file(&rawfile_directory_path)?,
            resource: Arc::clone(resource),
        })))
    }

    pub fn get_uri(&self) -> Uri {
        self.uri.clone()
    }

    pub fn get_resource(&self) -> Arc<Resource> {
        Arc::clone(&self.resource)
    }

    pub fn find_all(&self) -> Result<Vec<Uri>> {
        let rawfile_directory = self.get_uri();
        let mut files = Vec::new();
        for entry in WalkDir::new(rawfile_directory.fs_path()) {
            let entry = entry.map_err(DetectorError::walkdir)?;
            if entry.file_type().is_file() {
                files.push(Uri::file(entry.path())?);
            }
        }
        Ok(files)
    }
}
