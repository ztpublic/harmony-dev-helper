use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

use base64::Engine;
use indexmap::IndexMap;
use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use tokio::process::{Child, Command};
use walkdir::WalkDir;

use crate::defaults::{DEFAULT_EMULATOR_ENTRIES, DEFAULT_PRODUCT_CATALOG};
use crate::downloader::Downloader;
use crate::error::{ImageManagerError, RemoteApiError};
use crate::ini::FlatIni;
use crate::types::{
    api_version_from_string, guest_version_prefix, relative_path_from_sdk_path,
    trimmed_relative_image_dir, ConfigIni, EmulatorCatalog, EmulatorDevice, EmulatorDeviceType,
    EmulatorEntry, InfoFile, ListsEntry, ListsFile, LocalImage, NamedIni, ProductCatalog,
    ProductConfigItem, ProductDeviceType, RemoteImage, RemoteImageDownloadResponse, RemoteImageSdk,
    SdkPkgFile,
};

const DEFAULT_SUPPORT_VERSION: &str = "6.0-hos-single-9";
const DEFAULT_SERVICE_BASE_URL: &str = "https://devecostudio-drcn.deveco.dbankcloud.com";
const DEFAULT_DOWNLOAD_IMEI: &str = "d490a470-8719-4baf-9cc4-9c78d40d";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Windows,
    Mac,
    Linux,
}

impl Platform {
    pub fn current() -> Self {
        match std::env::consts::OS {
            "windows" => Self::Windows,
            "macos" => Self::Mac,
            _ => Self::Linux,
        }
    }

