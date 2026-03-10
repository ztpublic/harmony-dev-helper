use std::fmt;
use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use serde::de::{Error as DeError, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::ImageManagerError;
use crate::ini::FlatIni;
use crate::manager::{Device, DeviceSpec, ImageManager};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ProductDeviceType {
    Phone,
    Tablet,
    TwoInOne,
    Foldable,
    WideFold,
    TripleFold,
    TwoInOneFoldable,
    Tv,
    Wearable,
    WearableKid,
    Other(String),
}

impl ProductDeviceType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Phone => "Phone",
            Self::Tablet => "Tablet",
            Self::TwoInOne => "2in1",
            Self::Foldable => "Foldable",
            Self::WideFold => "WideFold",
            Self::TripleFold => "TripleFold",
            Self::TwoInOneFoldable => "2in1 Foldable",
            Self::Tv => "TV",
            Self::Wearable => "Wearable",
            Self::WearableKid => "WearableKid",
            Self::Other(value) => value.as_str(),
        }
    }

    pub fn from_section_key(value: &str) -> Self {
        match value {
            "Phone" => Self::Phone,
            "Tablet" => Self::Tablet,
            "2in1" => Self::TwoInOne,
            "Foldable" => Self::Foldable,
            "WideFold" => Self::WideFold,
            "TripleFold" => Self::TripleFold,
            "2in1 Foldable" => Self::TwoInOneFoldable,
            "TV" => Self::Tv,
            "Wearable" => Self::Wearable,
            "WearableKid" => Self::WearableKid,
            other => Self::Other(other.to_string()),
        }
    }

    pub fn from_lists_type(value: &str) -> Self {
        match value {
            "phone" => Self::Phone,
            "tablet" => Self::Tablet,
            "2in1" => Self::TwoInOne,
            "foldable" => Self::Foldable,
            "widefold" => Self::WideFold,
            "triplefold" => Self::TripleFold,
            "2in1_foldable" => Self::TwoInOneFoldable,
            "tv" => Self::Tv,
            "wearable" => Self::Wearable,
            other => Self::Other(other.replace('_', " ")),
        }
    }

    pub fn dev_model(&self) -> Option<&'static str> {
        match self {
            Self::Phone | Self::TwoInOne => Some("PHEMU-FD00"),
            Self::Foldable => Some("PHEMU-FD01"),
            Self::WideFold => Some("PHEMU-FD02"),
            Self::TripleFold => Some("PHEMU-FD06"),
            Self::TwoInOneFoldable => Some("PCEMU-FD05"),
            Self::Wearable => Some("MCHEMU-AL00CN"),
            _ => None,
        }
    }
}

impl Default for ProductDeviceType {
    fn default() -> Self {
        Self::Other(String::new())
    }
}

impl Serialize for ProductDeviceType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ProductDeviceType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ProductDeviceTypeVisitor;

        impl<'de> Visitor<'de> for ProductDeviceTypeVisitor {
            type Value = ProductDeviceType;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a product device type string")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                Ok(ProductDeviceType::from_section_key(value))
            }
        }

        deserializer.deserialize_str(ProductDeviceTypeVisitor)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EmulatorDeviceType {
    Phone,
    Tablet,
    TwoInOne,
    Foldable,
    WideFold,
    TripleFold,
    TwoInOneFoldable,
    Tv,
    Wearable,
    PhoneAll,
    PcAll,
    Other(String),
}

impl EmulatorDeviceType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Phone => "phone",
            Self::Tablet => "tablet",
            Self::TwoInOne => "2in1",
            Self::Foldable => "foldable",
            Self::WideFold => "widefold",
            Self::TripleFold => "triplefold",
            Self::TwoInOneFoldable => "2in1_foldable",
            Self::Tv => "tv",
            Self::Wearable => "wearable",
            Self::PhoneAll => "phone_all",
            Self::PcAll => "pc_all",
            Self::Other(value) => value.as_str(),
        }
    }

    pub fn from_str(value: &str) -> Self {
        match value {
            "phone" => Self::Phone,
            "tablet" => Self::Tablet,
            "2in1" => Self::TwoInOne,
            "foldable" => Self::Foldable,
            "widefold" => Self::WideFold,
            "triplefold" => Self::TripleFold,
            "2in1_foldable" => Self::TwoInOneFoldable,
            "tv" => Self::Tv,
            "wearable" => Self::Wearable,
            "phone_all" => Self::PhoneAll,
            "pc_all" => Self::PcAll,
            other => Self::Other(other.to_string()),
        }
    }

    pub fn is_group(&self) -> bool {
        matches!(self, Self::PhoneAll | Self::PcAll)
    }
}

