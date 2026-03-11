use std::io::{Cursor, Write};
use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::State;
use axum::http::header::{CONTENT_LENGTH, CONTENT_RANGE, RANGE};
use axum::http::{HeaderMap, Response, StatusCode};
use axum::routing::get;
use axum::Router;
use flate2::write::GzEncoder;
use flate2::Compression;
use sdk_manager_rs::{
    ProgressKind, SdkInstallOptions, SdkManager, SdkManagerError, SdkManagerOptions, SdkOs,
    SdkSource,
};
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use zip::write::FileOptions;

#[tokio::test]
async fn install_round_trip_resumes_download_and_cleans_cache(
) -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    let archive_bytes = build_sdk_archive(ArchiveMode::Safe)?;
    let checksum = hex::encode(Sha256::digest(&archive_bytes));
    let server = MockServer::start(archive_bytes.clone(), checksum.clone()).await?;
    let manager = fixture.manager(None)?;
    let downloader = manager.create_downloader(SdkInstallOptions::new(
        SdkSource::Url(server.archive_url()),
        fixture.cache_dir(),
        fixture.target_dir(),
    ))?;

    tokio::fs::create_dir_all(fixture.cache_dir()).await?;
    tokio::fs::write(downloader.cache_path(), &archive_bytes[..128]).await?;

    let mut events = Vec::new();
    downloader.install(|event| events.push(event)).await?;

    assert!(events
        .iter()
        .any(|event| event.kind == ProgressKind::Download && event.update.reset));
    assert!(events
        .iter()
        .any(|event| event.kind == ProgressKind::Checksum));
    assert!(events
        .iter()
        .any(|event| event.kind == ProgressKind::Extract));

    assert!(fixture
        .target_dir()
        .join("toolchain")
        .join("bin")
        .join("hdc")
        .exists());
    assert_eq!(
        std::fs::read_to_string(
            fixture
                .target_dir()
                .join("toolchain")
                .join("config")
                .join("sdk.txt")
        )?,
        "open-harmony-sdk"
    );

    if fixture.host_os() != SdkOs::MacOs {
        assert!(!fixture
            .target_dir()
            .join("unexpected")
            .join("wrong-os.txt")
            .exists());
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mode = std::fs::metadata(
            fixture
                .target_dir()
                .join("toolchain")
                .join("bin")
                .join("hdc"),
        )?
        .permissions()
        .mode();
        assert_ne!(mode & 0o100, 0);

        let symlink_path = fixture
            .target_dir()
            .join("toolchain")
            .join("bin")
            .join("hdc-link");
        let symlink_target = std::fs::read_link(symlink_path)?;
        assert_eq!(symlink_target, PathBuf::from("hdc"));
    }

    assert!(!fixture.cache_dir().exists());
    server.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn verify_checksum_reports_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    let archive_bytes = build_sdk_archive(ArchiveMode::Safe)?;
    let server = MockServer::start(archive_bytes.clone(), "0".repeat(64)).await?;
    let manager = fixture.manager(None)?;
    let downloader = manager.create_downloader(SdkInstallOptions::new(
        SdkSource::Url(server.archive_url()),
        fixture.cache_dir(),
        fixture.target_dir(),
    ))?;

    downloader.download_without_progress().await?;
    let error = downloader
        .verify_checksum_without_progress()
        .await
        .unwrap_err();
    assert!(matches!(error, SdkManagerError::ChecksumMismatch { .. }));

    server.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn extract_rejects_unsafe_zip_entries() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    let archive_bytes = build_sdk_archive(ArchiveMode::UnsafeZipEntry)?;
    let checksum = hex::encode(Sha256::digest(&archive_bytes));
    let server = MockServer::start(archive_bytes.clone(), checksum).await?;
    let manager = fixture.manager(None)?;
    let downloader = manager.create_downloader(SdkInstallOptions::new(
        SdkSource::Url(server.archive_url()),
        fixture.cache_dir(),
        fixture.target_dir(),
    ))?;

    downloader.download_without_progress().await?;
    downloader.verify_checksum_without_progress().await?;
    let error = downloader.extract_without_progress().await.unwrap_err();
    assert!(matches!(error, SdkManagerError::UnsafeArchivePath { .. }));

    server.shutdown().await;
    Ok(())
}

struct Fixture {
    _tempdir: TempDir,
    root: PathBuf,
}

impl Fixture {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let tempdir = TempDir::new()?;
        let root = tempdir.path().to_path_buf();
        std::fs::create_dir_all(root.join("cache"))?;
        std::fs::create_dir_all(root.join("target"))?;
        Ok(Self {
            _tempdir: tempdir,
            root,
        })
    }

    fn manager(
        &self,
        host_os_override: Option<SdkOs>,
    ) -> Result<SdkManager, Box<dyn std::error::Error>> {
        Ok(SdkManager::new(SdkManagerOptions {
            http_client: Some(reqwest::Client::builder().no_proxy().build()?),
            host_os: host_os_override,
        })?)
    }

    fn cache_dir(&self) -> PathBuf {
        self.root.join("cache")
    }

    fn target_dir(&self) -> PathBuf {
        self.root.join("target")
    }

    fn host_os(&self) -> SdkOs {
        SdkOs::current()
    }
}

#[derive(Clone)]
struct MockServer {
    shutdown: Arc<tokio::sync::Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
    archive_url: String,
}

