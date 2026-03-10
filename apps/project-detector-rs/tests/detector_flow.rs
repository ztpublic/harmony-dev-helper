use project_detector_rs::{
    DetectorError, ElementDirectory, ElementJsonFile, ElementJsonFileReference, Module, Product,
    ProfileDirectory, Project, ProjectDetector, RawfileDirectory, Resource, ResourceDirectory,
    Result, Uri,
};
use std::path::{Path, PathBuf};
use tempfile::tempdir;

fn mock_root() -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/mock")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .to_string()
}

fn write_project_fixture(
    project_root: &Path,
    project_build_profile: &str,
    module_build_profile: &str,
) {
    let module_root = project_root.join("entry");
    std::fs::create_dir_all(&module_root).unwrap();
    std::fs::write(
        project_root.join("build-profile.json5"),
        project_build_profile,
    )
    .unwrap();
    std::fs::write(
        module_root.join("build-profile.json5"),
        module_build_profile,
    )
    .unwrap();
}

fn load_project(project_root: &Path) -> Result<Project> {
    Project::load(project_root)?.ok_or_else(|| DetectorError::InvalidProjectBuildProfile {
        path: project_root.join("build-profile.json5"),
    })
}

#[test]
fn detector_flow_matches_upstream_mock_projects() -> Result<()> {
    let mock_root = mock_root();
    let detector = ProjectDetector::new(&mock_root)?;
    assert_eq!(detector.workspace_path(), Path::new(&mock_root));

    let projects = Project::find_all(&detector)?;
    let harmony_project_1 = projects
        .iter()
        .find(|project| {
            project
                .path()
                .to_string_lossy()
                .contains("harmony-project-1")
        })
        .unwrap();
    let harmony_project_2 = projects
        .iter()
        .find(|project| {
            project
                .path()
                .to_string_lossy()
                .contains("harmony-project-2")
        })
        .unwrap();

    let harmony_project_1_modules = Module::find_all(harmony_project_1)?;
    let harmony_project_2_modules = Module::find_all(harmony_project_2)?;
    assert_eq!(harmony_project_1_modules.len(), 1);
    assert_eq!(harmony_project_2_modules.len(), 1);

    let harmony_project_1_module = &harmony_project_1_modules[0];
    let harmony_project_2_module = &harmony_project_2_modules[0];
    assert!(harmony_project_1_module
        .path()
        .to_string_lossy()
        .contains("/entry"));
    assert!(harmony_project_2_module
        .path()
        .to_string_lossy()
        .contains("/entry"));

    let harmony_project_1_products = Product::find_all(harmony_project_1_module);
    let harmony_project_2_products = Product::find_all(harmony_project_2_module);
    assert_eq!(harmony_project_1_products.len(), 2);
    assert_eq!(harmony_project_2_products.len(), 2);

    let harmony_project_1_main_product = harmony_project_1_products
        .iter()
        .find(|product| product.name() == "default")
        .unwrap();
    let harmony_project_1_test_product = harmony_project_1_products
        .iter()
        .find(|product| product.name() == "ohosTest")
        .unwrap();
    let harmony_project_2_main_product = harmony_project_2_products
        .iter()
        .find(|product| product.name() == "default")
        .unwrap();
    let harmony_project_2_test_product = harmony_project_2_products
        .iter()
        .find(|product| product.name() == "ohosTest")
        .unwrap();
    assert!(!harmony_project_2_main_product
        .source_directories()?
        .is_empty());
    assert!(!harmony_project_2_test_product
        .source_directories()?
        .is_empty());

    let harmony_project_1_resources = Resource::find_all(harmony_project_1_main_product)?;
    assert_eq!(harmony_project_1_resources.len(), 1);
    let harmony_project_1_main_resource = &harmony_project_1_resources[0];

    let harmony_project_1_resource_directories =
        ResourceDirectory::find_all(harmony_project_1_main_resource)?;
    let harmony_project_1_main_base_resource = harmony_project_1_resource_directories
        .iter()
        .find(|resource_directory| {
            resource_directory
                .path()
                .to_string_lossy()
                .contains("/base")
        })
        .unwrap();
    let harmony_project_1_dark_resource = harmony_project_1_resource_directories
        .iter()
        .find(|resource_directory| {
            resource_directory
                .path()
                .to_string_lossy()
                .contains("/dark")
        })
        .unwrap();
    assert_eq!(
        harmony_project_1_main_base_resource.qualifiers(),
        serde_json::Value::String("base".to_string())
    );
    assert!(harmony_project_1_dark_resource.qualifiers().is_array());

    let element_directory = ElementDirectory::locate(harmony_project_1_main_base_resource)?
        .expect("base resource should contain an element directory");
    let element_json_files = ElementJsonFile::find_all(&element_directory)?;
    let string_json_file = element_json_files
        .iter()
        .find(|element_json_file| {
            element_json_file
                .path()
                .to_string_lossy()
                .contains("string.json")
        })
        .unwrap();

    let references = ElementJsonFileReference::find_all(string_json_file)?;
    assert!(!references.is_empty());
    for reference in references {
        assert!(reference.name_start() < reference.name_end());
        assert!(reference.value_start() < reference.value_end());
        assert_eq!(
            reference.name_full_text(),
            format!("\"{}\"", reference.name_text())
        );
        assert_eq!(
            reference.value_full_text(),
            format!("\"{}\"", reference.value_text())
        );
        assert_eq!(
            reference.to_ets_format(),
            format!("app.{}.{}", reference.element_type(), reference.name_text())
        );
        assert_eq!(
            reference.to_json_format(),
            format!("${}:{}", reference.element_type(), reference.name_text())
        );
    }

    let rawfile_directory = RawfileDirectory::locate(harmony_project_1_main_resource)?
        .expect("main resource should contain rawfile");
    let rawfiles = rawfile_directory.find_all()?;
    assert!(rawfiles
        .iter()
        .any(|path| path.to_string_lossy().contains("foo.txt")));

    let harmony_project_1_test_resources = Resource::find_all(harmony_project_1_test_product)?;
    let harmony_project_1_test_resource = &harmony_project_1_test_resources[0];
    let harmony_project_1_test_directories =
        ResourceDirectory::find_all(harmony_project_1_test_resource)?;
    let harmony_project_1_test_base = harmony_project_1_test_directories
        .iter()
        .find(|resource_directory| {
            resource_directory
                .path()
                .to_string_lossy()
                .contains("/base")
        })
        .unwrap();
    let profile_directory = ProfileDirectory::locate(harmony_project_1_test_base)?
        .expect("ohosTest base resource should contain profile");
    let profile_files = profile_directory.find_all()?;
    assert!(profile_files
        .iter()
        .any(|path| path.to_string_lossy().contains("test_pages.json")));

    Ok(())
}

