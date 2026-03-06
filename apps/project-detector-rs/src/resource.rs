use crate::error::{DetectorError, Result};
use crate::product::Product;
use crate::utils::path::path_is_dir;
use crate::utils::uri::Uri;
use std::path::PathBuf;
use std::sync::Arc;

pub struct Resource {
    product: Arc<Product>,
    uri: Uri,
}

impl Resource {
    pub fn find_all(product: &Arc<Product>) -> Result<Vec<Arc<Resource>>> {
        let mut resources = Vec::new();
        for resource_uri in product.get_resource_directories()? {
            resources.push(Self::create(product, resource_uri.fs_path())?);
        }

        Ok(resources)
    }

    pub fn create(product: &Arc<Product>, resource_uri: String) -> Result<Arc<Resource>> {
        let resource_path = PathBuf::from(&resource_uri);
        if !path_is_dir(&resource_path)? {
            return Err(DetectorError::ExpectedDirectory {
                path: resource_path,
            });
        }

        let uri = Uri::file(&resource_path)?;
        Ok(Arc::new(Resource {
            product: Arc::clone(product),
            uri,
        }))
    }

    pub fn get_product(&self) -> Arc<Product> {
        Arc::clone(&self.product)
    }

    pub fn get_uri(&self) -> Uri {
        self.uri.clone()
    }
}
