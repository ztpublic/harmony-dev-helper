use project_detector_rs::element_directory::ElementDirectory;
use project_detector_rs::error::{DetectorError, Result};
use project_detector_rs::files::element_json_file::ElementJsonFile;
use project_detector_rs::module::Module;
use project_detector_rs::product::Product;
use project_detector_rs::profile_directory::ProfileDirectory;
use project_detector_rs::project::Project;
use project_detector_rs::project_detector::ProjectDetector;
use project_detector_rs::rawfile_directory::RawfileDirectory;
use project_detector_rs::references::element_json_file_reference::ElementJsonFileReference;
use project_detector_rs::resource::Resource;
use project_detector_rs::resource_directory::ResourceDirectory;
use project_detector_rs::utils::uri::Uri;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::tempdir;

fn mock_root() -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/mock")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .to_string()
}

#[test]
fn detector_flow_matches_upstream_mock_projects() -> Result<()> {
    let mock_root = mock_root();
    let detector = Arc::new(ProjectDetector::create(Uri::file(&mock_root)?.to_string())?);
    assert_eq!(detector.get_workspace_folder().fs_path(), mock_root);

    let projects = Project::find_all(&detector)?;
    let harmony_project_1 = projects
        .iter()
        .find(|project| project.get_uri().to_string().contains("harmony-project-1"))
        .cloned()
        .unwrap();
    let harmony_project_2 = projects
        .iter()
        .find(|project| project.get_uri().to_string().contains("harmony-project-2"))
        .cloned()
        .unwrap();

    let harmony_project_1_modules = Module::find_all(&harmony_project_1)?;
    let harmony_project_2_modules = Module::find_all(&harmony_project_2)?;
    assert_eq!(harmony_project_1_modules.len(), 1);
    assert_eq!(harmony_project_2_modules.len(), 1);

    let harmony_project_1_module = Arc::clone(&harmony_project_1_modules[0]);
    let harmony_project_2_module = Arc::clone(&harmony_project_2_modules[0]);
    assert!(harmony_project_1_module
        .get_uri()
        .to_string()
        .contains("/entry"));
    assert!(harmony_project_2_module
        .get_uri()
        .to_string()
        .contains("/entry"));

    let harmony_project_1_products = Product::find_all(&harmony_project_1_module);
    let harmony_project_2_products = Product::find_all(&harmony_project_2_module);
    assert_eq!(harmony_project_1_products.len(), 2);
    assert_eq!(harmony_project_2_products.len(), 2);

    let harmony_project_1_main_product = harmony_project_1_products
        .iter()
        .find(|product| product.get_name() == "default")
        .cloned()
        .unwrap();
    let harmony_project_1_test_product = harmony_project_1_products
        .iter()
        .find(|product| product.get_name() == "ohosTest")
        .cloned()
        .unwrap();
    let harmony_project_2_main_product = harmony_project_2_products
        .iter()
        .find(|product| product.get_name() == "default")
        .cloned()
        .unwrap();
    let harmony_project_2_test_product = harmony_project_2_products
        .iter()
        .find(|product| product.get_name() == "ohosTest")
        .cloned()
        .unwrap();
    assert!(!harmony_project_2_main_product
        .get_source_directories()?
        .is_empty());
    assert!(!harmony_project_2_test_product
        .get_source_directories()?
        .is_empty());

    let harmony_project_1_resources = Resource::find_all(&harmony_project_1_main_product)?;
    assert_eq!(harmony_project_1_resources.len(), 1);
    let harmony_project_1_main_resource = Arc::clone(&harmony_project_1_resources[0]);

    let harmony_project_1_resource_directories =
        ResourceDirectory::find_all(&harmony_project_1_main_resource)?;
    let harmony_project_1_main_base_resource = harmony_project_1_resource_directories
        .iter()
        .find(|resource_directory| resource_directory.get_uri().to_string().contains("/base"))
        .cloned()
        .unwrap();
    let harmony_project_1_dark_resource = harmony_project_1_resource_directories
        .iter()
        .find(|resource_directory| resource_directory.get_uri().to_string().contains("/dark"))
        .cloned()
        .unwrap();
    assert_eq!(
        harmony_project_1_main_base_resource.get_qualifiers(),
        serde_json::Value::String("base".to_string())
    );
    assert!(harmony_project_1_dark_resource.get_qualifiers().is_array());

    let element_directory = ElementDirectory::from(&harmony_project_1_main_base_resource)?
        .expect("base resource should contain an element directory");
    let element_json_files = ElementJsonFile::find_all(&element_directory)?;
    let string_json_file = element_json_files
        .iter()
        .find(|element_json_file| {
            element_json_file
                .get_uri()
                .to_string()
                .contains("string.json")
        })
        .cloned()
        .unwrap();

    let references = ElementJsonFileReference::find_all(&string_json_file)?;
    assert!(!references.is_empty());
    for reference in references {
        assert!(reference.get_name_start() < reference.get_name_end());
        assert!(reference.get_value_start() < reference.get_value_end());
        assert_eq!(
            reference.get_name_full_text(),
            format!("\"{}\"", reference.get_name_text())
        );
        assert_eq!(
            reference.get_value_full_text(),
            format!("\"{}\"", reference.get_value_text())
        );
        assert_eq!(
            reference.to_ets_format(),
            format!(
                "app.{}.{}",
                reference.get_element_type(),
                reference.get_name_text()
            )
        );
        assert_eq!(
            reference.to_json_format(),
            format!(
                "${}:{}",
                reference.get_element_type(),
                reference.get_name_text()
            )
        );
    }

    let rawfile_directory = RawfileDirectory::from(&harmony_project_1_main_resource)?
        .expect("main resource should contain rawfile");
    let rawfiles = rawfile_directory.find_all()?;
    assert!(rawfiles
        .iter()
        .any(|uri| uri.to_string().contains("foo.txt")));

    let harmony_project_1_test_resources = Resource::find_all(&harmony_project_1_test_product)?;
    let harmony_project_1_test_resource = Arc::clone(&harmony_project_1_test_resources[0]);
    let harmony_project_1_test_directories =
        ResourceDirectory::find_all(&harmony_project_1_test_resource)?;
    let harmony_project_1_test_base = harmony_project_1_test_directories
        .iter()
        .find(|resource_directory| resource_directory.get_uri().to_string().contains("/base"))
        .cloned()
        .unwrap();
    let profile_directory = ProfileDirectory::from(&harmony_project_1_test_base)?
        .expect("ohosTest base resource should contain profile");
    let profile_files = profile_directory.find_all()?;
    assert!(profile_files
        .iter()
        .any(|uri| uri.to_string().contains("test_pages.json")));

    Ok(())
}

#[test]
fn project_detector_rejects_non_file_uri() {
    let error = match ProjectDetector::create("https://example.com/workspace".to_string()) {
        Ok(_) => panic!("expected non-file URI to be rejected"),
        Err(error) => error,
    };
    assert!(matches!(error, DetectorError::UnsupportedUriScheme { .. }));
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

    let detector = Arc::new(ProjectDetector::create(
        project_root.to_string_lossy().to_string(),
    )?);
    let project = Project::create(&detector, project_root.to_string_lossy().to_string())?.unwrap();
    let error = match Module::find_all(&project) {
        Ok(_) => panic!("expected escaping module path to be rejected"),
        Err(error) => error,
    };
    assert!(matches!(error, DetectorError::PathEscapesBase { .. }));
    Ok(())
}

#[test]
fn invalid_element_json_returns_a_parse_error() -> Result<()> {
    let element_json =
        ElementJsonFile::from_source("string.json".to_string(), "{ invalid json5".to_string())?;
    let error = element_json.parse().unwrap_err();
    assert!(matches!(error, DetectorError::Json5 { .. }));
    Ok(())
}