impl MockServer {
    async fn start(
        archive_bytes: Vec<u8>,
        checksum: String,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        #[derive(Clone)]
        struct AppState {
            archive_bytes: Arc<Vec<u8>>,
            checksum: Arc<String>,
        }

        async fn archive_handler(
            State(state): State<AppState>,
            headers: HeaderMap,
        ) -> Response<Body> {
            let total_len = state.archive_bytes.len();
            if let Some(range) = headers.get(RANGE).and_then(|value| value.to_str().ok()) {
                if let Some(start) = parse_range_header(range) {
                    if start >= total_len {
                        return Response::builder()
                            .status(StatusCode::RANGE_NOT_SATISFIABLE)
                            .body(Body::empty())
                            .unwrap();
                    }

                    let body = state.archive_bytes[start..].to_vec();
                    return Response::builder()
                        .status(StatusCode::PARTIAL_CONTENT)
                        .header(CONTENT_LENGTH, body.len().to_string())
                        .header(
                            CONTENT_RANGE,
                            format!("bytes {}-{}/{}", start, total_len - 1, total_len),
                        )
                        .body(Body::from(body))
                        .unwrap();
                }
            }

            Response::builder()
                .status(StatusCode::OK)
                .header(CONTENT_LENGTH, total_len.to_string())
                .body(Body::from((*state.archive_bytes).clone()))
                .unwrap()
        }

        async fn checksum_handler(State(state): State<AppState>) -> Response<Body> {
            Response::builder()
                .status(StatusCode::OK)
                .header(CONTENT_LENGTH, state.checksum.len().to_string())
                .body(Body::from((*state.checksum).clone()))
                .unwrap()
        }

        let state = AppState {
            archive_bytes: Arc::new(archive_bytes),
            checksum: Arc::new(checksum),
        };
        let app = Router::new()
            .route("/sdk.tar.gz", get(archive_handler))
            .route("/sdk.tar.gz.sha256", get(checksum_handler))
            .with_state(state);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let server = axum::serve(listener, app).with_graceful_shutdown(async {
            let _ = shutdown_rx.await;
        });
        tokio::spawn(async move {
            let _ = server.await;
        });

        Ok(Self {
            shutdown: Arc::new(tokio::sync::Mutex::new(Some(shutdown_tx))),
            archive_url: format!("http://{address}/sdk.tar.gz"),
        })
    }

    fn archive_url(&self) -> String {
        self.archive_url.clone()
    }

    async fn shutdown(&self) {
        if let Some(sender) = self.shutdown.lock().await.take() {
            let _ = sender.send(());
        }
    }
}

enum ArchiveMode {
    Safe,
    UnsafeZipEntry,
}

fn build_sdk_archive(mode: ArchiveMode) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let host_os = SdkOs::current();
    let relevant_zip_name = match host_os {
        SdkOs::Windows => "payload-windows.zip",
        SdkOs::Linux => "payload-linux.zip",
        SdkOs::MacOs => "payload-macos.zip",
    };
    let decoy_zip_name = match host_os {
        SdkOs::Windows => "payload-linux.zip",
        SdkOs::Linux => "payload-windows.zip",
        SdkOs::MacOs => "payload-windows.zip",
    };

    let relevant_zip = build_zip_archive(relevant_zip_name, true, mode)?;
    let decoy_zip = build_zip_archive(decoy_zip_name, false, ArchiveMode::Safe)?;
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    let mut tar = tar::Builder::new(&mut encoder);

    add_tar_entry(&mut tar, &format!("sdk/{relevant_zip_name}"), &relevant_zip)?;
    add_tar_entry(&mut tar, &format!("sdk/{decoy_zip_name}"), &decoy_zip)?;
    tar.finish()?;
    drop(tar);
    Ok(encoder.finish()?)
}

fn build_zip_archive(
    zip_name: &str,
    include_expected_payload: bool,
    mode: ArchiveMode,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let cursor = Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(cursor);
    let file_options = FileOptions::default();

    zip.add_directory("toolchain/bin/", file_options)?;
    zip.add_directory("toolchain/config/", file_options)?;

    if include_expected_payload {
        let executable_options = file_options.unix_permissions(0o755);
        zip.start_file("toolchain/bin/hdc", executable_options)?;
        zip.write_all(b"binary")?;

        zip.add_symlink("toolchain/bin/hdc-link", "hdc", file_options)?;

        zip.start_file("toolchain/config/sdk.txt", file_options)?;
        zip.write_all(b"open-harmony-sdk")?;
    } else {
        zip.add_directory("unexpected/", file_options)?;
        zip.start_file("unexpected/wrong-os.txt", file_options)?;
        zip.write_all(zip_name.as_bytes())?;
    }

    if let ArchiveMode::UnsafeZipEntry = mode {
        zip.start_file("../escape.txt", file_options)?;
        zip.write_all(b"escape")?;
    }

    Ok(zip.finish()?.into_inner())
}

fn add_tar_entry(
    tar: &mut tar::Builder<&mut GzEncoder<Vec<u8>>>,
    path: &str,
    bytes: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut header = tar::Header::new_gnu();
    header.set_path(path)?;
    header.set_mode(0o644);
    header.set_size(bytes.len() as u64);
    header.set_cksum();
    tar.append(&header, bytes)?;
    Ok(())
}

fn parse_range_header(value: &str) -> Option<usize> {
    value
        .strip_prefix("bytes=")
        .and_then(|value| value.strip_suffix('-').or(Some(value)))
        .and_then(|value| value.parse::<usize>().ok())
}
