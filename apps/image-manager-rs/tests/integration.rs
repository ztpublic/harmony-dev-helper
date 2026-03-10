use std::io::{Cursor, Write};
use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::State;
use axum::http::header::{CONTENT_LENGTH, CONTENT_RANGE, RANGE};
use axum::http::{HeaderMap, Response, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use image_manager_rs::{
    DeviceSpec, EmulatorDeviceType, ImageManager, ImageManagerOptions, ProductDeviceType,
    ProgressKind, ScreenPreset,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use tempfile::TempDir;

#[tokio::test]
async fn local_image_device_lifecycle_and_cleanup() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = FixturePaths::new()?;
    fixture
        .create_local_image(
            "system-image/HarmonyOS-6.0.1/foldable_arm",
            "System-image-foldable",
            "21",
            "6.0.1",
            "6.0.0.112",
            "HarmonyOS 6.0.1(21)",
            "arm",
        )
        .await?;
    fixture.write_emulator_sdk_version("6.0.2.10").await?;

    let manager = fixture.manager(None)?;

    let local_images = manager.local_images().await?;
    assert_eq!(local_images.len(), 1);
    assert!(manager.is_compatible().await?);

    let product_catalog = manager.read_product_catalog().await?;
    let product = product_catalog
        .find_item(Some(&ProductDeviceType::Foldable), Some("Mate X5"))
        .cloned()
        .expect("default product config should contain Mate X5");
    let emulator_catalog = manager.read_emulator_catalog().await?;
    let emulator = emulator_catalog
        .find_device(Some(21), Some(&EmulatorDeviceType::Foldable))
        .cloned()
        .expect("default emulator config should contain foldable api 21");

    let device = local_images[0]
        .create_device(DeviceSpec::new(
            "test-device",
            4,
            8192,
            6144,
            ScreenPreset::new(emulator, product),
        ))
        .await?;

    let config_ini = tokio::fs::read_to_string(device.config_ini().path()).await?;
    assert!(config_ini.contains("name=test-device"));
    assert!(config_ini.contains("deviceType=foldable"));
    assert!(config_ini.contains("productModel=Mate X5"));
    assert!(config_ini.contains("hw.lcd.number=2"));
    assert!(config_ini.contains(&format!("sdkPath={}", fixture.sdk_root().display())));

    let named_ini = tokio::fs::read_to_string(device.named_ini().path()).await?;
    assert!(named_ini.contains("hvd.ini.encoding=UTF-8"));
    assert!(named_ini.contains(&format!(
        "path={}",
        fixture.deployed_dir().join("test-device").display()
    )));

    let lists = manager.read_lists().await?;
    assert_eq!(lists.entries().len(), 1);
    assert_eq!(lists.entries()[0].name, "test-device");

    let deployed_devices = manager.deployed_devices().await?;
    assert_eq!(deployed_devices.len(), 1);
    assert_eq!(deployed_devices[0].lists_entry().name, "test-device");
    assert!(deployed_devices[0].storage_size().await? > 0);

    device.delete().await?;

    let lists = manager.read_lists().await?;
    assert!(lists.entries().is_empty());
    assert!(!fixture.deployed_dir().join("test-device").exists());
    assert!(!fixture.deployed_dir().join("test-device.ini").exists());

    Ok(())
}

#[tokio::test]
async fn remote_images_and_downloader_round_trip() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = FixturePaths::new()?;
    let archive_bytes = build_image_archive(
        "System-image-foldable",
        "system-image,HarmonyOS-6.0.1,foldable_arm",
        "21",
        "6.0.1",
        "6.0.0.112",
        "HarmonyOS 6.0.1(21)",
        "arm",
    )?;
    let checksum = hex::encode(Sha256::digest(&archive_bytes));
    let sdk_list = json!([
        {
            "path": "system-image,HarmonyOS-6.0.1,foldable_arm",
            "apiVersion": "21",
            "license": "HarmonyOS-SDK",
            "version": "6.0.0.112",
            "displayName": "System-image-foldable",
            "description": "HarmonyOS emulator image for foldable",
            "experimentalFlag": "0",
            "releaseType": "Release",
            "metaVersion": "1.0.0",
            "archive": {
                "complete": {
                    "size": archive_bytes.len().to_string(),
                    "checksum": checksum,
                    "osArch": "arm64"
                }
            },
            "stage": "Release",
            "platformVersion": "0.0.0"
        }
    ]);
    let server = MockServer::start(sdk_list, archive_bytes.clone()).await?;
    let manager = fixture.manager(Some(server.base_url.clone()))?;

    let remote_images = manager.remote_images(None).await?;
    assert_eq!(remote_images.len(), 1);
    let downloader = remote_images[0].create_downloader().await?;
    tokio::fs::create_dir_all(fixture.cache_dir()).await?;
    tokio::fs::write(downloader.cache_path(), &archive_bytes[..128]).await?;

    let mut download_events = Vec::new();
    downloader
        .download(|event| download_events.push(event))
        .await?;
    assert!(download_events
        .iter()
        .any(|event| event.kind == ProgressKind::Download && event.update.reset));

    let mut checksum_events = Vec::new();
    assert!(
        downloader
            .verify_checksum(|event| checksum_events.push(event))
            .await?
    );
    assert!(checksum_events
        .iter()
        .any(|event| event.kind == ProgressKind::Checksum));

    let mut extract_events = Vec::new();
    downloader
        .extract(|event| extract_events.push(event))
        .await?;
    assert!(extract_events
        .iter()
        .any(|event| event.kind == ProgressKind::Extract));

    let local_images = manager.local_images().await?;
    assert_eq!(local_images.len(), 1);

    let downloaded_remote_images = manager.downloaded_remote_images(None).await?;
    assert_eq!(downloaded_remote_images.len(), 1);
    assert!(remote_images[0].local_image().await?.is_some());

    server.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn remote_images_parse_external_fixture_response() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = FixturePaths::new()?;
    let sdk_list: serde_json::Value = serde_json::from_str(include_str!(
        "../../../external-src/image-manager-main/test/image-list-response.json"
    ))?;
    let server = MockServer::start(sdk_list, Vec::new()).await?;
    let manager = fixture.manager(Some(server.base_url.clone()))?;

    let remote_images = manager.remote_images(None).await?;
    assert!(remote_images.len() > 5);
    assert_eq!(remote_images[0].sdk().display_name, "System-image-pc_all");
    assert_eq!(remote_images[0].sdk().version, "6.0.0.112");

    server.shutdown().await;
    Ok(())
}

