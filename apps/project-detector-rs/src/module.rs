use crate::build_profile::{load_module_build_profile, ModuleBuildProfile, ProjectModuleConfig};
use crate::error::Result;
use crate::project::Project;
use crate::utils::path::resolve_within;
use crate::utils::uri::Uri;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct Module {
    module_name: String,
    uri: Uri,
    project: Arc<Project>,
    build_profile: ModuleBuildProfile,
    parsed_build_profile: serde_json::Value,
    build_profile_uri: Uri,
    build_profile_content: String,
}

impl Module {
    pub fn create(project: &Arc<Project>, module_uri: String) -> Result<Arc<Module>> {
        let module_path = PathBuf::from(&module_uri);
        let module_name = module_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string();
        Self::create_with_name(project, &module_path, module_name)
    }

    pub fn find_all(project: &Arc<Project>) -> Result<Vec<Arc<Module>>> {
        let mut modules = Vec::new();

        for module_config in project.build_profile().modules() {
            let uri = Self::build_module_uri(project, module_config)?;
            modules.push(Self::create_with_name(
                project,
                Path::new(&uri.fs_path()),
                module_config.name().to_string(),
            )?);
        }

        Ok(modules)
    }

    pub fn reload(&mut self) -> Result<()> {
        let module_path = PathBuf::from(self.uri.fs_path());
        let loaded_profile = load_module_build_profile(&module_path)?;

        self.build_profile = loaded_profile.profile;
        self.parsed_build_profile = loaded_profile.raw;
        self.build_profile_uri = loaded_profile.uri;
        self.build_profile_content = loaded_profile.content;
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

    pub(crate) fn build_profile(&self) -> &ModuleBuildProfile {
        &self.build_profile
    }

    pub fn get_parsed_build_profile(&self) -> serde_json::Value {
        self.parsed_build_profile.clone()
    }

    pub fn get_build_profile_uri(&self) -> Uri {
        self.build_profile_uri.clone()
    }

    pub fn get_build_profile_content(&self) -> String {
        self.build_profile_content.clone()
    }
}

impl Module {
    fn create_with_name(
        project: &Arc<Project>,
        module_path: &Path,
        module_name: String,
    ) -> Result<Arc<Module>> {
        let uri = Uri::file(module_path)?;
        let loaded_profile = load_module_build_profile(module_path)?;

        Ok(Arc::new(Module {
            module_name,
            uri,
            project: Arc::clone(project),
            build_profile: loaded_profile.profile,
            parsed_build_profile: loaded_profile.raw,
            build_profile_uri: loaded_profile.uri,
            build_profile_content: loaded_profile.content,
        }))
    }

    fn build_module_uri(project: &Project, module_config: &ProjectModuleConfig) -> Result<Uri> {
        let project_path = project.get_uri().fs_path();
        let full_path = resolve_within(
            Path::new(&project_path),
            Path::new(module_config.src_path()),
        )?;

        Uri::file(&full_path)
    }
}
