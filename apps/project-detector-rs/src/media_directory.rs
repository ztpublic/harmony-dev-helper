use crate::error::{DetectorError, Result};
use crate::utils::path::path_is_dir;
use crate::{resource_directory::ResourceDirectory, utils::uri::Uri};
use std::sync::Arc;
use std::{fs, path::Path};

pub struct MediaDirectory {
    uri: Uri,
    resource_directory: Arc<ResourceDirectory>,
}

impl MediaDirectory {
    pub fn from(
        resource_directory: &Arc<ResourceDirectory>,
    ) -> Result<Option<Arc<MediaDirectory>>> {
        let media_directory_path = Path::new(&resource_directory.get_uri().fs_path()).join("media");
        if !path_is_dir(&media_directory_path)? {
            return Ok(None);
        }

        Ok(Some(Arc::new(Self {
            uri: Uri::file(&media_directory_path)?,
            resource_directory: Arc::clone(resource_directory),
        })))
    }

    pub fn get_uri(&self) -> Uri {
        self.uri.clone()
    }

    pub fn get_resource_directory(&self) -> Arc<ResourceDirectory> {
        Arc::clone(&self.resource_directory)
    }

    pub fn find_all(&self) -> Result<Vec<Uri>> {
        let mut media_files = Vec::new();
        let media_directory = self.get_uri();
        let dirs = fs::read_dir(media_directory.fs_path())
            .map_err(|source| DetectorError::io(media_directory.fs_path(), source))?;

        for dir in dirs {
            let dir = dir.map_err(|source| DetectorError::io(media_directory.fs_path(), source))?;
            let path = dir.path();
            let metadata = dir
                .metadata()
                .map_err(|source| DetectorError::io(path.clone(), source))?;
            if metadata.is_file() {
                media_files.push(Uri::file(&path)?);
            }
        }

        Ok(media_files)
    }
}