    pub fn as_sdk_os(self) -> &'static str {
        match self {
            Self::Windows => "windows",
            Self::Mac => "mac",
            Self::Linux => "linux",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Architecture {
    X86,
    Arm64,
}

impl Architecture {
    pub fn current() -> Self {
        let arch = std::env::consts::ARCH;
        if arch.contains("arm") || arch.contains("aarch64") {
            Self::Arm64
        } else {
            Self::X86
        }
    }

    pub fn as_sdk_arch(self) -> &'static str {
        match self {
            Self::X86 => "x86",
            Self::Arm64 => "arm64",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ImageManagerOptions {
    pub image_base_path: Option<PathBuf>,
    pub deployed_path: Option<PathBuf>,
    pub cache_path: Option<PathBuf>,
    pub sdk_path: Option<PathBuf>,
    pub config_path: Option<PathBuf>,
    pub log_path: Option<PathBuf>,
    pub emulator_path: Option<PathBuf>,
    pub http_client: Option<reqwest::Client>,
    pub service_base_url: Option<String>,
    pub download_imei: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ResolvedPaths {
    pub image_base_path: PathBuf,
    pub deployed_path: PathBuf,
    pub cache_path: PathBuf,
    pub sdk_path: PathBuf,
    pub default_sdk_root: Option<PathBuf>,
    pub config_path: PathBuf,
    pub log_path: PathBuf,
    pub emulator_path: PathBuf,
}

#[derive(Clone)]
pub struct ImageManager(Arc<ImageManagerInner>);

#[derive(Debug)]
struct ImageManagerInner {
    paths: ResolvedPaths,
    platform: Platform,
    architecture: Architecture,
    http_client: reqwest::Client,
    service_base_url: String,
    download_imei: String,
}

impl std::fmt::Debug for ImageManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageManager")
            .field("paths", &self.0.paths)
            .field("platform", &self.0.platform)
            .field("architecture", &self.0.architecture)
            .finish()
    }
}

impl ImageManager {
    pub fn new(options: ImageManagerOptions) -> Result<Self, ImageManagerError> {
        Self::from_options(options)
    }

    pub fn from_options(options: ImageManagerOptions) -> Result<Self, ImageManagerError> {
        let ImageManagerOptions {
            image_base_path,
            deployed_path,
            cache_path,
            sdk_path,
            config_path,
            log_path,
            emulator_path,
            http_client,
            service_base_url,
            download_imei,
        } = options;
        let platform = Platform::current();
        let architecture = Architecture::current();
        let service_base_url = service_base_url
            .unwrap_or_else(|| DEFAULT_SERVICE_BASE_URL.to_string())
            .trim_end_matches('/')
            .to_string();
        let download_imei = download_imei.unwrap_or_else(|| DEFAULT_DOWNLOAD_IMEI.to_string());
        let http_client = match http_client {
            Some(client) => client,
            None => reqwest::Client::builder().build()?,
        };
        let paths = ResolvedPaths::resolve(
            platform,
            ImageManagerOptions {
                image_base_path,
                deployed_path,
                cache_path,
                sdk_path,
                config_path,
                log_path,
                emulator_path,
                http_client: None,
                service_base_url: None,
                download_imei: None,
            },
        )?;

        Ok(Self(Arc::new(ImageManagerInner {
            paths,
            platform,
            architecture,
            http_client,
            service_base_url,
            download_imei,
        })))
    }

    pub fn resolved_paths(&self) -> &ResolvedPaths {
        &self.0.paths
    }

    pub fn platform(&self) -> Platform {
        self.0.platform
    }

    pub fn architecture(&self) -> Architecture {
        self.0.architecture
    }

    pub(crate) fn http_client(&self) -> reqwest::Client {
        self.0.http_client.clone()
    }

    pub fn lists_path(&self) -> PathBuf {
        self.resolved_paths().deployed_path.join("lists.json")
    }

    pub async fn local_images(&self) -> Result<Vec<LocalImage>, ImageManagerError> {
        let system_image_root = self.resolved_paths().image_base_path.join("system-image");
        if !system_image_root.exists() {
            return Ok(Vec::new());
        }

        let mut images = Vec::new();
        for entry in WalkDir::new(&system_image_root)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file() && entry.file_name() == "sdk-pkg.json")
        {
            let sdk_pkg_path = entry.into_path();
            let info_path = sdk_pkg_path.with_file_name("info.json");

            let sdk_pkg = match read_json_file::<SdkPkgFile>(&sdk_pkg_path).await {
                Ok(value) => value,
                Err(_) => continue,
            };
            let info = match read_json_file::<InfoFile>(&info_path).await {
                Ok(value) => value,
                Err(_) => continue,
            };

            let relative_path = relative_path_from_sdk_path(&sdk_pkg.data.path);
            images.push(LocalImage {
                manager: self.clone(),
                api_version: api_version_from_string(&sdk_pkg.data.api_version),
                relative_path,
                sdk_pkg,
                info,
            });
        }

        Ok(images)
    }

    pub async fn remote_images(
        &self,
        support_version: Option<&str>,
    ) -> Result<Vec<RemoteImage>, ImageManagerError> {
        let url = self.endpoint("/sdkmanager/v8/hos/getSdkList");
        let payload = serde_json::json!({
            "osArch": self.architecture().as_sdk_arch(),
            "osType": self.platform().as_sdk_os(),
            "supportVersion": support_version.unwrap_or(DEFAULT_SUPPORT_VERSION),
        });

        let response = self.0.http_client.post(url).json(&payload).send().await?;
        let remote_sdks: Vec<RemoteImageSdk> =
            decode_remote_response(response, "getSdkList").await?;

        Ok(remote_sdks
            .into_iter()
            .map(|sdk| RemoteImage {
                manager: self.clone(),
                api_version: api_version_from_string(&sdk.api_version),
                relative_path: relative_path_from_sdk_path(&sdk.path),
                sdk,
            })
            .collect())
    }

    pub async fn downloaded_remote_images(
        &self,
        support_version: Option<&str>,
    ) -> Result<Vec<RemoteImage>, ImageManagerError> {
        let remote_images = self.remote_images(support_version).await?;
        let local_images = self.local_images().await?;
        let local_paths: std::collections::HashSet<PathBuf> = local_images
            .into_iter()
            .map(|image| image.full_path())
            .collect();

        Ok(remote_images
            .into_iter()
            .filter(|image| local_paths.contains(&image.full_path()))
            .collect())
    }

    pub async fn read_lists(&self) -> Result<ListsFile, ImageManagerError> {
        let path = self.lists_path();
        let Some(contents) = read_optional_string(&path).await? else {
            return Ok(ListsFile::empty(path));
        };

        if contents.trim().is_empty() {
            return Ok(ListsFile::empty(path));
        }

        let entries = serde_json::from_str::<Vec<ListsEntry>>(&contents).map_err(|source| {
            ImageManagerError::Json {
                path: path.clone(),
                source,
            }
        })?;
        Ok(ListsFile::from_entries(path, entries))
    }

    pub async fn read_product_catalog(&self) -> Result<ProductCatalog, ImageManagerError> {
        let path = self
            .resolved_paths()
            .emulator_path
            .join("product-config.json");
        let Some(contents) = read_optional_string(&path).await? else {
            return DEFAULT_PRODUCT_CATALOG
                .as_ref()
                .map(Clone::clone)
                .map_err(clone_static_error);
        };

        if contents.trim().is_empty() {
            return DEFAULT_PRODUCT_CATALOG
                .as_ref()
                .map(Clone::clone)
                .map_err(clone_static_error);
        }

        let sections = serde_json::from_str::<IndexMap<String, Vec<ProductConfigItem>>>(&contents)
            .map_err(|source| ImageManagerError::Json { path, source })?;
        Ok(ProductCatalog::from_sections(sections))
    }

    pub async fn read_emulator_catalog(&self) -> Result<EmulatorCatalog, ImageManagerError> {
        let path = self.resolved_paths().emulator_path.join("emulator.json");
        let Some(contents) = read_optional_string(&path).await? else {
            return DEFAULT_EMULATOR_ENTRIES
                .as_ref()
                .map(|entries| EmulatorCatalog::new(entries.clone()))
                .map_err(clone_static_error);
        };

        if contents.trim().is_empty() {
            return DEFAULT_EMULATOR_ENTRIES
                .as_ref()
                .map(|entries| EmulatorCatalog::new(entries.clone()))
                .map_err(clone_static_error);
        }

        let entries = serde_json::from_str::<Vec<EmulatorEntry>>(&contents)
            .map_err(|source| ImageManagerError::Json { path, source })?;
        Ok(EmulatorCatalog::new(entries))
    }

    pub async fn deployed_devices(&self) -> Result<Vec<Device>, ImageManagerError> {
        let lists = self.read_lists().await?;
        let product_catalog = self.read_product_catalog().await?;
        let emulator_catalog = self.read_emulator_catalog().await?;

        let mut valid_entries = Vec::new();
        let mut devices = Vec::new();

        for entry in lists.entries().iter().cloned() {
            let product_device_type = ProductDeviceType::from_lists_type(&entry.device_type);
            let Some(product_config) = product_catalog
                .find_item(Some(&product_device_type), entry.model.as_deref())
                .cloned()
            else {
                continue;
            };

            let api_version = entry.api_version.parse().unwrap_or_default();
            let emulator_device_type = EmulatorDeviceType::from_str(&entry.device_type);
            let Some(emulator_device) = emulator_catalog
                .find_device(Some(api_version), Some(&emulator_device_type))
                .cloned()
            else {
                continue;
            };

            let config_path = PathBuf::from(&entry.path).join("config.ini");
            let config = match FlatIni::read_from_path(&config_path).await {
                Ok(contents) => ConfigIni::new(config_path.clone(), contents),
                Err(ImageManagerError::Io { source, .. })
                    if source.kind() == std::io::ErrorKind::NotFound =>
                {
                    continue;
                }
                Err(error) => return Err(error),
            };

            let named_path = self
                .resolved_paths()
                .deployed_path
                .join(format!("{}.ini", entry.name));
            let named = match FlatIni::read_from_path(&named_path).await {
                Ok(contents) => NamedIni::new(named_path.clone(), contents),
                Err(ImageManagerError::Io { source, .. })
                    if source.kind() == std::io::ErrorKind::NotFound =>
                {
                    continue;
                }
                Err(error) => return Err(error),
            };

            let screen_preset = ScreenPreset::new(emulator_device, product_config);
            valid_entries.push(entry.clone());
            devices.push(Device::new(
                self.clone(),
                entry,
                screen_preset,
                config,
                named,
            ));
        }

        if valid_entries.len() != lists.entries().len() {
            ListsFile::from_entries(self.lists_path(), valid_entries)
                .write()
                .await?;
        }

        Ok(devices)
    }

    pub async fn is_compatible(&self) -> Result<bool, ImageManagerError> {
        let path = self.resolved_paths().emulator_path.join("sdk-pkg.json");
        let Some(contents) = read_optional_string(&path).await? else {
            return Ok(false);
        };

        let value: serde_json::Value = serde_json::from_str(&contents)
            .map_err(|source| ImageManagerError::Json { path, source })?;
        let version = value
            .get("data")
            .and_then(|data| data.get("version"))
            .and_then(serde_json::Value::as_str);
        Ok(version
            .map(parse_semver_tuple)
            .is_some_and(|tuple| tuple >= (6, 0, 2)))
    }

    pub(crate) async fn create_device_from_local_image(
        &self,
        image: &LocalImage,
        spec: DeviceSpec,
    ) -> Result<Device, ImageManagerError> {
        if spec.name.trim().is_empty() {
            return Err(ImageManagerError::Validation(
                "device name must not be empty".to_string(),
            ));
        }

        let mut lists = self.read_lists().await?;
        let entry = self.build_lists_entry(image, &spec);
        let config = self.build_config_ini(image, &entry, &spec);
        let named = self.build_named_ini(&entry);

        named.write().await?;
        if let Err(error) = config.write().await {
            let _ = tokio::fs::remove_file(named.path()).await;
            return Err(error);
        }

        lists.upsert(entry.clone());
        if let Err(error) = lists.write().await {
            let _ = tokio::fs::remove_file(named.path()).await;
            let _ = tokio::fs::remove_file(config.path()).await;
            let _ = tokio::fs::remove_dir_all(PathBuf::from(&entry.path)).await;
            return Err(error);
        }

        Ok(Device::new(
            self.clone(),
            entry,
            spec.screen_preset,
            config,
            named,
        ))
    }

    pub(crate) async fn create_downloader(
        &self,
        remote_image: &RemoteImage,
    ) -> Result<Downloader, ImageManagerError> {
        let url = self.download_url(remote_image).await?;
        Ok(Downloader::new(self.clone(), remote_image.clone(), url))
    }

    async fn download_url(&self, remote_image: &RemoteImage) -> Result<String, ImageManagerError> {
        let url = self.endpoint("/sdkmanager/v7/hos/download");
        let payload = serde_json::json!({
            "osArch": self.architecture().as_sdk_arch(),
            "osType": self.platform().as_sdk_os(),
            "path": {
                "path": remote_image.sdk().path,
                "version": remote_image.sdk().version,
            },
            "imei": self.0.download_imei,
        });

        let response = self.0.http_client.post(url).json(&payload).send().await?;
        let body: RemoteImageDownloadResponse =
            decode_remote_response(response, "download").await?;

        body.url.ok_or_else(|| {
            RemoteApiError {
                endpoint: "download",
                status: None,
                body: body.body,
                message: format!(
                    "download endpoint did not return a url{}",
                    body.code
                        .map(|code| format!(" (code {code})"))
                        .unwrap_or_default()
                ),
            }
            .into()
        })
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}{}", self.0.service_base_url, path)
    }

    fn build_lists_entry(&self, image: &LocalImage, spec: &DeviceSpec) -> ListsEntry {
        let paths = self.resolved_paths();
        let emulator = &spec.screen_preset.emulator_device;
        let product = &spec.screen_preset.product_config;
        let device_folder_path = paths.deployed_path.join(&spec.name);
        let sdk_path_for_lists = paths
            .default_sdk_root
            .clone()
            .unwrap_or_else(|| paths.sdk_path.clone());
        let guest_version_prefix = guest_version_prefix(&image.sdk_pkg.data.guest_version);

        ListsEntry {
            diagonal_size: emulator.diagonal_size.to_string(),
            density: emulator.density.to_string(),
            resolution_height: emulator.resolution_height.to_string(),
            resolution_width: emulator.resolution_width.to_string(),
            memory_ram_size: spec.memory_ram_mb.to_string(),
            cpu_number: spec.cpu_cores.to_string(),
            data_disk_size: spec.data_disk_mb.to_string(),
            name: spec.name.clone(),
            uuid: uuid::Uuid::new_v4().to_string(),
            hw_api_name: image.sdk_pkg.data.platform_version.clone(),
            dev_model: product.dev_model().map(str::to_string),
            model: Some(product.name.clone()),
            image_dir: trimmed_relative_image_dir(image.relative_path()),
            version: image.sdk_pkg.data.version.clone(),
            device_type: emulator.device_type.as_str().to_string(),
            abi: image.info.abi.clone(),
            api_version: image.api_version().to_string(),
            path: device_folder_path.to_string_lossy().to_string(),
            show_version: format!(
                "{} {}({})",
                guest_version_prefix,
                image.sdk_pkg.data.platform_version,
                image.sdk_pkg.data.api_version
            ),
            harmony_os_version: Some(format!(
                "{}-{}",
                guest_version_prefix, image.sdk_pkg.data.platform_version
            )),
            guest_version: Some(image.sdk_pkg.data.guest_version.clone()),
            cover_resolution_width: emulator
                .cover_resolution_width
                .map(|value| value.to_string()),
            cover_resolution_height: emulator
                .cover_resolution_height
                .map(|value| value.to_string()),
            cover_diagonal_size: emulator.cover_diagonal_size.map(|value| value.to_string()),
            harmony_sdk_path: sdk_path_for_lists.to_string_lossy().to_string(),
            harmony_config_path: paths.config_path.to_string_lossy().to_string(),
            harmony_log_path: paths.log_path.to_string_lossy().to_string(),
            extra: IndexMap::new(),
        }
    }

    fn build_config_ini(
        &self,
        image: &LocalImage,
        entry: &ListsEntry,
        spec: &DeviceSpec,
    ) -> ConfigIni {
        let emulator = &spec.screen_preset.emulator_device;
        let product = &spec.screen_preset.product_config;
        let mut ini = FlatIni::new();

        ini.insert("name", &entry.name);
        ini.insert("deviceType", &entry.device_type);
        if let Some(dev_model) = &entry.dev_model {
            ini.insert("deviceModel", dev_model);
        }
        if !emulator.is_triple_fold() {
            if let Some(model) = &entry.model {
                ini.insert("productModel", model);
            }
        }
        ini.insert("vendorCountry", spec.vendor_country.clone());
        ini.insert("uuid", &entry.uuid);
        ini.insert("configPath", &entry.harmony_config_path);
        ini.insert("logPath", &entry.harmony_log_path);
        ini.insert("sdkPath", &entry.harmony_sdk_path);
        ini.insert("imageSubPath", &entry.image_dir);
        ini.insert("instancePath", &entry.path);
        ini.insert(
            "os.osVersion",
            format!(
                "{} {}({})",
                guest_version_prefix(&image.sdk_pkg.data.guest_version),
                image.sdk_pkg.data.platform_version,
                image.api_version()
            ),
        );
        ini.insert("os.apiVersion", image.api_version().to_string());
        ini.insert("os.softwareVersion", image.sdk_pkg.data.version.clone());
        ini.insert("os.isPublic", if spec.is_public { "true" } else { "false" });
        ini.insert("hw.cpu.arch", self.architecture().as_sdk_arch());
        ini.insert("hw.cpu.ncore", &entry.cpu_number);
        ini.insert("hw.lcd.density", emulator.density.to_string());

        if emulator.is_triple_fold() {
            if let Some(value) = emulator.single_diagonal_size {
                ini.insert("hw.lcd.single.diagonalSize", value.to_string());
            }
            if let Some(value) = emulator.single_resolution_height {
                ini.insert("hw.lcd.single.height", value.to_string());
            }
            if let Some(value) = emulator.single_resolution_width {
                ini.insert("hw.lcd.single.width", value.to_string());
            }
            if let Some(value) = emulator.double_diagonal_size {
                ini.insert("hw.lcd.double.diagonalSize", value.to_string());
            }
            if let Some(value) = emulator.double_resolution_height {
                ini.insert("hw.lcd.double.height", value.to_string());
            }
            if let Some(value) = emulator.double_resolution_width {
                ini.insert("hw.lcd.double.width", value.to_string());
            }
            ini.insert(
                "hw.lcd.triple.diagonalSize",
                emulator.diagonal_size.to_string(),
            );
            ini.insert(
                "hw.lcd.triple.height",
                emulator.resolution_height.to_string(),
            );
            ini.insert("hw.lcd.triple.width", emulator.resolution_width.to_string());
        } else if product.is_fold_like() {
            if let Some(value) = &product.outer_screen_diagonal {
                ini.insert("hw.lcd.single.diagonalSize", value.clone());
            }
            if let Some(value) = &product.outer_screen_height {
                ini.insert("hw.lcd.single.height", value.clone());
            }
            if let Some(value) = &product.outer_screen_width {
                ini.insert("hw.lcd.single.width", value.clone());
            }
            ini.insert("hw.lcd.double.diagonalSize", &product.screen_diagonal);
            ini.insert("hw.lcd.double.height", &product.screen_height);
            ini.insert("hw.lcd.double.width", &product.screen_width);
        }

        ini.insert("hw.lcd.phy.height", emulator.physical_height.to_string());
        ini.insert("hw.lcd.phy.width", emulator.physical_width.to_string());
        ini.insert(
            "hw.lcd.number",
            if emulator.is_triple_fold() {
                "3"
            } else if emulator.is_fold_like() {
                "2"
            } else {
                "1"
            },
        );
        ini.insert("hw.ramSize", &entry.memory_ram_size);
        ini.insert("hw.dataPartitionSize", &entry.data_disk_size);
        ini.insert(
            "isCustomize",
            if spec.screen_preset.is_customized() {
                "true"
            } else {
                "false"
            },
        );
        ini.insert("hw.hdc.port", "notset");

        ConfigIni::new(PathBuf::from(&entry.path).join("config.ini"), ini)
    }

    fn build_named_ini(&self, entry: &ListsEntry) -> NamedIni {
        let mut ini = FlatIni::new();
        ini.insert("hvd.ini.encoding", "UTF-8");
        ini.insert("path", &entry.path);
        NamedIni::new(
            self.resolved_paths()
                .deployed_path
                .join(format!("{}.ini", entry.name)),
            ini,
        )
    }
}

#[derive(Debug, Clone)]
pub struct ScreenCustomization {
    pub config_name: String,
    pub diagonal_size: f64,
    pub resolution_width: u32,
    pub resolution_height: u32,
    pub density: u32,
}

#[derive(Debug, Clone)]
pub struct FoldableScreenCustomization {
    pub cover_resolution_width: u32,
    pub cover_resolution_height: u32,
    pub cover_diagonal_size: f64,
}

#[derive(Debug, Clone)]
pub struct ScreenPreset {
    pub(crate) emulator_device: EmulatorDevice,
    pub(crate) product_config: ProductConfigItem,
    pub(crate) customization: Option<ScreenCustomization>,
    pub(crate) foldable_customization: Option<FoldableScreenCustomization>,
}

impl ScreenPreset {
    pub fn new(emulator_device: EmulatorDevice, product_config: ProductConfigItem) -> Self {
        Self {
            emulator_device,
            product_config,
            customization: None,
            foldable_customization: None,
        }
    }

