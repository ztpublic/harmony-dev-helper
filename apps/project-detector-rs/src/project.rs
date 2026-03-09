use crate::build_profile::{load_project_build_profile, ProjectBuildProfile};
use crate::error::{DetectorError, Result};
use crate::project_detector::ProjectDetector;
use crate::utils::path::path_is_dir;
use crate::utils::uri::Uri;
use std::path::PathBuf;
use std::sync::Arc;
use walkdir::WalkDir;

pub struct Project {
    project_detector: Arc<ProjectDetector>,
    uri: Uri,
    build_profile: ProjectBuildProfile,
    parsed_build_profile: serde_json::Value,
    build_profile_uri: Uri,
    build_profile_content: String,
}

impl Project {
    pub fn get_project_detector(&self) -> Arc<ProjectDetector> {
        Arc::clone(&self.project_detector)
    }

    pub fn is_in_exclude_dirs(entry: &walkdir::DirEntry) -> bool {
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

    pub fn find_all(project_detector: &Arc<ProjectDetector>) -> Result<Vec<Arc<Project>>> {
        let mut projects = Vec::new();
        let workspace_folder = PathBuf::from(project_detector.get_workspace_folder().fs_path());

        for entry in WalkDir::new(&workspace_folder)
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
                if let Some(project) =
                    Self::create(project_detector, project_dir.to_string_lossy().to_string())?
                {
                    projects.push(project);
                }
            }
        }

        Ok(projects)
    }

    pub fn create(
        project_detector: &Arc<ProjectDetector>,
        project_uri: String,
    ) -> Result<Option<Arc<Project>>> {
        let project_path = PathBuf::from(&project_uri);
        if !path_is_dir(&project_path)? {
            return Ok(None);
        }

        let uri = Uri::file(&project_path)?;
        let Some(loaded_profile) = load_project_build_profile(&project_path)? else {
            return Ok(None);
        };

        Ok(Some(Arc::new(Project {
            project_detector: Arc::clone(project_detector),
            uri,
            build_profile: loaded_profile.profile,
            parsed_build_profile: loaded_profile.raw,
            build_profile_uri: loaded_profile.uri,
            build_profile_content: loaded_profile.content,
        })))
    }

    pub fn reload(&mut self) -> Result<()> {
        let project_path = PathBuf::from(self.uri.fs_path());
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

    pub fn get_uri(&self) -> Uri {
        self.uri.clone()
    }

    pub(crate) fn build_profile(&self) -> &ProjectBuildProfile {
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
