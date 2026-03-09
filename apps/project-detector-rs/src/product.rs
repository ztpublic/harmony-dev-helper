use crate::build_profile::TargetConfig;
use crate::error::Result;
use crate::module::Module;
use crate::utils::path::resolve_within;
use crate::utils::uri::Uri;
use std::path::{Path, PathBuf};

pub struct Product {
    module_uri: Uri,
    name: String,
    target_config: TargetConfig,
}

impl Product {
    pub fn find_all(module: &Module) -> Vec<Product> {
        let mut products = Vec::new();

        for target_config in module.build_profile().targets().iter().cloned() {
            products.push(Product {
                module_uri: module.uri().clone(),
                name: target_config.name().to_string(),
                target_config,
            });
        }

        products
    }

    pub fn module_uri(&self) -> &Uri {
        &self.module_uri
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn source_directories(&self) -> Result<Vec<Uri>> {
        let mut target_directories = Vec::new();
        let default_child_path = self.default_child_path();
        let default_source_root = resolve_within(
            self.module_uri.as_path(),
            &PathBuf::from("src").join(default_child_path),
        )?;

        if let Some(source_roots) = self.target_config.source_roots() {
            if source_roots.is_empty() {
                target_directories.push(Uri::file(&default_source_root)?);
            } else {
                for source_root in source_roots {
                    let source_root_path =
                        resolve_within(self.module_uri.as_path(), Path::new(source_root))?;
                    target_directories.push(Uri::file(&source_root_path)?);
                }
            }
        } else {
            target_directories.push(Uri::file(&default_source_root)?);
        }

        Ok(target_directories)
    }

    pub fn target_directory(&self) -> Result<Uri> {
        let default_child_path = self.default_child_path();
        let target_directory = resolve_within(
            self.module_uri.as_path(),
            &PathBuf::from("src").join(default_child_path),
        )?;
        Uri::file(&target_directory)
    }

    pub fn module_json5_path(&self) -> Result<Uri> {
        let target_directory = self.target_directory()?;
        let module_json5_path = target_directory.as_path().join("module.json5");
        Uri::file(&module_json5_path)
    }

    pub fn config_json_path(&self) -> Result<Uri> {
        let target_directory = self.target_directory()?;
        let config_json_path = target_directory.as_path().join("config.json");
        Uri::file(&config_json_path)
    }

    pub fn resource_directories(&self) -> Result<Vec<Uri>> {
        let mut target_directories = Vec::new();
        let default_child_path = self.default_child_path();
        let default_resource_root = resolve_within(
            self.module_uri.as_path(),
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
                        resolve_within(self.module_uri.as_path(), Path::new(resource_root))?;
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
