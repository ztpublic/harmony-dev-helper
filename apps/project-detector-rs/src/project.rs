use crate::build_profile::{load_project_build_profile, ProjectBuildProfile};
use crate::error::{DetectorError, Result};
use crate::project_detector::ProjectDetector;
use crate::utils::path::path_is_dir;
use crate::utils::uri::Uri;
use std::path::Path;
use walkdir::WalkDir;

pub struct Project {
    uri: Uri,
    build_profile: ProjectBuildProfile,
    parsed_build_profile: serde_json::Value,
    build_profile_uri: Uri,
    build_profile_content: String,
}

impl Project {
    fn is_in_exclude_dirs(entry: &walkdir::DirEntry) -> bool {
        entry.path().iter().any(|component| {
            if let Some(component_str) = component.to_str() {
                component_str == "node_modules"
                    || component_str == "oh_modules"
                    || component_str.starts_with('.')
            } else {
                false
            }
        })
    }

    pub fn find_all(project_detector: &ProjectDetector) -> Result<Vec<Project>> {
        let mut projects = Vec::new();
        let workspace_folder = project_detector.workspace_folder().as_path();

        for entry in WalkDir::new(workspace_folder)
            .into_iter()
            .filter_entry(|entry| !Self::is_in_exclude_dirs(entry))
        {
            let entry = entry.map_err(DetectorError::walkdir)?;
            let path = entry.path();
            if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name == "build-profile.json5")
                && entry.file_type().is_file()
            {
                let project_dir = path
                    .parent()
                    .ok_or_else(|| DetectorError::InvalidFilePath {
                        path: path.to_string_lossy().to_string(),
                    })?;
                if let Some(project) = Self::load(project_dir)? {
                    projects.push(project);
                }
            }
        }

        Ok(projects)
    }

    pub fn load(project_path: impl AsRef<Path>) -> Result<Option<Project>> {
        let project_path = project_path.as_ref().to_path_buf();
        if !path_is_dir(&project_path)? {
            return Ok(None);
        }

        let uri = Uri::file(&project_path)?;
        let Some(loaded_profile) = load_project_build_profile(&project_path)? else {
            return Ok(None);
        };

        Ok(Some(Project {
            uri,
            build_profile: loaded_profile.profile,
            parsed_build_profile: loaded_profile.raw,
            build_profile_uri: loaded_profile.uri,
            build_profile_content: loaded_profile.content,
        }))
    }

    pub fn reload(&mut self) -> Result<()> {
        let project_path = self.uri.as_path().to_path_buf();
        let loaded_profile = load_project_build_profile(&project_path)?.ok_or_else(|| {
            DetectorError::InvalidProjectBuildProfile {
                path: project_path.join("build-profile.json5"),
            }
        })?;

        self.build_profile = loaded_profile.profile;
        self.parsed_build_profile = loaded_profile.raw;
        self.build_profile_uri = loaded_profile.uri;
        self.build_profile_content = loaded_profile.content;
        Ok(())
    }

    pub fn uri(&self) -> &Uri {
        &self.uri
    }

    pub(crate) fn build_profile(&self) -> &ProjectBuildProfile {
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
