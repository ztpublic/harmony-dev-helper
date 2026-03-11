# sdk-manager-rs

Internal async Rust crate for OpenHarmony SDK download and extraction.

## Scope

Implemented:

- Static OpenHarmony SDK URL resolution for supported API versions
- Resumable archive download with HTTP `Range`
- Remote SHA256 lookup and checksum verification
- Safe `tar.gz` staging plus nested zip extraction into the final target
- Unix executable-bit preservation and zip symlink restoration on Unix hosts
- Cleanup of cache artifacts after installation

## Minimal Example

```rust
use sdk_manager_rs::{SdkArch, SdkInstallOptions, SdkManager, SdkSource, SdkVersion};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manager = SdkManager::new(Default::default())?;
    let downloader = manager.create_downloader(SdkInstallOptions::new(
        SdkSource::Release {
            version: SdkVersion::Api20,
            arch: SdkArch::current(),
            os: sdk_manager_rs::SdkOs::current(),
        },
        ".cache/sdk",
        "target/sdk",
    ))?;

    downloader.install_without_progress().await?;
    Ok(())
}
```
