use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::{DetectorError, Result};
use crate::utils::path::read_to_string;

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
    pub(crate) path: PathBuf,
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
        path,
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
        path,
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn write_build_profile(root: &Path, content: &str) -> PathBuf {
        std::fs::create_dir_all(root).unwrap();
        let path = root.join("build-profile.json5");
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn load_project_build_profile_parses_typed_modules() -> Result<()> {
        let temp_dir = tempdir().unwrap();
        let build_profile_path = write_build_profile(
            temp_dir.path(),
            r#"
            {
              app: {},
              modules: [
                { name: "entry", srcPath: "entry" }
              ]
            }
            "#,
        );

        let loaded = load_project_build_profile(temp_dir.path())?
            .expect("project fixture should be detected as a project profile");

        assert_eq!(loaded.path, build_profile_path);
        assert!(loaded.content.contains("modules"));
        assert_eq!(loaded.profile.modules().len(), 1);
        assert_eq!(loaded.profile.modules()[0].name(), "entry");
        assert_eq!(loaded.profile.modules()[0].src_path(), "entry");
        assert_eq!(loaded.raw["modules"][0]["srcPath"], "entry");
        Ok(())
    }

    #[test]
    fn load_project_build_profile_returns_none_for_module_profiles() -> Result<()> {
        let temp_dir = tempdir().unwrap();
        write_build_profile(
            temp_dir.path(),
            r#"
            {
              targets: [
                { name: "default" }
              ]
            }
            "#,
        );

        assert!(load_project_build_profile(temp_dir.path())?.is_none());
        Ok(())
    }

    #[test]
    fn load_project_build_profile_rejects_blank_module_fields() {
        let temp_dir = tempdir().unwrap();
        let build_profile_path = write_build_profile(
            temp_dir.path(),
            r#"
            {
              app: {},
              modules: [
                { name: "", srcPath: "entry" }
              ]
            }
            "#,
        );

        let error = match load_project_build_profile(temp_dir.path()) {
            Ok(_) => panic!("expected an invalid project build profile"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            DetectorError::InvalidProjectBuildProfile { path } if path == build_profile_path
        ));
    }

    #[test]
    fn load_module_build_profile_parses_typed_targets() -> Result<()> {
        let temp_dir = tempdir().unwrap();
        let build_profile_path = write_build_profile(
            temp_dir.path(),
            r#"
            {
              targets: [
                {
                  name: "default",
                  source: {
                    sourceRoots: ["src/custom"]
                  },
                  resource: {
                    directories: ["src/custom/resources"]
                  }
                },
                {
                  name: "ohosTest"
                }
              ]
            }
            "#,
        );

        let loaded = load_module_build_profile(temp_dir.path())?;
        let default_target = &loaded.profile.targets()[0];
        let default_source_roots = default_target
            .source_roots()
            .expect("default target should expose source roots");
        let default_resource_directories = default_target
            .resource_directories()
            .expect("default target should expose resource directories");
        let test_target = &loaded.profile.targets()[1];

        assert_eq!(loaded.path, build_profile_path);
        assert_eq!(loaded.profile.targets().len(), 2);
        assert_eq!(default_target.name(), "default");
        assert_eq!(default_source_roots, &[String::from("src/custom")]);
        assert_eq!(
            default_resource_directories,
            &[String::from("src/custom/resources")]
        );
        assert_eq!(test_target.name(), "ohosTest");
        assert!(test_target.source_roots().is_none());
        assert!(test_target.resource_directories().is_none());
        assert_eq!(loaded.raw["targets"][1]["name"], "ohosTest");
        Ok(())
    }

    #[test]
    fn load_module_build_profile_rejects_non_string_source_roots() {
        let temp_dir = tempdir().unwrap();
        let build_profile_path = write_build_profile(
            temp_dir.path(),
            r#"
            {
              targets: [
                {
                  name: "default",
                  source: {
                    sourceRoots: ["src/main", 1]
                  }
                }
              ]
            }
            "#,
        );

        let error = match load_module_build_profile(temp_dir.path()) {
            Ok(_) => panic!("expected an invalid module build profile"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            DetectorError::InvalidModuleBuildProfile { path } if path == build_profile_path
        ));
    }

    #[test]
    fn load_module_build_profile_rejects_non_string_resource_directories() {
        let temp_dir = tempdir().unwrap();
        let build_profile_path = write_build_profile(
            temp_dir.path(),
            r#"
            {
              targets: [
                {
                  name: "default",
                  resource: {
                    directories: ["src/main/resources", {}]
                  }
                }
              ]
            }
            "#,
        );

        let error = match load_module_build_profile(temp_dir.path()) {
            Ok(_) => panic!("expected an invalid module build profile"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            DetectorError::InvalidModuleBuildProfile { path } if path == build_profile_path
        ));
    }
}