#[test]
fn project_detector_rejects_non_file_uri() {
    let error = match ProjectDetector::from_uri("https://example.com/workspace") {
        Ok(_) => panic!("expected non-file URI to be rejected"),
        Err(error) => error,
    };
    assert!(matches!(error, DetectorError::UnsupportedUriScheme { .. }));
}

#[test]
fn project_detector_normalizes_relative_workspace_paths() -> Result<()> {
    let detector = ProjectDetector::new(".")?;
    assert_eq!(
        detector.workspace_path(),
        std::env::current_dir().unwrap().as_path()
    );
    Ok(())
}

#[test]
fn project_detector_accepts_file_uris() -> Result<()> {
    let current_dir_uri = Uri::file(".")?.to_string();
    let detector = ProjectDetector::from_uri(&current_dir_uri)?;
    assert_eq!(
        detector.workspace_path(),
        std::env::current_dir().unwrap().as_path()
    );
    Ok(())
}

#[test]
fn module_find_all_rejects_paths_that_escape_the_project_root() -> Result<()> {
    let temp_dir = tempdir().unwrap();
    let project_root = temp_dir.path().join("workspace");
    std::fs::create_dir_all(&project_root).unwrap();
    std::fs::write(
        project_root.join("build-profile.json5"),
        r#"
        {
          "app": {},
          "modules": [
            { "name": "entry", "srcPath": "../outside" }
          ]
        }
        "#,
    )
    .unwrap();

    let project = Project::load(&project_root)?.unwrap();
    let error = match Module::find_all(&project) {
        Ok(_) => panic!("expected escaping module path to be rejected"),
        Err(error) => error,
    };
    assert!(matches!(error, DetectorError::PathEscapesBase { .. }));
    Ok(())
}

#[test]
fn invalid_element_json_returns_a_parse_error() -> Result<()> {
    let element_json = ElementJsonFile::from_source("string.json", "{ invalid json5")?;
    let error = element_json.parse().unwrap_err();
    assert!(matches!(error, DetectorError::Json5 { .. }));
    Ok(())
}

#[test]
fn module_find_all_rejects_blank_source_roots() -> Result<()> {
    let temp_dir = tempdir().unwrap();
    let project_root = temp_dir.path().join("workspace");
    write_project_fixture(
        &project_root,
        r#"
        {
          "app": {},
          "modules": [
            { "name": "entry", "srcPath": "entry" }
          ]
        }
        "#,
        r#"
        {
          "targets": [
            {
              "name": "default",
              "source": {
                "sourceRoots": [""]
              }
            }
          ]
        }
        "#,
    );

    let project = load_project(&project_root)?;
    let error = match Module::find_all(&project) {
        Ok(_) => panic!("expected blank source root to be rejected"),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        DetectorError::InvalidModuleBuildProfile { .. }
    ));
    Ok(())
}

#[test]
fn module_find_all_rejects_blank_resource_directories() -> Result<()> {
    let temp_dir = tempdir().unwrap();
    let project_root = temp_dir.path().join("workspace");
    write_project_fixture(
        &project_root,
        r#"
        {
          "app": {},
          "modules": [
            { "name": "entry", "srcPath": "entry" }
          ]
        }
        "#,
        r#"
        {
          "targets": [
            {
              "name": "default",
              "resource": {
                "directories": [""]
              }
            }
          ]
        }
        "#,
    );

    let project = load_project(&project_root)?;
    let error = match Module::find_all(&project) {
        Ok(_) => panic!("expected blank resource directory to be rejected"),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        DetectorError::InvalidModuleBuildProfile { .. }
    ));
    Ok(())
}

#[test]
fn project_create_rejects_blank_module_src_path() -> Result<()> {
    let temp_dir = tempdir().unwrap();
    let project_root = temp_dir.path().join("workspace");
    write_project_fixture(
        &project_root,
        r#"
        {
          "app": {},
          "modules": [
            { "name": "entry", "srcPath": "" }
          ]
        }
        "#,
        r#"
        {
          "targets": [
            {
              "name": "default"
            }
          ]
        }
        "#,
    );

    let error = match load_project(&project_root) {
        Ok(_) => panic!("expected blank module srcPath to be rejected"),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        DetectorError::InvalidProjectBuildProfile { .. }
    ));
    Ok(())
}