    pub fn with_customization(mut self, customization: ScreenCustomization) -> Self {
        self.customization = Some(customization);
        self
    }

    pub fn with_foldable_customization(
        mut self,
        customization: FoldableScreenCustomization,
    ) -> Self {
        self.foldable_customization = Some(customization);
        self
    }

    pub fn emulator_device(&self) -> &EmulatorDevice {
        &self.emulator_device
    }

    pub fn product_config(&self) -> &ProductConfigItem {
        &self.product_config
    }

    pub fn is_customized(&self) -> bool {
        self.product_config.name == "Customize" && self.customization.is_some()
    }
}

#[derive(Debug, Clone)]
pub struct DeviceSpec {
    pub name: String,
    pub cpu_cores: u32,
    pub memory_ram_mb: u32,
    pub data_disk_mb: u32,
    pub screen_preset: ScreenPreset,
    pub vendor_country: String,
    pub is_public: bool,
}

impl DeviceSpec {
    pub fn new(
        name: impl Into<String>,
        cpu_cores: u32,
        memory_ram_mb: u32,
        data_disk_mb: u32,
        screen_preset: ScreenPreset,
    ) -> Self {
        Self {
            name: name.into(),
            cpu_cores,
            memory_ram_mb,
            data_disk_mb,
            screen_preset,
            vendor_country: "CN".to_string(),
            is_public: true,
        }
    }

