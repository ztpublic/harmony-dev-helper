use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::Arc;

use image_manager_rs::{
    Device, DeviceSpec, EmulatorDeviceType, ImageManager, ImageManagerOptions, LocalImage,
    ProductDeviceType, ProgressEvent, ProgressKind, RemoteImage, ScreenPreset, SpeedUnit,
};
use serde::Serialize;
use serde_json::json;
use tokio::sync::{mpsc, Mutex};

use crate::{host_message, next_message_id, Envelope};

const COMPATIBILITY_WARNING: &str =
    "DevEco emulator package 6.0.2 or newer was not detected. Some emulator features may fail.";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmulatorResolvedPathsPayload {
    pub image_base_path: String,
    pub deployed_path: String,
    pub cache_path: String,
    pub sdk_path: String,
    pub config_path: String,
    pub log_path: String,
    pub emulator_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmulatorEnvironmentPayload {
    pub compatibility: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub paths: EmulatorResolvedPathsPayload,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmulatorImageSummaryPayload {
    pub relative_path: String,
    pub display_name: String,
    pub api_version: u32,
    pub device_type: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guest_version: Option<String>,
    pub release_type: String,
    pub description: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archive_size_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checksum: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmulatorDownloadJobPayload {
    pub job_id: String,
    pub image_relative_path: String,
    pub stage: String,
    pub status: String,
    pub progress: f64,
    pub increment: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    pub reset: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmulatorProductPresetPayload {
    pub name: String,
    pub device_type: String,
    pub screen_width: String,
    pub screen_height: String,
    pub screen_diagonal: String,
    pub screen_density: String,
    pub default_cpu_cores: u32,
    pub default_memory_ram_mb: u32,
    pub default_data_disk_mb: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmulatorCreateDeviceOptionsPayload {
    pub image_relative_path: String,
    pub product_presets: Vec<EmulatorProductPresetPayload>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmulatorDeviceSummaryPayload {
    pub name: String,
    pub instance_path: String,
    pub device_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub api_version: u32,
    pub show_version: String,
    pub storage_size_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot_base64: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EmulatorCreateDeviceArgs {
    pub relative_path: String,
    pub product_device_type: String,
    pub product_name: String,
    pub name: String,
    pub cpu_cores: u32,
    pub memory_ram_mb: u32,
    pub data_disk_mb: u32,
    pub vendor_country: Option<String>,
    pub is_public: bool,
}

#[derive(Debug, Clone, Default)]
pub struct EmulatorSessionState {
    download_jobs: Arc<Mutex<BTreeMap<String, EmulatorDownloadJobPayload>>>,
}

impl EmulatorSessionState {
    pub async fn list_download_jobs(&self) -> Vec<EmulatorDownloadJobPayload> {
        let jobs = self.download_jobs.lock().await;
        jobs.values().rev().cloned().collect()
    }

    async fn active_job_for_image(&self, image_relative_path: &str) -> Option<EmulatorDownloadJobPayload> {
        let jobs = self.download_jobs.lock().await;
        jobs.values()
            .find(|job| job.image_relative_path == image_relative_path && job.status == "running")
            .cloned()
    }

    async fn upsert_job(&self, job: EmulatorDownloadJobPayload) {
        let mut jobs = self.download_jobs.lock().await;
        jobs.insert(job.job_id.clone(), job);
    }
}

pub async fn get_environment() -> Result<EmulatorEnvironmentPayload, String> {
    let manager = build_manager()?;
    let resolved = manager.resolved_paths();
    let compatibility = manager
        .is_compatible()
        .await
        .map_err(|error| error.to_string())?;

    Ok(EmulatorEnvironmentPayload {
        compatibility,
        message: (!compatibility).then(|| COMPATIBILITY_WARNING.to_string()),
        paths: EmulatorResolvedPathsPayload {
            image_base_path: resolved.image_base_path.to_string_lossy().to_string(),
            deployed_path: resolved.deployed_path.to_string_lossy().to_string(),
            cache_path: resolved.cache_path.to_string_lossy().to_string(),
            sdk_path: resolved.sdk_path.to_string_lossy().to_string(),
            config_path: resolved.config_path.to_string_lossy().to_string(),
            log_path: resolved.log_path.to_string_lossy().to_string(),
            emulator_path: resolved.emulator_path.to_string_lossy().to_string(),
        },
    })
}

pub async fn list_images(
    session: &EmulatorSessionState,
) -> Result<Vec<EmulatorImageSummaryPayload>, String> {
    let manager = build_manager()?;
    let remote_images = manager
        .remote_images(None)
        .await
        .map_err(|error| error.to_string())?;
    let local_images = manager
        .local_images()
        .await
        .map_err(|error| error.to_string())?;

    let local_map = local_images
        .into_iter()
        .map(|image| (relative_path_string(image.relative_path()), image))
        .collect::<HashMap<_, _>>();
    let active_jobs = session.list_download_jobs().await;
    let active_image_paths = active_jobs
        .into_iter()
        .filter(|job| job.status == "running")
        .map(|job| job.image_relative_path)
        .collect::<std::collections::HashSet<_>>();

    let mut images = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for remote_image in remote_images {
        let relative_path = relative_path_string(remote_image.relative_path());
        let local_image = local_map.get(&relative_path);
        images.push(image_summary_from_sources(
            Some(&remote_image),
            local_image,
            active_image_paths.contains(&relative_path),
        ));
        seen.insert(relative_path);
    }

    for local_image in local_map.values() {
        let relative_path = relative_path_string(local_image.relative_path());
        if seen.contains(&relative_path) {
            continue;
        }

        images.push(image_summary_from_sources(
            None,
            Some(local_image),
            active_image_paths.contains(&relative_path),
        ));
    }

    images.sort_by(|left, right| {
        left.status
            .cmp(&right.status)
            .then_with(|| left.display_name.cmp(&right.display_name))
            .then_with(|| left.api_version.cmp(&right.api_version))
    });

    Ok(images)
}

pub async fn list_download_jobs(session: &EmulatorSessionState) -> Vec<EmulatorDownloadJobPayload> {
    session.list_download_jobs().await
}

pub async fn get_create_device_options(
    relative_path: &str,
) -> Result<EmulatorCreateDeviceOptionsPayload, String> {
    let manager = build_manager()?;
    let local_image = find_local_image(&manager, relative_path).await?;
    let product_catalog = manager
        .read_product_catalog()
        .await
        .map_err(|error| error.to_string())?;
    let emulator_catalog = manager
        .read_emulator_catalog()
        .await
        .map_err(|error| error.to_string())?;

    let mut product_presets = Vec::new();
    for product_device_type in supported_product_device_types(local_image.device_type()) {
        let emulator_device_type = product_to_emulator_device_type(&product_device_type)
            .ok_or_else(|| format!("unsupported emulator device type for `{}`", product_device_type.as_str()))?;
        let emulator_device = emulator_catalog
            .find_device(Some(local_image.api_version()), Some(&emulator_device_type))
            .cloned()
            .ok_or_else(|| {
                format!(
                    "no emulator preset found for API {} and device type {}",
                    local_image.api_version(),
                    emulator_device_type.as_str()
                )
            })?;

        for product in product_catalog.find_items(Some(&product_device_type), None) {
            if !product.visible {
                continue;
            }

            product_presets.push(EmulatorProductPresetPayload {
                name: product.name.clone(),
                device_type: product_device_type.as_str().to_string(),
                screen_width: product.screen_width.clone(),
                screen_height: product.screen_height.clone(),
                screen_diagonal: product.screen_diagonal.clone(),
                screen_density: product.screen_density.clone(),
                default_cpu_cores: emulator_device.proc_number,
                default_memory_ram_mb: emulator_device.memory_ram_size,
                default_data_disk_mb: emulator_device.data_disk_size,
            });
        }
    }

    product_presets.sort_by(|left, right| {
        left.device_type
            .cmp(&right.device_type)
            .then_with(|| left.name.cmp(&right.name))
    });

    Ok(EmulatorCreateDeviceOptionsPayload {
        image_relative_path: relative_path.to_string(),
        product_presets,
    })
}

pub async fn download_image(
    relative_path: &str,
    session: &EmulatorSessionState,
    outbound_tx: mpsc::Sender<Envelope>,
) -> Result<String, String> {
    if let Some(job) = session.active_job_for_image(relative_path).await {
        return Ok(job.job_id);
    }

    let manager = build_manager()?;
    let remote_image = find_remote_image(&manager, relative_path).await?;
    let image_relative_path = relative_path_string(remote_image.relative_path());
    let job_id = next_message_id("emulator-download");
    let initial_job = EmulatorDownloadJobPayload {
        job_id: job_id.clone(),
        image_relative_path: image_relative_path.clone(),
        stage: "download".to_string(),
        status: "running".to_string(),
        progress: 0.0,
        increment: 0.0,
        network: None,
        unit: None,
        reset: false,
        message: None,
    };

    session.upsert_job(initial_job).await;

    let spawn_job_id = job_id.clone();
    let spawn_image_relative_path = image_relative_path.clone();
    let spawn_outbound_tx = outbound_tx.clone();
    let session_state = session.clone();
    tokio::spawn(async move {
        let run_result =
            run_download_job(
                spawn_job_id.clone(),
                spawn_image_relative_path.clone(),
                session_state.clone(),
                spawn_outbound_tx.clone(),
            )
            .await;

        if let Err((stage, message)) = run_result {
            let failed_job = EmulatorDownloadJobPayload {
                job_id: spawn_job_id.clone(),
                image_relative_path: spawn_image_relative_path.clone(),
                stage,
                status: "failed".to_string(),
                progress: 0.0,
                increment: 0.0,
                network: None,
                unit: None,
                reset: false,
                message: Some(message.clone()),
            };
            session_state.upsert_job(failed_job.clone()).await;
            let _ = spawn_outbound_tx
                .send(host_message(
                    next_message_id("event"),
                    "event",
                    json!({
                        "name": "emulator.download.failed",
                        "data": {
                            "jobId": failed_job.job_id,
                            "imageRelativePath": failed_job.image_relative_path,
                            "stage": failed_job.stage,
                            "status": failed_job.status,
                            "message": message
                        }
                    }),
                ))
                .await;
        }
    });

    Ok(job_id)
}

pub async fn list_devices() -> Result<Vec<EmulatorDeviceSummaryPayload>, String> {
    let manager = build_manager()?;
    let devices = manager
        .deployed_devices()
        .await
        .map_err(|error| error.to_string())?;
    let mut summaries = Vec::new();

    for device in devices {
        summaries.push(device_summary(device).await?);
    }

    summaries.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(summaries)
}

pub async fn create_device(args: EmulatorCreateDeviceArgs) -> Result<EmulatorDeviceSummaryPayload, String> {
    let manager = build_manager()?;
    let local_image = find_local_image(&manager, &args.relative_path).await?;
    let product_device_type = parse_product_device_type(&args.product_device_type)?;
    let emulator_device_type = product_to_emulator_device_type(&product_device_type)
        .ok_or_else(|| format!("unsupported product device type `{}`", args.product_device_type))?;
    let product_catalog = manager
        .read_product_catalog()
        .await
        .map_err(|error| error.to_string())?;
    let product_config = product_catalog
        .find_item(Some(&product_device_type), Some(&args.product_name))
        .cloned()
        .ok_or_else(|| {
            format!(
                "product preset `{}` not found for device type `{}`",
                args.product_name, args.product_device_type
            )
        })?;
    let emulator_catalog = manager
        .read_emulator_catalog()
        .await
        .map_err(|error| error.to_string())?;
    let emulator_device = emulator_catalog
        .find_device(Some(local_image.api_version()), Some(&emulator_device_type))
        .cloned()
        .ok_or_else(|| {
            format!(
                "emulator preset missing for API {} and device type `{}`",
                local_image.api_version(),
                emulator_device_type.as_str()
            )
        })?;

    let mut spec = DeviceSpec::new(
        args.name,
        args.cpu_cores,
        args.memory_ram_mb,
        args.data_disk_mb,
        ScreenPreset::new(emulator_device, product_config),
    );

    if let Some(vendor_country) = args.vendor_country {
        if !vendor_country.trim().is_empty() {
            spec = spec.with_vendor_country(vendor_country.trim().to_uppercase());
        }
    }
    spec = spec.with_public(args.is_public);

    let device = local_image
        .create_device(spec)
        .await
        .map_err(|error| error.to_string())?;
    device_summary(device).await
}

pub async fn start_device(name: &str) -> Result<String, String> {
    let manager = build_manager()?;
    let device = find_device_by_name(&manager, name).await?;
    let _child = device.start().await.map_err(|error| error.to_string())?;
    Ok(name.to_string())
}

pub async fn stop_device(name: &str) -> Result<String, String> {
    let manager = build_manager()?;
    let device = find_device_by_name(&manager, name).await?;
    let _child = device.stop().await.map_err(|error| error.to_string())?;
    Ok(name.to_string())
}

pub async fn delete_device(name: &str) -> Result<String, String> {
    let manager = build_manager()?;
    let device = find_device_by_name(&manager, name).await?;
    device.delete().await.map_err(|error| error.to_string())?;
    Ok(name.to_string())
}

fn build_manager() -> Result<ImageManager, String> {
    ImageManager::new(ImageManagerOptions::default()).map_err(|error| error.to_string())
}

fn relative_path_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn parse_archive_size(value: Option<&str>) -> Option<u64> {
    value.and_then(|size| size.parse::<u64>().ok())
}

fn image_summary_from_sources(
    remote_image: Option<&RemoteImage>,
    local_image: Option<&LocalImage>,
    downloading: bool,
) -> EmulatorImageSummaryPayload {
    let relative_path = remote_image
        .map(|image| relative_path_string(image.relative_path()))
        .or_else(|| local_image.map(|image| relative_path_string(image.relative_path())))
        .unwrap_or_default();
    let device_type = remote_image
        .map(|image| image.device_type().as_str().to_string())
        .or_else(|| local_image.map(|image| image.device_type().as_str().to_string()))
        .unwrap_or_default();
    let status = if downloading {
        "downloading"
    } else if local_image.is_some() {
        "installed"
    } else {
        "available"
    };

    if let Some(remote_image) = remote_image {
        let archive = remote_image.sdk().archive.as_ref().and_then(|archive| archive.complete.as_ref());
        return EmulatorImageSummaryPayload {
            relative_path,
            display_name: remote_image.sdk().display_name.clone(),
            api_version: remote_image.api_version(),
            device_type,
            version: remote_image.sdk().version.clone(),
            platform_version: remote_image
                .sdk()
                .extra
                .get("platformVersion")
                .and_then(|value| value.as_str())
                .map(ToString::to_string),
            guest_version: remote_image
                .sdk()
                .extra
                .get("guestVersion")
                .and_then(|value| value.as_str())
                .map(ToString::to_string),
            release_type: remote_image.sdk().release_type.clone(),
            description: remote_image.sdk().description.clone(),
            status: status.to_string(),
            local_path: local_image.map(|image| image.full_path().to_string_lossy().to_string()),
            archive_size_bytes: parse_archive_size(archive.map(|entry| entry.size.as_str())),
            checksum: archive.map(|entry| entry.checksum.clone()),
        };
    }

    let local_image = local_image.expect("local image required when remote image is absent");
    EmulatorImageSummaryPayload {
        relative_path,
        display_name: local_image.sdk_pkg().data.display_name.clone(),
        api_version: local_image.api_version(),
        device_type,
        version: local_image.sdk_pkg().data.version.clone(),
        platform_version: Some(local_image.sdk_pkg().data.platform_version.clone()),
        guest_version: Some(local_image.sdk_pkg().data.guest_version.clone()),
        release_type: local_image.sdk_pkg().data.release_type.clone(),
        description: String::new(),
        status: status.to_string(),
        local_path: Some(local_image.full_path().to_string_lossy().to_string()),
        archive_size_bytes: None,
        checksum: None,
    }
}

async fn run_download_job(
    job_id: String,
    image_relative_path: String,
    session: EmulatorSessionState,
    outbound_tx: mpsc::Sender<Envelope>,
) -> Result<(), (String, String)> {
    let manager = build_manager().map_err(|message| ("download".to_string(), message))?;
    let remote_image = find_remote_image(&manager, &image_relative_path)
        .await
        .map_err(|message| ("download".to_string(), message))?;
    let downloader = remote_image
        .create_downloader()
        .await
        .map_err(|error| ("download".to_string(), error.to_string()))?;

    downloader
        .download({
            let outbound_tx = outbound_tx.clone();
            let session = session.clone();
            let job_id = job_id.clone();
            let image_relative_path = image_relative_path.clone();
            move |event| {
                let outbound_tx = outbound_tx.clone();
                let session = session.clone();
                let job_id = job_id.clone();
                let image_relative_path = image_relative_path.clone();
                tokio::spawn(async move {
                    emit_progress_event(session, outbound_tx, job_id, image_relative_path, event).await;
                });
            }
        })
        .await
        .map_err(|error| ("download".to_string(), error.to_string()))?;

    let checksum_verified = downloader
        .verify_checksum({
            let outbound_tx = outbound_tx.clone();
            let session = session.clone();
            let job_id = job_id.clone();
            let image_relative_path = image_relative_path.clone();
            move |event| {
                let outbound_tx = outbound_tx.clone();
                let session = session.clone();
                let job_id = job_id.clone();
                let image_relative_path = image_relative_path.clone();
                tokio::spawn(async move {
                    emit_progress_event(session, outbound_tx, job_id, image_relative_path, event).await;
                });
            }
        })
        .await
        .map_err(|error| ("checksum".to_string(), error.to_string()))?;

    if !checksum_verified {
        return Err((
            "checksum".to_string(),
            "checksum verification failed for the downloaded emulator image".to_string(),
        ));
    }

    downloader
        .extract({
            let outbound_tx = outbound_tx.clone();
            let session = session.clone();
            let job_id = job_id.clone();
            let image_relative_path = image_relative_path.clone();
            move |event| {
                let outbound_tx = outbound_tx.clone();
                let session = session.clone();
                let job_id = job_id.clone();
                let image_relative_path = image_relative_path.clone();
                tokio::spawn(async move {
                    emit_progress_event(session, outbound_tx, job_id, image_relative_path, event).await;
                });
            }
        })
        .await
        .map_err(|error| ("extract".to_string(), error.to_string()))?;

    let manager = build_manager().map_err(|message| ("extract".to_string(), message))?;
    let local_image = find_local_image(&manager, &image_relative_path)
        .await
        .map_err(|message| ("extract".to_string(), message))?;
    let remote_image = find_remote_image(&manager, &image_relative_path)
        .await
        .map_err(|message| ("extract".to_string(), message))?;
    let image = image_summary_from_sources(Some(&remote_image), Some(&local_image), false);
    let finished_job = EmulatorDownloadJobPayload {
        job_id: job_id.clone(),
        image_relative_path: image_relative_path.clone(),
        stage: "extract".to_string(),
        status: "succeeded".to_string(),
        progress: 100.0,
        increment: 0.0,
        network: None,
        unit: None,
        reset: false,
        message: None,
    };
    session.upsert_job(finished_job).await;

    let _ = outbound_tx
        .send(host_message(
            next_message_id("event"),
            "event",
            json!({
                "name": "emulator.download.finished",
                "data": {
                    "jobId": job_id,
                    "imageRelativePath": image_relative_path,
                    "stage": "extract",
                    "status": "succeeded",
                    "image": image
                }
            }),
        ))
        .await;

    Ok(())
}

async fn emit_progress_event(
    session: EmulatorSessionState,
    outbound_tx: mpsc::Sender<Envelope>,
    job_id: String,
    image_relative_path: String,
    event: ProgressEvent,
) {
    let stage = progress_kind_string(event.kind);
    let job = EmulatorDownloadJobPayload {
        job_id: job_id.clone(),
        image_relative_path: image_relative_path.clone(),
        stage: stage.clone(),
        status: "running".to_string(),
        progress: event.update.progress,
        increment: event.update.increment,
        network: event.update.network,
        unit: event.update.unit.map(speed_unit_string),
        reset: event.update.reset,
        message: None,
    };

    session.upsert_job(job.clone()).await;

    let _ = outbound_tx
        .send(host_message(
            next_message_id("event"),
            "event",
            json!({
                "name": "emulator.download.progress",
                "data": {
                    "jobId": job.job_id,
                    "imageRelativePath": job.image_relative_path,
                    "stage": job.stage,
                    "status": job.status,
                    "progress": job.progress,
                    "increment": job.increment,
                    "network": job.network,
                    "unit": job.unit,
                    "reset": job.reset
                }
            }),
        ))
        .await;
}

fn progress_kind_string(kind: ProgressKind) -> String {
    match kind {
        ProgressKind::Download => "download".to_string(),
        ProgressKind::Checksum => "checksum".to_string(),
        ProgressKind::Extract => "extract".to_string(),
    }
}

fn speed_unit_string(unit: SpeedUnit) -> String {
    match unit {
        SpeedUnit::KB => "KB".to_string(),
        SpeedUnit::MB => "MB".to_string(),
    }
}

async fn device_summary(device: Device) -> Result<EmulatorDeviceSummaryPayload, String> {
    let storage_size_bytes = device
        .storage_size()
        .await
        .map_err(|error| error.to_string())?;
    let snapshot_base64 = if device.snapshot_path().exists() {
        device.snapshot_base64().await.ok()
    } else {
        None
    };
    let entry = device.lists_entry();

    Ok(EmulatorDeviceSummaryPayload {
        name: entry.name.clone(),
        instance_path: entry.path.clone(),
        device_type: entry.device_type.clone(),
        model: entry.model.clone(),
        api_version: entry.api_version.parse().unwrap_or_default(),
        show_version: entry.show_version.clone(),
        storage_size_bytes,
        snapshot_base64,
    })
}

async fn find_remote_image(manager: &ImageManager, relative_path: &str) -> Result<RemoteImage, String> {
    let normalized = normalize_relative_path(relative_path);
    manager
        .remote_images(None)
        .await
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|image| relative_path_string(image.relative_path()) == normalized)
        .ok_or_else(|| format!("remote image `{normalized}` not found"))
}

async fn find_local_image(manager: &ImageManager, relative_path: &str) -> Result<LocalImage, String> {
    let normalized = normalize_relative_path(relative_path);
    manager
        .local_images()
        .await
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|image| relative_path_string(image.relative_path()) == normalized)
        .ok_or_else(|| format!("installed image `{normalized}` not found"))
}

async fn find_device_by_name(manager: &ImageManager, name: &str) -> Result<Device, String> {
    let normalized_name = name.trim();
    manager
        .deployed_devices()
        .await
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|device| device.lists_entry().name == normalized_name)
        .ok_or_else(|| format!("emulator device `{normalized_name}` not found"))
}

fn normalize_relative_path(value: &str) -> String {
    value.trim().replace('\\', "/")
}

fn supported_product_device_types(device_type: EmulatorDeviceType) -> Vec<ProductDeviceType> {
    match device_type {
        EmulatorDeviceType::Phone => vec![ProductDeviceType::Phone],
        EmulatorDeviceType::Tablet => vec![ProductDeviceType::Tablet],
        EmulatorDeviceType::TwoInOne => vec![ProductDeviceType::TwoInOne],
        EmulatorDeviceType::Foldable => vec![ProductDeviceType::Foldable],
        EmulatorDeviceType::WideFold => vec![ProductDeviceType::WideFold],
        EmulatorDeviceType::TripleFold => vec![ProductDeviceType::TripleFold],
        EmulatorDeviceType::TwoInOneFoldable => vec![ProductDeviceType::TwoInOneFoldable],
        EmulatorDeviceType::Tv => vec![ProductDeviceType::Tv],
        EmulatorDeviceType::Wearable => vec![ProductDeviceType::Wearable],
        EmulatorDeviceType::PhoneAll => vec![
            ProductDeviceType::Phone,
            ProductDeviceType::Foldable,
            ProductDeviceType::WideFold,
            ProductDeviceType::TripleFold,
        ],
        EmulatorDeviceType::PcAll => vec![
            ProductDeviceType::TwoInOne,
            ProductDeviceType::TwoInOneFoldable,
        ],
        EmulatorDeviceType::Other(_) => Vec::new(),
    }
}

fn product_to_emulator_device_type(device_type: &ProductDeviceType) -> Option<EmulatorDeviceType> {
    match device_type {
        ProductDeviceType::Phone => Some(EmulatorDeviceType::Phone),
        ProductDeviceType::Tablet => Some(EmulatorDeviceType::Tablet),
        ProductDeviceType::TwoInOne => Some(EmulatorDeviceType::TwoInOne),
        ProductDeviceType::Foldable => Some(EmulatorDeviceType::Foldable),
        ProductDeviceType::WideFold => Some(EmulatorDeviceType::WideFold),
        ProductDeviceType::TripleFold => Some(EmulatorDeviceType::TripleFold),
        ProductDeviceType::TwoInOneFoldable => Some(EmulatorDeviceType::TwoInOneFoldable),
        ProductDeviceType::Tv => Some(EmulatorDeviceType::Tv),
        ProductDeviceType::Wearable => Some(EmulatorDeviceType::Wearable),
        ProductDeviceType::WearableKid | ProductDeviceType::Other(_) => None,
    }
}

fn parse_product_device_type(value: &str) -> Result<ProductDeviceType, String> {
    let parsed = ProductDeviceType::from_section_key(value.trim());
    match parsed {
        ProductDeviceType::Other(ref raw) if raw.trim().is_empty() => {
            Err("`productDeviceType` must be a non-empty string".to_string())
        }
        ProductDeviceType::Other(raw) => Err(format!("unsupported product device type `{raw}`")),
        other => Ok(other),
    }
}