impl Serialize for EmulatorDeviceType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for EmulatorDeviceType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct EmulatorDeviceTypeVisitor;

        impl<'de> Visitor<'de> for EmulatorDeviceTypeVisitor {
            type Value = EmulatorDeviceType;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an emulator device type string")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                Ok(EmulatorDeviceType::from_str(value))
            }
        }

        deserializer.deserialize_str(EmulatorDeviceTypeVisitor)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkPkgFile {
    pub data: SdkPkgData,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SdkPkgData {
    #[serde(rename = "apiVersion", default)]
    pub api_version: String,
    #[serde(rename = "displayName", default)]
    pub display_name: String,
    #[serde(default)]
    pub path: String,
    #[serde(rename = "platformVersion", default)]
    pub platform_version: String,
    #[serde(rename = "releaseType", default)]
    pub release_type: String,
    #[serde(default)]
    pub version: String,
    #[serde(rename = "guestVersion", default)]
    pub guest_version: String,
    #[serde(default)]
    pub stage: String,
    #[serde(flatten)]
    pub extra: IndexMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InfoFile {
    #[serde(rename = "apiVersion", default)]
    pub api_version: String,
    #[serde(default)]
    pub abi: String,
    #[serde(default)]
    pub version: String,
    #[serde(flatten)]
    pub extra: IndexMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteImageSdk {
    pub path: String,
    #[serde(rename = "apiVersion", default)]
    pub api_version: String,
    #[serde(default)]
    pub license: String,
    #[serde(default)]
    pub version: String,
    #[serde(rename = "displayName", default)]
    pub display_name: String,
    #[serde(default)]
    pub description: String,
    #[serde(rename = "experimentalFlag", default)]
    pub experimental_flag: String,
    #[serde(rename = "releaseType", default)]
    pub release_type: String,
    #[serde(rename = "metaVersion", default)]
    pub meta_version: String,
    #[serde(default)]
    pub archive: Option<RemoteImageArchive>,
    #[serde(flatten)]
    pub extra: IndexMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteImageArchive {
    pub complete: Option<RemoteImageArchiveComplete>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteImageArchiveComplete {
    pub size: String,
    pub checksum: String,
    #[serde(rename = "osArch")]
    pub os_arch: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RemoteImageDownloadResponse {
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub code: Option<i64>,
    #[serde(flatten)]
    pub extra: IndexMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListsEntry {
    #[serde(rename = "diagonalSize")]
    pub diagonal_size: String,
    pub density: String,
    #[serde(rename = "resolutionHeight")]
    pub resolution_height: String,
    #[serde(rename = "resolutionWidth")]
    pub resolution_width: String,
    #[serde(rename = "memoryRamSize")]
    pub memory_ram_size: String,
    #[serde(rename = "cpuNumber")]
    pub cpu_number: String,
    #[serde(rename = "dataDiskSize")]
    pub data_disk_size: String,
    pub name: String,
    pub uuid: String,
    #[serde(rename = "hw.apiName")]
    pub hw_api_name: String,
    #[serde(rename = "devModel", default)]
    pub dev_model: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(rename = "imageDir")]
    pub image_dir: String,
    pub version: String,
    #[serde(rename = "type")]
    pub device_type: String,
    pub abi: String,
    #[serde(rename = "apiVersion")]
    pub api_version: String,
    pub path: String,
    #[serde(rename = "showVersion")]
    pub show_version: String,
    #[serde(rename = "harmonyOSVersion", default)]
    pub harmony_os_version: Option<String>,
    #[serde(rename = "guestVersion", default)]
    pub guest_version: Option<String>,
    #[serde(rename = "coverResolutionWidth", default)]
    pub cover_resolution_width: Option<String>,
    #[serde(rename = "coverResolutionHeight", default)]
    pub cover_resolution_height: Option<String>,
    #[serde(rename = "coverDiagonalSize", default)]
    pub cover_diagonal_size: Option<String>,
    #[serde(rename = "harmonyos.sdk.path")]
    pub harmony_sdk_path: String,
    #[serde(rename = "harmonyos.config.path")]
    pub harmony_config_path: String,
    #[serde(rename = "harmonyos.log.path")]
    pub harmony_log_path: String,
    #[serde(flatten)]
    pub extra: IndexMap<String, serde_json::Value>,
}

impl ListsEntry {
    pub fn instance_path(&self) -> PathBuf {
        PathBuf::from(&self.path)
    }
}

#[derive(Debug, Clone)]
pub struct ListsFile {
    path: PathBuf,
    entries: Vec<ListsEntry>,
}

impl ListsFile {
    pub fn empty(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            entries: Vec::new(),
        }
    }

    pub fn from_entries(path: impl Into<PathBuf>, entries: Vec<ListsEntry>) -> Self {
        Self {
            path: path.into(),
            entries,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn entries(&self) -> &[ListsEntry] {
        &self.entries
    }

    pub fn entries_mut(&mut self) -> &mut Vec<ListsEntry> {
        &mut self.entries
    }

    pub fn upsert(&mut self, entry: ListsEntry) {
        self.entries
            .retain(|existing| existing.uuid != entry.uuid && existing.name != entry.name);
        self.entries.push(entry);
    }

    pub fn remove_by_name(&mut self, name: &str) -> bool {
        let before = self.entries.len();
        self.entries.retain(|entry| entry.name != name);
        before != self.entries.len()
    }

    pub fn to_pretty_json(&self) -> Result<String, ImageManagerError> {
        serde_json::to_string_pretty(&self.entries).map_err(|source| ImageManagerError::Json {
            path: self.path.clone(),
            source,
        })
    }

    pub async fn write(&self) -> Result<(), ImageManagerError> {
        ensure_parent_dir(&self.path).await?;
        let json = self.to_pretty_json()?;
        tokio::fs::write(&self.path, format!("{json}\n"))
            .await
            .map_err(|source| ImageManagerError::Io {
                path: self.path.clone(),
                source,
            })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductConfigItem {
    #[serde(skip)]
    pub device_type: ProductDeviceType,
    pub name: String,
    #[serde(rename = "screenWidth")]
    pub screen_width: String,
    #[serde(rename = "screenHeight")]
    pub screen_height: String,
    #[serde(rename = "screenDiagonal")]
    pub screen_diagonal: String,
    #[serde(rename = "screenDensity")]
    pub screen_density: String,
    pub visible: bool,
    #[serde(rename = "oneCutoutPath", default)]
    pub one_cutout_path: Option<String>,
    #[serde(rename = "outerScreenWidth", default)]
    pub outer_screen_width: Option<String>,
    #[serde(rename = "outerScreenHeight", default)]
    pub outer_screen_height: Option<String>,
    #[serde(rename = "outerScreenDiagonal", default)]
    pub outer_screen_diagonal: Option<String>,
    #[serde(rename = "outerDoubleScreenWidth", default)]
    pub outer_double_screen_width: Option<String>,
    #[serde(rename = "outerDoubleScreenHeight", default)]
    pub outer_double_screen_height: Option<String>,
    #[serde(rename = "outerDoubleScreenDiagonal", default)]
    pub outer_double_screen_diagonal: Option<String>,
    #[serde(flatten)]
    pub extra: IndexMap<String, serde_json::Value>,
}

impl ProductConfigItem {
    pub fn with_device_type(mut self, device_type: ProductDeviceType) -> Self {
        self.device_type = device_type;
        self
    }

    pub fn dev_model(&self) -> Option<&'static str> {
        self.device_type.dev_model()
    }

    pub fn is_fold_like(&self) -> bool {
        matches!(
            self.device_type,
            ProductDeviceType::Foldable
                | ProductDeviceType::WideFold
                | ProductDeviceType::TripleFold
                | ProductDeviceType::TwoInOneFoldable
        )
    }
}

#[derive(Debug, Clone)]
pub struct ProductCatalog {
    sections: IndexMap<ProductDeviceType, Vec<ProductConfigItem>>,
}

impl ProductCatalog {
    pub fn from_sections(sections: IndexMap<String, Vec<ProductConfigItem>>) -> Self {
        let sections = sections
            .into_iter()
            .map(|(key, items)| {
                let device_type = ProductDeviceType::from_section_key(&key);
                let items = items
                    .into_iter()
                    .map(|item| item.with_device_type(device_type.clone()))
                    .collect();
                (device_type, items)
            })
            .collect();
        Self { sections }
    }

    pub fn sections(&self) -> &IndexMap<ProductDeviceType, Vec<ProductConfigItem>> {
        &self.sections
    }

    pub fn find_items(
        &self,
        device_type: Option<&ProductDeviceType>,
        name: Option<&str>,
    ) -> Vec<&ProductConfigItem> {
        let mut items = Vec::new();
        for (section_type, section_items) in &self.sections {
            if let Some(expected) = device_type {
                if section_type != expected {
                    continue;
                }
            }
            for item in section_items {
                if let Some(expected_name) = name {
                    if item.name != expected_name {
                        continue;
                    }
                }
                items.push(item);
            }
        }
        items
    }

    pub fn find_item(
        &self,
        device_type: Option<&ProductDeviceType>,
        name: Option<&str>,
    ) -> Option<&ProductConfigItem> {
        self.find_items(device_type, name).into_iter().next()
    }

    pub fn to_pretty_json(&self) -> Result<String, ImageManagerError> {
        let sections: IndexMap<String, Vec<ProductConfigItem>> = self
            .sections
            .iter()
            .map(|(device_type, items)| {
                let items = items
                    .iter()
                    .cloned()
                    .map(|mut item| {
                        item.device_type = ProductDeviceType::Other(String::new());
                        item
                    })
                    .collect();
                (device_type.as_str().to_string(), items)
            })
            .collect();
        serde_json::to_string_pretty(&sections).map_err(|source| ImageManagerError::Json {
            path: PathBuf::from("product-config.json"),
            source,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmulatorDevice {
    pub name: String,
    #[serde(rename = "deviceType")]
    pub device_type: EmulatorDeviceType,
    #[serde(rename = "resolutionWidth")]
    pub resolution_width: u32,
    #[serde(rename = "resolutionHeight")]
    pub resolution_height: u32,
    #[serde(rename = "physicalWidth")]
    pub physical_width: u32,
    #[serde(rename = "physicalHeight")]
    pub physical_height: u32,
    #[serde(rename = "diagonalSize")]
    pub diagonal_size: f64,
    pub density: u32,
    #[serde(rename = "memoryRamSize")]
    pub memory_ram_size: u32,
    #[serde(rename = "datadiskSize")]
    pub data_disk_size: u32,
    #[serde(rename = "procNumber")]
    pub proc_number: u32,
    pub api: u32,
    #[serde(rename = "coverResolutionWidth", default)]
    pub cover_resolution_width: Option<u32>,
    #[serde(rename = "coverResolutionHeight", default)]
    pub cover_resolution_height: Option<u32>,
    #[serde(rename = "coverDiagonalSize", default)]
    pub cover_diagonal_size: Option<f64>,
    #[serde(rename = "singleResolutionWidth", default)]
    pub single_resolution_width: Option<u32>,
    #[serde(rename = "singleResolutionHeight", default)]
    pub single_resolution_height: Option<u32>,
    #[serde(rename = "singleDiagonalSize", default)]
    pub single_diagonal_size: Option<f64>,
    #[serde(rename = "doubleResolutionWidth", default)]
    pub double_resolution_width: Option<u32>,
    #[serde(rename = "doubleResolutionHeight", default)]
    pub double_resolution_height: Option<u32>,
    #[serde(rename = "doubleDiagonalSize", default)]
    pub double_diagonal_size: Option<f64>,
    #[serde(flatten)]
    pub extra: IndexMap<String, serde_json::Value>,
}

impl EmulatorDevice {
    pub fn is_fold_like(&self) -> bool {
        matches!(
            self.device_type,
            EmulatorDeviceType::Foldable
                | EmulatorDeviceType::WideFold
                | EmulatorDeviceType::TripleFold
                | EmulatorDeviceType::TwoInOneFoldable
        )
    }

    pub fn is_triple_fold(&self) -> bool {
        matches!(self.device_type, EmulatorDeviceType::TripleFold)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmulatorGroup {
    pub name: String,
    #[serde(rename = "deviceType")]
    pub device_type: EmulatorDeviceType,
    pub api: u32,
    pub children: Vec<EmulatorDevice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EmulatorEntry {
    Group(EmulatorGroup),
    Device(EmulatorDevice),
}

#[derive(Debug, Clone)]
pub struct EmulatorCatalog {
    entries: Vec<EmulatorEntry>,
    devices: Vec<EmulatorDevice>,
}

impl EmulatorCatalog {
    pub fn new(entries: Vec<EmulatorEntry>) -> Self {
        let mut devices = Vec::new();
        for entry in &entries {
            match entry {
                EmulatorEntry::Group(group) => devices.extend(group.children.clone()),
                EmulatorEntry::Device(device) => devices.push(device.clone()),
            }
        }
        Self { entries, devices }
    }

    pub fn entries(&self) -> &[EmulatorEntry] {
        &self.entries
    }

    pub fn devices(&self) -> &[EmulatorDevice] {
        &self.devices
    }

    pub fn find_device(
        &self,
        api_version: Option<u32>,
        device_type: Option<&EmulatorDeviceType>,
    ) -> Option<&EmulatorDevice> {
        self.devices.iter().find(|device| {
            if let Some(api) = api_version {
                if device.api != api {
                    return false;
                }
            }
            if let Some(expected) = device_type {
                if &device.device_type != expected {
                    return false;
                }
            }
            true
        })
    }

    pub fn find_remote_image<'a>(
        &self,
        device: &EmulatorDevice,
        remote_images: &'a [RemoteImage],
    ) -> Option<&'a RemoteImage> {
        remote_images.iter().find(|image| {
            if image.api_version() != device.api {
                return false;
            }

            let image_type = image.device_type();
            if image_type == device.device_type {
                return true;
            }

            matches!(
                (&image_type, &device.device_type),
                (EmulatorDeviceType::PcAll, EmulatorDeviceType::TwoInOne)
                    | (
                        EmulatorDeviceType::PcAll,
                        EmulatorDeviceType::TwoInOneFoldable
                    )
                    | (EmulatorDeviceType::PhoneAll, EmulatorDeviceType::Phone)
                    | (EmulatorDeviceType::PhoneAll, EmulatorDeviceType::Foldable)
                    | (EmulatorDeviceType::PhoneAll, EmulatorDeviceType::WideFold)
                    | (EmulatorDeviceType::PhoneAll, EmulatorDeviceType::TripleFold)
            )
        })
    }

    pub fn to_pretty_json(&self) -> Result<String, ImageManagerError> {
        serde_json::to_string_pretty(&self.entries).map_err(|source| ImageManagerError::Json {
            path: PathBuf::from("emulator.json"),
            source,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ConfigIni {
    path: PathBuf,
    contents: FlatIni,
}

impl ConfigIni {
    pub fn new(path: impl Into<PathBuf>, contents: FlatIni) -> Self {
        Self {
            path: path.into(),
            contents,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn contents(&self) -> &FlatIni {
        &self.contents
    }

    pub async fn write(&self) -> Result<(), ImageManagerError> {
        self.contents.write_to_path(&self.path).await
    }
}

#[derive(Debug, Clone)]
pub struct NamedIni {
    path: PathBuf,
    contents: FlatIni,
}

impl NamedIni {
    pub fn new(path: impl Into<PathBuf>, contents: FlatIni) -> Self {
        Self {
            path: path.into(),
            contents,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn contents(&self) -> &FlatIni {
        &self.contents
    }

    pub async fn write(&self) -> Result<(), ImageManagerError> {
        self.contents.write_to_path(&self.path).await
    }
}

#[derive(Debug, Clone)]
pub struct LocalImage {
    pub(crate) manager: ImageManager,
    pub(crate) sdk_pkg: SdkPkgFile,
    pub(crate) info: InfoFile,
    pub(crate) relative_path: PathBuf,
    pub(crate) api_version: u32,
}

impl LocalImage {
    pub fn sdk_pkg(&self) -> &SdkPkgFile {
        &self.sdk_pkg
    }

    pub fn info(&self) -> &InfoFile {
        &self.info
    }

    pub fn relative_path(&self) -> &Path {
        &self.relative_path
    }

    pub fn full_path(&self) -> PathBuf {
        self.manager
            .resolved_paths()
            .image_base_path
            .join(&self.relative_path)
    }

    pub fn api_version(&self) -> u32 {
        self.api_version
    }

    pub fn device_type(&self) -> EmulatorDeviceType {
        let display_name = self.sdk_pkg.data.display_name.as_str();
        let value = display_name.rsplit('-').next().unwrap_or_default();
        EmulatorDeviceType::from_str(value)
    }

    pub async fn create_device(&self, spec: DeviceSpec) -> Result<Device, ImageManagerError> {
        self.manager
            .create_device_from_local_image(self, spec)
            .await
    }
}

#[derive(Debug, Clone)]
pub struct RemoteImage {
    pub(crate) manager: ImageManager,
    pub(crate) sdk: RemoteImageSdk,
    pub(crate) relative_path: PathBuf,
    pub(crate) api_version: u32,
}

impl RemoteImage {
    pub fn sdk(&self) -> &RemoteImageSdk {
        &self.sdk
    }

    pub fn relative_path(&self) -> &Path {
        &self.relative_path
    }

    pub fn full_path(&self) -> PathBuf {
        self.manager
            .resolved_paths()
            .image_base_path
            .join(&self.relative_path)
    }

    pub fn api_version(&self) -> u32 {
        self.api_version
    }

    pub fn device_type(&self) -> EmulatorDeviceType {
        let display_name = self.sdk.display_name.as_str();
        let value = display_name.rsplit('-').next().unwrap_or_default();
        EmulatorDeviceType::from_str(value)
    }

    pub async fn is_downloaded(&self) -> Result<bool, ImageManagerError> {
        Ok(tokio::fs::metadata(self.full_path()).await.is_ok())
    }

    pub async fn local_image(&self) -> Result<Option<LocalImage>, ImageManagerError> {
        let images = self.manager.local_images().await?;
        Ok(images
            .into_iter()
            .find(|image| image.full_path() == self.full_path()))
    }

    pub async fn create_downloader(
        &self,
    ) -> Result<crate::downloader::Downloader, ImageManagerError> {
        self.manager.create_downloader(self).await
    }
}

pub(crate) fn relative_path_from_sdk_path(value: &str) -> PathBuf {
    PathBuf::from(value.replace(',', "/"))
}

pub(crate) fn api_version_from_string(value: &str) -> u32 {
    value.parse().unwrap_or_default()
}

pub(crate) fn trimmed_relative_image_dir(path: &Path) -> String {
    let mut value = path.to_string_lossy().replace('\\', "/");
    if !value.ends_with('/') {
        value.push('/');
    }
    value
}

pub(crate) fn guest_version_prefix(value: &str) -> &str {
    value.split_whitespace().next().unwrap_or(value)
}

pub(crate) async fn ensure_parent_dir(path: &Path) -> Result<(), ImageManagerError> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };

    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|source| ImageManagerError::Io {
            path: parent.to_path_buf(),
            source,
        })
}
