use crate::{resource_directory::ResourceDirectory, utils::uri::Uri};
use std::sync::Arc;
use std::{fs, path::Path};

pub struct ProfileDirectory {
    uri: Uri,
    resource_directory: Arc<ResourceDirectory>,
}

impl ProfileDirectory {
    pub fn from(resource_directory: &Arc<ResourceDirectory>) -> Option<Arc<ProfileDirectory>> {
        let uri = Uri::file(
            Path::new(&resource_directory.get_uri().fs_path())
                .join("profile")
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

    pub fn find_all(&self) -> Vec<Uri> {
        let mut profile_directories = Vec::new();
        let profiles = match fs::read_dir(self.uri.fs_path()) {
            Ok(profiles) => profiles,
            Err(_) => return profile_directories,
        };

        for profile in profiles.flatten() {
            if profile
                .metadata()
                .map(|metadata| metadata.is_file())
                .unwrap_or(false)
            {
                profile_directories.push(Uri::file(profile.path().to_string_lossy().to_string()));
            }
        }

        profile_directories
    }
}
