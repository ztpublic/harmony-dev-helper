use crate::error::{DetectorError, Result};
use crate::project::Project;
use crate::utils::path::{read_to_string, resolve_within};
use crate::utils::uri::Uri;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct Module {
    module_name: String,
    uri: Uri,
    project: Arc<Project>,
    parsed_build_profile: serde_json::Value,
    build_profile_uri: Uri,
    build_profile_content: String,
}

impl Module {
    pub fn create(project: &Arc<Project>, module_uri: String) -> Result<Arc<Module>> {
        let module_path = PathBuf::from(&module_uri);
        let uri = Uri::file(&module_path)?;
        let build_profile_path = module_path.join("build-profile.json5");
        let build_profile_uri = Uri::file(&build_profile_path)?;
        let build_profile_content = read_to_string(&build_profile_path)?;
        let parsed_build_profile: serde_json::Value = serde_json5::from_str(&build_profile_content)
            .map_err(|source| DetectorError::json5(build_profile_path.clone(), source))?;
        if !parsed_build_profile.is_object()
            || !parsed_build_profile
                .get("targets")
                .is_some_and(|targets| targets.is_array())
        {
            return Err(DetectorError::InvalidModuleBuildProfile {
                path: build_profile_path,
            });
        }

        Ok(Arc::new(Module {
            module_name: Self::extract_module_name(&parsed_build_profile),
            uri,
            project: Arc::clone(project),
            parsed_build_profile,
            build_profile_uri,
            build_profile_content,
        }))
    }

    pub fn find_all(project: &Arc<Project>) -> Result<Vec<Arc<Module>>> {
        let parsed_build_profile = project.get_parsed_build_profile();
        let mut modules = Vec::new();

        let modules_array = match parsed_build_profile
            .get("modules")
            .and_then(|modules| modules.as_array())
        {
            Some(array) => array,
            None => return Ok(modules),
        };

        for module_config in modules_array {
            let uri = Self::build_module_uri(project, module_config)?;
            modules.push(Self::create(project, uri.fs_path())?);
        }

        Ok(modules)
    }

    pub fn reload(&mut self) -> Result<()> {
        let module_uri = self.get_uri();
        let build_profile_path = Path::new(&module_uri.fs_path()).join("build-profile.json5");
        let build_profile_content = read_to_string(&build_profile_path)?;
        let parsed_build_profile: serde_json::Value = serde_json5::from_str(&build_profile_content)
            .map_err(|source| DetectorError::json5(build_profile_path.clone(), source))?;
        if !parsed_build_profile.is_object()
            || !parsed_build_profile
                .get("targets")
                .is_some_and(|targets| targets.is_array())
        {
            return Err(DetectorError::InvalidModuleBuildProfile {
                path: build_profile_path,
            });
        }

        self.update_parsed_build_profile(parsed_build_profile);
        self.update_build_profile_content(build_profile_content);
        Ok(())
    }

    pub fn get_uri(&self) -> Uri {
        self.uri.clone()
    }

    pub fn get_module_name(&self) -> String {
        self.module_name.clone()
    }

    pub fn get_project(&self) -> Arc<Project> {
        Arc::clone(&self.project)
    }

    pub fn get_parsed_build_profile(&self) -> serde_json::Value {
        self.parsed_build_profile.clone()
    }

    pub fn update_parsed_build_profile(&mut self, parsed_build_profile: serde_json::Value) {
        self.parsed_build_profile = parsed_build_profile;
    }

    pub fn get_build_profile_uri(&self) -> Uri {
        self.build_profile_uri.clone()
    }

    pub fn get_build_profile_content(&self) -> String {
        self.build_profile_content.clone()
    }

    pub fn update_build_profile_content(&mut self, build_profile_content: String) {
        self.build_profile_content = build_profile_content;
    }
}

impl Module {
    fn extract_module_name(module_config: &serde_json::Value) -> String {
        module_config
            .get("name")
            .and_then(|name| name.as_str())
            .unwrap_or("")
            .to_string()
    }

    fn build_module_uri(project: &Project, module_config: &serde_json::Value) -> Result<Uri> {
        let project_path = project.get_uri().fs_path();
        let src_path = module_config
            .get("srcPath")
            .and_then(|path| path.as_str())
            .ok_or_else(|| DetectorError::InvalidProjectBuildProfile {
                path: PathBuf::from(project.get_build_profile_uri().fs_path()),
            })?;
        let full_path = resolve_within(Path::new(&project_path), Path::new(src_path))?;

        Uri::file(&full_path)
    }
}
