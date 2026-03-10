use crate::build_profile::{load_module_build_profile, ModuleBuildProfile, ProjectModuleConfig};
use crate::error::Result;
use crate::project::Project;
use crate::utils::path::{absolute_path, resolve_within};
use crate::utils::uri::Uri;
use std::path::{Path, PathBuf};

pub struct Module {
    name: String,
    path: PathBuf,
    build_profile: ModuleBuildProfile,
    parsed_build_profile: serde_json::Value,
    build_profile_path: PathBuf,
    build_profile_content: String,
}

impl Module {
    pub fn load(module_path: impl AsRef<Path>) -> Result<Module> {
        let module_path = absolute_path(module_path.as_ref())?;
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
        let module_path = self.path.clone();
        let loaded_profile = load_module_build_profile(&module_path)?;

        self.build_profile = loaded_profile.profile;
        self.parsed_build_profile = loaded_profile.raw;
        self.build_profile_path = loaded_profile.path;
        self.build_profile_content = loaded_profile.content;
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn uri(&self) -> Uri {
        Uri::from_absolute_path(self.path.clone())
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

    pub fn build_profile_path(&self) -> &Path {
        &self.build_profile_path
    }

    pub fn build_profile_uri(&self) -> Uri {
        Uri::from_absolute_path(self.build_profile_path.clone())
    }

    pub fn build_profile_content(&self) -> &str {
        &self.build_profile_content
    }
}

impl Module {
    fn load_named(module_path: &Path, module_name: String) -> Result<Module> {
        let module_path = absolute_path(module_path)?;
        let loaded_profile = load_module_build_profile(&module_path)?;

        Ok(Module {
            name: module_name,
            path: module_path,
            build_profile: loaded_profile.profile,
            parsed_build_profile: loaded_profile.raw,
            build_profile_path: loaded_profile.path,
            build_profile_content: loaded_profile.content,
        })
    }

    fn build_module_path(
        project: &Project,
        module_config: &ProjectModuleConfig,
    ) -> Result<PathBuf> {
        resolve_within(project.path(), Path::new(module_config.src_path()))
    }
}
