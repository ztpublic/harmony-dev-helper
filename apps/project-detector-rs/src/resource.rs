use crate::error::{DetectorError, Result};
use crate::product::Product;
use crate::utils::path::{absolute_path, path_is_dir};
use crate::utils::uri::Uri;
use std::path::{Path, PathBuf};

pub struct Resource {
    path: PathBuf,
}

impl Resource {
    pub fn find_all(product: &Product) -> Result<Vec<Resource>> {
        let mut resources = Vec::new();
        for resource_path in product.resource_directories()? {
            resources.push(Self::load(&resource_path)?);
        }

        Ok(resources)
    }

    pub fn load(resource_path: impl AsRef<Path>) -> Result<Resource> {
        let resource_path = absolute_path(resource_path.as_ref())?;
        if !path_is_dir(&resource_path)? {
            return Err(DetectorError::ExpectedDirectory {
                path: resource_path,
            });
        }

        Ok(Resource {
            path: resource_path,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn uri(&self) -> Uri {
        Uri::from_absolute_path(self.path.clone())
    }
}