    pub fn with_vendor_country(mut self, vendor_country: impl Into<String>) -> Self {
        self.vendor_country = vendor_country.into();
        self
    }

    pub fn with_public(mut self, is_public: bool) -> Self {
        self.is_public = is_public;
        self
    }
}

#[derive(Debug, Clone)]
pub struct DeviceCommand {
    pub program: PathBuf,
    pub args: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Device {
    manager: ImageManager,
    entry: ListsEntry,
    screen_preset: ScreenPreset,
    config_ini: ConfigIni,
    named_ini: NamedIni,
}

impl Device {
    pub(crate) fn new(
        manager: ImageManager,
        entry: ListsEntry,
        screen_preset: ScreenPreset,
        config_ini: ConfigIni,
        named_ini: NamedIni,
    ) -> Self {
        Self {
            manager,
            entry,
            screen_preset,
            config_ini,
            named_ini,
        }
    }

    pub fn screen_preset(&self) -> &ScreenPreset {
        &self.screen_preset
    }

    pub fn lists_entry(&self) -> &ListsEntry {
        &self.entry
    }

    pub fn config_ini(&self) -> &ConfigIni {
        &self.config_ini
    }

    pub fn named_ini(&self) -> &NamedIni {
        &self.named_ini
    }

    pub fn executable_path(&self) -> PathBuf {
        self.manager.resolved_paths().emulator_path.join(
            if self.manager.platform() == Platform::Windows {
                "Emulator.exe"
            } else {
                "Emulator"
            },
        )
    }

