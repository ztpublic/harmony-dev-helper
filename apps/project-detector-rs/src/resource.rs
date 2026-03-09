use crate::error::{DetectorError, Result};
use crate::product::Product;
use crate::utils::path::path_is_dir;
use crate::utils::uri::Uri;
use std::path::Path;

pub struct Resource {
    uri: Uri,
}

impl Resource {
    pub fn find_all(product: &Product) -> Result<Vec<Resource>> {
        let mut resources = Vec::new();
        for resource_uri in product.resource_directories()? {
            resources.push(Self::load(resource_uri.as_path())?);
        }

        Ok(resources)
    }

    pub fn load(resource_path: impl AsRef<Path>) -> Result<Resource> {
        let resource_path = resource_path.as_ref().to_path_buf();
        if !path_is_dir(&resource_path)? {
            return Err(DetectorError::ExpectedDirectory {
                path: resource_path,
            });
        }

        let uri = Uri::file(&resource_path)?;
        Ok(Resource { uri })
    }

    pub fn uri(&self) -> &Uri {
        &self.uri
    }
}
