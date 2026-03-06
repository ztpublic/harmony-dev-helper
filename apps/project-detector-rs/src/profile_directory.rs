use crate::error::{DetectorError, Result};
use crate::utils::path::path_is_dir;
use crate::{resource_directory::ResourceDirectory, utils::uri::Uri};
use std::sync::Arc;
use std::{fs, path::Path};

pub struct ProfileDirectory {
    uri: Uri,
    resource_directory: Arc<ResourceDirectory>,
}

impl ProfileDirectory {
    pub fn from(
        resource_directory: &Arc<ResourceDirectory>,
    ) -> Result<Option<Arc<ProfileDirectory>>> {
        let profile_directory_path =
            Path::new(&resource_directory.get_uri().fs_path()).join("profile");
        if !path_is_dir(&profile_directory_path)? {
            return Ok(None);
        }

        Ok(Some(Arc::new(Self {
            uri: Uri::file(&profile_directory_path)?,
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
        let mut profile_directories = Vec::new();
        let profiles = fs::read_dir(self.uri.fs_path())
            .map_err(|source| DetectorError::io(self.uri.fs_path(), source))?;

        for profile in profiles {
            let profile =
                profile.map_err(|source| DetectorError::io(self.uri.fs_path(), source))?;
            let path = profile.path();
            let metadata = profile
                .metadata()
                .map_err(|source| DetectorError::io(path.clone(), source))?;
            if metadata.is_file() {
                profile_directories.push(Uri::file(&path)?);
            }
        }

        Ok(profile_directories)
    }
}
