# image-manager-rs

Internal async Rust crate for Harmony emulator image management.

## Scope

Implemented:

- Local image discovery from `system-image/**/sdk-pkg.json`
- Remote image listing from the Harmony SDK endpoints
- Download URL resolution, resumable archive download, checksum verification, and zip extraction
- Default or on-disk `emulator.json` / `product-config.json` loading
- Device creation, deployed-device discovery, config generation, and deletion
- Emulator command construction and process spawning

## Minimal Example

```rust
use image_manager_rs::{DeviceSpec, ImageManager, ImageManagerOptions, ProductDeviceType, ScreenPreset};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manager = ImageManager::new(ImageManagerOptions::default())?;
    let local_images = manager.local_images().await?;

    if let Some(image) = local_images.first() {
        let products = manager.read_product_catalog().await?;
        let emulators = manager.read_emulator_catalog().await?;
        let product = products
            .find_item(Some(&ProductDeviceType::Phone), None)
            .cloned()
            .expect("expected a phone product preset");
        let emulator = emulators
            .find_device(Some(image.api_version()), None)
            .cloned()
            .expect("expected an emulator preset");

        let screen = ScreenPreset::new(emulator, product);
        let _device = image
            .create_device(DeviceSpec::new("Example_Device", 4, 4096, 6144, screen))
            .await?;
    }

    Ok(())
}
```
