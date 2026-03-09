use crate::build_profile::{to_json_value, TargetConfig};
use crate::error::Result;
use crate::module::Module;
use crate::utils::path::resolve_within;
use crate::utils::uri::Uri;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct Product {
    module: Arc<Module>,
    name: String,
    target_config: TargetConfig,
}

impl Product {
    pub fn find_all(module: &Arc<Module>) -> Vec<Arc<Product>> {
        let mut products = Vec::new();

        for target_config in module.build_profile().targets().iter().cloned() {
            products.push(Arc::new(Product {
                module: Arc::clone(module),
                name: target_config.name().to_string(),
                target_config,
            }));
        }

        products
    }

    pub fn get_module(&self) -> Arc<Module> {
        Arc::clone(&self.module)
    }

    pub fn get_name(&self) -> String {
        self.name.clone()
    }

    pub fn get_current_target_config(&self) -> serde_json::Value {
        to_json_value(&self.target_config)
    }

    pub fn get_source_directories(&self) -> Result<Vec<Uri>> {
        let mut target_directories = Vec::new();
        let module_uri = self.module.get_uri();
        let default_child_path = self.default_child_path();
        let default_source_root = resolve_within(
            Path::new(&module_uri.fs_path()),
            &PathBuf::from("src").join(default_child_path),
        )?;

        if let Some(source_roots) = self.target_config.source_roots() {
            if source_roots.is_empty() {
                target_directories.push(Uri::file(&default_source_root)?);
            } else {
                for source_root in source_roots {
                    let source_root_path =
                        resolve_within(Path::new(&module_uri.fs_path()), Path::new(source_root))?;
                    target_directories.push(Uri::file(&source_root_path)?);
                }
            }
        } else {
            target_directories.push(Uri::file(&default_source_root)?);
        }

        Ok(target_directories)
    }

    pub fn get_current_target_directory(&self) -> Result<Uri> {
        let default_child_path = self.default_child_path();
        let target_directory = resolve_within(
            Path::new(&self.module.get_uri().fs_path()),
            &PathBuf::from("src").join(default_child_path),
        )?;
        Uri::file(&target_directory)
    }

    pub fn get_module_json5_path(&self) -> Result<Uri> {
        let target_directory = self.get_current_target_directory()?;
        let module_json5_path = Path::new(&target_directory.fs_path()).join("module.json5");
        Uri::file(&module_json5_path)
    }

    pub fn get_config_json_path(&self) -> Result<Uri> {
        let target_directory = self.get_current_target_directory()?;
        let config_json_path = Path::new(&target_directory.fs_path()).join("config.json");
        Uri::file(&config_json_path)
    }

    pub fn get_resource_directories(&self) -> Result<Vec<Uri>> {
        let mut target_directories = Vec::new();
        let module_uri = self.module.get_uri();
        let default_child_path = self.default_child_path();
        let default_resource_root = resolve_within(
            Path::new(&module_uri.fs_path()),
            &PathBuf::from("src")
                .join(default_child_path)
                .join("resources"),
        )?;

        if let Some(resource_roots) = self.target_config.resource_directories() {
            if resource_roots.is_empty() {
                target_directories.push(Uri::file(&default_resource_root)?);
            } else {
                for resource_root in resource_roots {
                    let resource_root_path =
                        resolve_within(Path::new(&module_uri.fs_path()), Path::new(resource_root))?;
                    target_directories.push(Uri::file(&resource_root_path)?);
                }
            }
        } else {
            target_directories.push(Uri::file(&default_resource_root)?);
        }

        Ok(target_directories)
    }
}

impl Product {
    fn default_child_path(&self) -> &str {
        if self.name == "default" {
            "main"
        } else {
            &self.name
        }
    }
}
