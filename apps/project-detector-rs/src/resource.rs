use crate::product::Product;
use crate::utils::uri::Uri;
use std::sync::Arc;
use std::{fs, path::Path};

pub struct Resource {
    product: Arc<Product>,
    uri: Uri,
}

impl Resource {
    pub fn find_all(product: &Arc<Product>) -> Vec<Arc<Resource>> {
        let mut resources = Vec::new();
        let current_target_config = product.get_current_target_config();
        let name = product.get_name();
        let module_uri = product.get_module().get_uri();

        if current_target_config.is_null() {
            return resources;
        }

        let default_child_path = if name == "default" { "main" } else { &name };
        let default_resource_root = Path::new(&module_uri.fs_path())
            .join("src")
            .join(default_child_path)
            .join("resources");
        let resource_roots = current_target_config
            .get("resource")
            .and_then(|resource| resource.get("directories"))
            .and_then(|resource_roots| resource_roots.as_array());

        if let Some(resource_roots) = resource_roots {
            if !resource_roots.is_empty() {
                for resource_root in resource_roots {
                    let resource_root_path = path_clean::clean(
                        Path::new(&module_uri.fs_path())
                            .join(resource_root.as_str().unwrap_or_default()),
                    );
                    if let Some(resource) =
                        Self::create(product, resource_root_path.to_string_lossy().to_string())
                    {
                        resources.push(resource);
                    }
                }
                return resources;
            }
        }

        resources.push(Arc::new(Resource {
            product: Arc::clone(product),
            uri: Uri::file(default_resource_root.to_string_lossy().to_string()),
        }));

        resources
    }

    pub fn create(product: &Arc<Product>, resource_uri: String) -> Option<Arc<Resource>> {
        let uri = Uri::file(resource_uri);
        if fs::metadata(uri.fs_path())
            .map(|metadata| metadata.is_dir())
            .unwrap_or(false)
        {
            Some(Arc::new(Resource {
                product: Arc::clone(product),
                uri,
            }))
        } else {
            None
        }
    }

    pub fn get_product(&self) -> Arc<Product> {
        Arc::clone(&self.product)
    }

    pub fn get_uri(&self) -> Uri {
        self.uri.clone()
    }
}