    pub fn snapshot_path(&self) -> PathBuf {
        self.manager
            .resolved_paths()
            .deployed_path
            .join(&self.entry.name)
            .join("Snapshot.png")
    }

    pub async fn snapshot_base64(&self) -> Result<String, ImageManagerError> {
        let path = self.snapshot_path();
        let bytes = tokio::fs::read(&path)
            .await
            .map_err(|source| ImageManagerError::Io { path, source })?;
        Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
    }

    pub fn start_command(&self) -> DeviceCommand {
        DeviceCommand {
            program: self.executable_path(),
            args: vec![
                "-hvd".to_string(),
                self.entry.name.clone(),
                "-path".to_string(),
                self.manager
                    .resolved_paths()
                    .deployed_path
                    .to_string_lossy()
                    .to_string(),
                "-imageRoot".to_string(),
                self.manager
                    .resolved_paths()
                    .image_base_path
                    .to_string_lossy()
                    .to_string(),
            ],
        }
    }

    pub fn stop_command(&self) -> DeviceCommand {
        DeviceCommand {
            program: self.executable_path(),
            args: vec!["-stop".to_string(), self.entry.name.clone()],
        }
    }

    pub async fn start(&self) -> Result<Child, ImageManagerError> {
        spawn_command(
            self.start_command(),
            self.manager.resolved_paths().emulator_path.clone(),
        )
        .await
    }