struct FixturePaths {
    _tempdir: TempDir,
    root: PathBuf,
}

impl FixturePaths {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let tempdir = TempDir::new()?;
        let root = tempdir.path().to_path_buf();
        std::fs::create_dir_all(root.join("images"))?;
        std::fs::create_dir_all(root.join("deployed"))?;
        std::fs::create_dir_all(root.join("cache"))?;
        std::fs::create_dir_all(root.join("sdk").join("default").join("openharmony"))?;
        std::fs::create_dir_all(root.join("config"))?;
        std::fs::create_dir_all(root.join("log"))?;
        std::fs::create_dir_all(root.join("emulator"))?;
        Ok(Self {
            _tempdir: tempdir,
            root,
        })
    }

    fn manager(
        &self,
        service_base_url: Option<String>,
    ) -> Result<ImageManager, Box<dyn std::error::Error>> {
        Ok(ImageManager::new(ImageManagerOptions {
            image_base_path: Some(self.images_dir()),
            deployed_path: Some(self.deployed_dir()),
            cache_path: Some(self.cache_dir()),
            sdk_path: Some(self.sdk_openharmony_dir()),
            config_path: Some(self.root.join("config")),
            log_path: Some(self.root.join("log")),
            emulator_path: Some(self.root.join("emulator")),
            http_client: Some(reqwest::Client::builder().no_proxy().build()?),
            service_base_url,
            ..Default::default()
        })?)
    }

    fn images_dir(&self) -> PathBuf {
        self.root.join("images")
    }

    fn deployed_dir(&self) -> PathBuf {
        self.root.join("deployed")
    }

    fn cache_dir(&self) -> PathBuf {
        self.root.join("cache")
    }

    fn sdk_root(&self) -> PathBuf {
        self.root.join("sdk")
    }

    fn sdk_openharmony_dir(&self) -> PathBuf {
        self.sdk_root().join("default").join("openharmony")
    }

    async fn create_local_image(
        &self,
        relative_path: &str,
        display_name: &str,
        api_version: &str,
        platform_version: &str,
        version: &str,
        guest_version: &str,
        abi: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let full_path = self.images_dir().join(relative_path);
        tokio::fs::create_dir_all(&full_path).await?;
        let sdk_pkg = json!({
            "data": {
                "apiVersion": api_version,
                "displayName": display_name,
                "path": relative_path.replace('/', ","),
                "platformVersion": platform_version,
                "releaseType": "Release",
                "version": version,
                "guestVersion": guest_version,
                "stage": "Release"
            }
        });
        let info = json!({
            "apiVersion": api_version,
            "abi": abi,
            "version": version
        });
        tokio::fs::write(
            full_path.join("sdk-pkg.json"),
            serde_json::to_vec_pretty(&sdk_pkg)?,
        )
        .await?;
        tokio::fs::write(
            full_path.join("info.json"),
            serde_json::to_vec_pretty(&info)?,
        )
        .await?;
        Ok(())
    }

    async fn write_emulator_sdk_version(
        &self,
        version: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let sdk_pkg = json!({ "data": { "version": version } });
        tokio::fs::write(
            self.root.join("emulator").join("sdk-pkg.json"),
            serde_json::to_vec_pretty(&sdk_pkg)?,
        )
        .await?;
        Ok(())
    }
}

