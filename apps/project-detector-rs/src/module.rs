use crate::build_profile::{load_module_build_profile, ModuleBuildProfile, ProjectModuleConfig};
use crate::error::Result;
use crate::project::Project;
use crate::utils::path::resolve_within;
use crate::utils::uri::Uri;
use std::path::{Path, PathBuf};

pub struct Module {
    name: String,
    uri: Uri,
    build_profile: ModuleBuildProfile,
    parsed_build_profile: serde_json::Value,
    build_profile_uri: Uri,
    build_profile_content: String,
}

impl Module {
    pub fn load(module_path: impl AsRef<Path>) -> Result<Module> {
        let module_path = module_path.as_ref().to_path_buf();
        let module_name = module_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string();
        Self::load_named(&module_path, module_name)
    }

    pub fn find_all(project: &Project) -> Result<Vec<Module>> {
        let mut modules = Vec::new();

        for module_config in project.build_profile().modules() {
            let module_path = Self::build_module_path(project, module_config)?;
            modules.push(Self::load_named(
                &module_path,
                module_config.name().to_string(),
            )?);
        }

        Ok(modules)
    }

    pub fn reload(&mut self) -> Result<()> {
        let module_path = self.uri.as_path().to_path_buf();
        let loaded_profile = load_module_build_profile(&module_path)?;

        self.build_profile = loaded_profile.profile;
        self.parsed_build_profile = loaded_profile.raw;
        self.build_profile_uri = loaded_profile.uri;
        self.build_profile_content = loaded_profile.content;
        Ok(())
    }

    pub fn uri(&self) -> &Uri {
        &self.uri
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn build_profile(&self) -> &ModuleBuildProfile {
        &self.build_profile
    }

    pub fn parsed_build_profile(&self) -> &serde_json::Value {
        &self.parsed_build_profile
    }

    pub fn build_profile_uri(&self) -> &Uri {
        &self.build_profile_uri
    }

    pub fn build_profile_content(&self) -> &str {
        &self.build_profile_content
    }
}

impl Module {
    fn load_named(module_path: &Path, module_name: String) -> Result<Module> {
        let uri = Uri::file(module_path)?;
        let loaded_profile = load_module_build_profile(module_path)?;

        Ok(Module {
            name: module_name,
            uri,
            build_profile: loaded_profile.profile,
            parsed_build_profile: loaded_profile.raw,
            build_profile_uri: loaded_profile.uri,
            build_profile_content: loaded_profile.content,
        })
    }

    fn build_module_path(
        project: &Project,
        module_config: &ProjectModuleConfig,
    ) -> Result<PathBuf> {
        resolve_within(project.uri().as_path(), Path::new(module_config.src_path()))
    }
}