    pub async fn stop(&self) -> Result<Child, ImageManagerError> {
        spawn_command(
            self.stop_command(),
            self.manager.resolved_paths().emulator_path.clone(),
        )
        .await
    }

    pub async fn delete(&self) -> Result<(), ImageManagerError> {
        remove_path_if_exists(Path::new(&self.entry.path), true).await?;
        remove_path_if_exists(self.named_ini.path(), false).await?;

        let mut lists = self.manager.read_lists().await?;
        if lists.remove_by_name(&self.entry.name) {
            lists.write().await?;
        }

        Ok(())
    }

    pub async fn storage_size(&self) -> Result<u64, ImageManagerError> {
        let path = PathBuf::from(&self.entry.path);
        if !path.exists() {
            return Ok(0);
        }

        let mut total = 0_u64;
        for entry in WalkDir::new(&path).into_iter().filter_map(Result::ok) {
            if entry.file_type().is_file() {
                total += entry.metadata().map(|metadata| metadata.len()).unwrap_or(0);
            }
        }
        Ok(total)
    }
}

impl ResolvedPaths {
    fn resolve(
        platform: Platform,
        options: ImageManagerOptions,
    ) -> Result<Self, ImageManagerError> {
        let home = home_dir()?;

        let image_base_path = options
            .image_base_path
            .unwrap_or_else(|| default_image_base_path(platform, &home));
        let deployed_path = options
            .deployed_path
            .unwrap_or_else(|| default_deployed_path(platform, &home));
        let cache_path = options
            .cache_path
            .unwrap_or_else(|| image_base_path.join("cache"));
        let sdk_path = options
            .sdk_path
            .unwrap_or_else(|| default_sdk_path(platform, &home));
        let config_path = options
            .config_path
            .unwrap_or_else(|| default_config_path(platform, &home));
        let log_path = options
            .log_path
            .unwrap_or_else(|| default_log_path(platform, &home));
        let emulator_path = options
            .emulator_path
            .unwrap_or_else(|| default_emulator_path(platform, &home));
        let default_sdk_root = infer_default_sdk_root(&sdk_path);

        Ok(Self {
            image_base_path,
            deployed_path,
            cache_path,
            sdk_path,
            default_sdk_root,
            config_path,
            log_path,
            emulator_path,
        })
    }
}

fn home_dir() -> Result<PathBuf, ImageManagerError> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .ok_or(ImageManagerError::HomeDirectoryUnavailable)
}

fn default_image_base_path(platform: Platform, home: &Path) -> PathBuf {
    match platform {
        Platform::Windows => std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join("AppData").join("Local"))
            .join("Huawei")
            .join("Sdk"),
        Platform::Mac => home.join("Library").join("Huawei").join("Sdk"),
        Platform::Linux => home.join(".Huawei").join("Sdk"),
    }
}

