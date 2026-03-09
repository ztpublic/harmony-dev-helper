use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::{DetectorError, Result};
use crate::utils::path::read_to_string;
use crate::utils::uri::Uri;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ProjectModuleConfig {
    name: String,
    #[serde(rename = "srcPath")]
    src_path: String,
}

impl ProjectModuleConfig {
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn src_path(&self) -> &str {
        &self.src_path
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ProjectBuildProfile {
    modules: Vec<ProjectModuleConfig>,
}

impl ProjectBuildProfile {
    pub(crate) fn modules(&self) -> &[ProjectModuleConfig] {
        &self.modules
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct SourceConfig {
    #[serde(default, rename = "sourceRoots")]
    source_roots: Vec<String>,
}

impl SourceConfig {
    pub(crate) fn roots(&self) -> &[String] {
        &self.source_roots
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct ResourceConfig {
    #[serde(default)]
    directories: Vec<String>,
}

impl ResourceConfig {
    pub(crate) fn directories(&self) -> &[String] {
        &self.directories
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct TargetConfig {
    name: String,
    #[serde(default)]
    source: Option<SourceConfig>,
    #[serde(default)]
    resource: Option<ResourceConfig>,
}

impl TargetConfig {
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn source_roots(&self) -> Option<&[String]> {
        self.source.as_ref().map(SourceConfig::roots)
    }

    pub(crate) fn resource_directories(&self) -> Option<&[String]> {
        self.resource.as_ref().map(ResourceConfig::directories)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ModuleBuildProfile {
    targets: Vec<TargetConfig>,
}

impl ModuleBuildProfile {
    pub(crate) fn targets(&self) -> &[TargetConfig] {
        &self.targets
    }
}

#[derive(Clone)]
pub(crate) struct LoadedBuildProfile<T> {
    pub(crate) uri: Uri,
    pub(crate) content: String,
    pub(crate) raw: serde_json::Value,
    pub(crate) profile: T,
}

pub(crate) fn load_project_build_profile(
    root: &Path,
) -> Result<Option<LoadedBuildProfile<ProjectBuildProfile>>> {
    let path = build_profile_path(root);
    let (content, raw) = read_build_profile_value(&path)?;
    if !looks_like_project_build_profile(&raw) {
        return Ok(None);
    }

    let profile = serde_json::from_value::<ProjectBuildProfile>(raw.clone())
        .map_err(|_| DetectorError::InvalidProjectBuildProfile { path: path.clone() })?;
    validate_project_build_profile(&profile, &path)?;

    Ok(Some(LoadedBuildProfile {
        uri: Uri::file(&path)?,
        content,
        raw,
        profile,
    }))
}

pub(crate) fn load_module_build_profile(
    root: &Path,
) -> Result<LoadedBuildProfile<ModuleBuildProfile>> {
    let path = build_profile_path(root);
    let (content, raw) = read_build_profile_value(&path)?;
    if !looks_like_module_build_profile(&raw) {
        return Err(DetectorError::InvalidModuleBuildProfile { path });
    }

    let profile = serde_json::from_value::<ModuleBuildProfile>(raw.clone())
        .map_err(|_| DetectorError::InvalidModuleBuildProfile { path: path.clone() })?;
    validate_module_build_profile(&profile, &path)?;

    Ok(LoadedBuildProfile {
        uri: Uri::file(&path)?,
        content,
        raw,
        profile,
    })
}

fn build_profile_path(root: &Path) -> PathBuf {
    root.join("build-profile.json5")
}

fn read_build_profile_value(path: &Path) -> Result<(String, serde_json::Value)> {
    let content = read_to_string(path)?;
    let raw =
        serde_json5::from_str(&content).map_err(|source| DetectorError::json5(path, source))?;
    Ok((content, raw))
}

fn looks_like_project_build_profile(raw: &serde_json::Value) -> bool {
    raw.as_object().is_some_and(|object| {
        object.get("app").is_some_and(serde_json::Value::is_object)
            && object
                .get("modules")
                .is_some_and(serde_json::Value::is_array)
    })
}

fn looks_like_module_build_profile(raw: &serde_json::Value) -> bool {
    raw.as_object().is_some_and(|object| {
        object
            .get("targets")
            .is_some_and(serde_json::Value::is_array)
    })
}

fn validate_project_build_profile(profile: &ProjectBuildProfile, path: &Path) -> Result<()> {
    for module in profile.modules() {
        if module.name().trim().is_empty() || module.src_path().trim().is_empty() {
            return Err(DetectorError::InvalidProjectBuildProfile {
                path: path.to_path_buf(),
            });
        }
    }

    Ok(())
}

fn validate_module_build_profile(profile: &ModuleBuildProfile, path: &Path) -> Result<()> {
    for target in profile.targets() {
        if target.name().trim().is_empty() {
            return Err(DetectorError::InvalidModuleBuildProfile {
                path: path.to_path_buf(),
            });
        }

        if target
            .source_roots()
            .is_some_and(|roots| roots.iter().any(|root| root.trim().is_empty()))
        {
            return Err(DetectorError::InvalidModuleBuildProfile {
                path: path.to_path_buf(),
            });
        }

        if target.resource_directories().is_some_and(|directories| {
            directories
                .iter()
                .any(|directory| directory.trim().is_empty())
        }) {
            return Err(DetectorError::InvalidModuleBuildProfile {
                path: path.to_path_buf(),
            });
        }
    }

    Ok(())
}