#[derive(Clone)]
struct MockState {
    sdk_list: serde_json::Value,
    download_response: serde_json::Value,
    archive_bytes: Arc<Vec<u8>>,
}

struct MockServer {
    base_url: String,
    task: tokio::task::JoinHandle<()>,
}

impl MockServer {
    async fn start(
        sdk_list: serde_json::Value,
        archive_bytes: Vec<u8>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await?;
        let addr = listener.local_addr()?;
        let base_url = format!("http://{}", addr);
        let state = MockState {
            sdk_list,
            download_response: json!({
                "url": format!("{base_url}/download/archive.zip")
            }),
            archive_bytes: Arc::new(archive_bytes),
        };

        let app = Router::new()
            .route("/sdkmanager/v8/hos/getSdkList", post(get_sdk_list))
            .route("/sdkmanager/v7/hos/download", post(get_download_info))
            .route("/download/archive.zip", get(download_archive))
            .with_state(state);

        let task = tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("mock server should run");
        });

        Ok(Self { base_url, task })
    }

    async fn shutdown(self) {
        self.task.abort();
        let _ = self.task.await;
    }
}

async fn get_sdk_list(State(state): State<MockState>) -> Json<serde_json::Value> {
    Json(state.sdk_list)
}

async fn get_download_info(State(state): State<MockState>) -> Json<serde_json::Value> {
    Json(state.download_response)
}

async fn download_archive(State(state): State<MockState>, headers: HeaderMap) -> Response<Body> {
    let archive = state.archive_bytes;
    let total = archive.len();
    if let Some(range_header) = headers.get(RANGE).and_then(|value| value.to_str().ok()) {
        if let Some(start) = parse_range_start(range_header) {
            if start >= total {
                return Response::builder()
                    .status(StatusCode::RANGE_NOT_SATISFIABLE)
                    .body(Body::empty())
                    .expect("range error response should build");
            }

            let body = archive[start..].to_vec();
            return Response::builder()
                .status(StatusCode::PARTIAL_CONTENT)
                .header(
                    CONTENT_RANGE,
                    format!("bytes {}-{}/{}", start, total.saturating_sub(1), total),
                )
                .header(CONTENT_LENGTH, body.len().to_string())
                .body(Body::from(body))
                .expect("partial content response should build");
        }
    }

    Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_LENGTH, total.to_string())
        .body(Body::from(archive.as_ref().clone()))
        .expect("archive response should build")
}

fn parse_range_start(value: &str) -> Option<usize> {
    value
        .strip_prefix("bytes=")
        .and_then(|value| value.strip_suffix('-'))
        .and_then(|value| value.parse::<usize>().ok())
}

fn build_image_archive(
    display_name: &str,
    sdk_path: &str,
    api_version: &str,
    platform_version: &str,
    version: &str,
    guest_version: &str,
    abi: &str,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let sdk_pkg = json!({
        "data": {
            "apiVersion": api_version,
            "displayName": display_name,
            "path": sdk_path,
            "platformVersion": platform_version,
            "releaseType": "Release",
            "version": version,
            "guestVersion": guest_version,
            "stage": "Release"
        }
    });
    let info = json!({
        "apiVersion": api_version,
        "abi": abi,
        "version": version
    });

    let cursor = Cursor::new(Vec::new());
    let mut writer = zip::ZipWriter::new(cursor);
    let options =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    writer.start_file("sdk-pkg.json", options)?;
    writer.write_all(&serde_json::to_vec_pretty(&sdk_pkg)?)?;
    writer.start_file("info.json", options)?;
    writer.write_all(&serde_json::to_vec_pretty(&info)?)?;
    writer.start_file("bin/readme.txt", options)?;
    writer.write_all(b"fixture archive")?;
    Ok(writer.finish()?.into_inner())
}