fn default_deployed_path(platform: Platform, home: &Path) -> PathBuf {
    match platform {
        Platform::Windows => std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join("AppData").join("Local"))
            .join("Huawei")
            .join("Emulator")
            .join("deployed"),
        Platform::Mac | Platform::Linux => home.join(".Huawei").join("Emulator").join("deployed"),
    }
}

fn default_sdk_path(platform: Platform, home: &Path) -> PathBuf {
    match platform {
        Platform::Mac => {
            PathBuf::from("/Applications/DevEco-Studio.app/Contents/sdk/default/openharmony")
        }
        Platform::Windows => {
            PathBuf::from(r"C:\Program Files\Huawei\DevEco Studio\sdk\default\openharmony")
        }
        Platform::Linux => home
            .join(".Huawei")
            .join("Sdk")
            .join("default")
            .join("openharmony"),
    }
}

fn default_config_path(platform: Platform, home: &Path) -> PathBuf {
    match platform {
        Platform::Mac => home
            .join("Library")
            .join("Application Support")
            .join("Huawei")
            .join("DevEcoStudio6.0"),
        Platform::Windows => std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join("AppData").join("Roaming"))
            .join("Huawei")
            .join("DevEcoStudio6.0"),
        Platform::Linux => home.join(".Huawei").join("DevEcoStudio6.0"),
    }
}

fn default_log_path(platform: Platform, home: &Path) -> PathBuf {
    match platform {
        Platform::Mac => home
            .join("Library")
            .join("Logs")
            .join("Huawei")
            .join("DevEcoStudio6.0"),
        Platform::Windows => std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join("AppData").join("Local"))
            .join("Huawei")
            .join("DevEcoStudio6.0")
            .join("log"),
        Platform::Linux => home.join(".Huawei").join("DevEcoStudio6.0").join("log"),
    }
}

fn default_emulator_path(platform: Platform, home: &Path) -> PathBuf {
    match platform {
        Platform::Mac => PathBuf::from("/Applications/DevEco-Studio.app/Contents/tools/emulator"),
        Platform::Windows => PathBuf::from(r"C:\Program Files\Huawei\DevEco Studio\tools\emulator"),
        Platform::Linux => home.join(".Huawei").join("Emulator"),
    }
}

fn infer_default_sdk_root(path: &Path) -> Option<PathBuf> {
    let is_dir = std::fs::metadata(path)
        .map(|metadata| metadata.is_dir())
        .unwrap_or(false);
    if !is_dir {
        return None;
    }

    let parent = path.parent()?;
    let grand_parent = parent.parent()?;
    Some(grand_parent.to_path_buf())
}

fn parse_semver_tuple(version: &str) -> (u32, u32, u32) {
    let mut parts = version
        .split('.')
        .map(|part| part.parse::<u32>().unwrap_or_default());
    (
        parts.next().unwrap_or_default(),
        parts.next().unwrap_or_default(),
        parts.next().unwrap_or_default(),
    )
}

async fn read_json_file<T: DeserializeOwned>(path: &Path) -> Result<T, ImageManagerError> {
    let contents =
        tokio::fs::read_to_string(path)
            .await
            .map_err(|source| ImageManagerError::Io {
                path: path.to_path_buf(),
                source,
            })?;
    serde_json::from_str(&contents).map_err(|source| ImageManagerError::Json {
        path: path.to_path_buf(),
        source,
    })
}

async fn read_optional_string(path: &Path) -> Result<Option<String>, ImageManagerError> {
    match tokio::fs::read_to_string(path).await {
        Ok(contents) => Ok(Some(contents)),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(ImageManagerError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

async fn decode_remote_response<T: DeserializeOwned>(
    response: reqwest::Response,
    endpoint: &'static str,
) -> Result<T, ImageManagerError> {
    let status = response.status();
    if status.is_success() {
        return response.json::<T>().await.map_err(ImageManagerError::Http);
    }

    let body = response.text().await.ok();
    Err(RemoteApiError {
        endpoint,
        status: Some(status.as_u16()),
        body: body.clone(),
        message: remote_error_message(endpoint, status, body.as_deref()),
    }
    .into())
}

fn remote_error_message(endpoint: &str, status: StatusCode, body: Option<&str>) -> String {
    match body {
        Some(body) if !body.trim().is_empty() => {
            format!(
                "{endpoint} failed with status {}: {}",
                status.as_u16(),
                body.trim()
            )
        }
        _ => format!("{endpoint} failed with status {}", status.as_u16()),
    }
}

fn clone_static_error(error: &ImageManagerError) -> ImageManagerError {
    match error {
        ImageManagerError::DefaultConfig { label, message } => ImageManagerError::DefaultConfig {
            label,
            message: message.clone(),
        },
        ImageManagerError::Json5 { label, source } => ImageManagerError::DefaultConfig {
            label,
            message: source.to_string(),
        },
        other => ImageManagerError::Validation(other.to_string()),
    }
}

async fn spawn_command(
    command: DeviceCommand,
    current_dir: PathBuf,
) -> Result<Child, ImageManagerError> {
    tokio::fs::create_dir_all(&current_dir)
        .await
        .map_err(|source| ImageManagerError::Io {
            path: current_dir.clone(),
            source,
        })?;
    Command::new(&command.program)
        .args(&command.args)
        .current_dir(current_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|source| ImageManagerError::Process {
            program: command.program,
            source,
        })
}

async fn remove_path_if_exists(path: &Path, recursive: bool) -> Result<(), ImageManagerError> {
    let result = if recursive {
        tokio::fs::remove_dir_all(path).await
    } else {
        tokio::fs::remove_file(path).await
    };

    match result {
        Ok(()) => Ok(()),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(ImageManagerError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}
