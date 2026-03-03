# hdckit-rs

Async Rust HDC client focused on behavioral parity with the existing TypeScript `hdckit` core API.

## Scope

Implemented in v1:

- `Client`: list targets, track targets, list forwards/reverses, kill server
- `Target`: parameters, shell, file send/recv, install/uninstall, forward/reverse, hilog
- Streaming primitives: target tracker and hilog stream

Not implemented in v1 (feature gaps vs `doc/awesome-hdc/README.md`):

- `UiDriver` and high-level UI automation APIs.
- HDC server/session management APIs: version query (`hdc -v`), explicit start/restart (`hdc start -r`), detailed target listing (`hdc list targets -v`), and reboot (`hdc target boot`).
- Wireless debugging/connection management: `tmode port ...`, `tconn ...`, and close-wireless workflows.
- Typed device info helpers built on `param`/`hidumper` output parsing (name/brand/model/version/CPU, resolution, rotation, power/screen state, wlan ip, network status, battery/temperature).
- Application lifecycle APIs beyond install/uninstall: list installed apps, start/stop app, ability/app dump helpers, version extraction, clear cache/data, and debug process tracking (`jpid`/`track-jpid`).
- Full `aa`, `bm`, and `param` tool wrappers (including `aa test`, `aa attach/detach/appdebug`, `bm clean/get/...`, and `param set/wait/dump/save`).
- High-level screenshot/layout/recording APIs: `uitest uiInput`, `uitest screenCap`, `snapshot_display`, `uitest dumpLayout`, and `uiRecord`.
- Screen recording helper API (the upstream CLI notes this area is still evolving).
- Expanded logging/diagnostics APIs: full `hilog` options (filters/buffer/persist/flow-control/baselevel), log/crash export helpers, and `hidumper` service-specific wrappers.
- Performance tooling wrappers (`SmartPerf` / `SP_daemon`) plus structured output parsing.

## API Mapping (Node -> Rust)

- `Hdc.createClient()` -> `Client::from_env()`
- `client.listTargets()` -> `client.list_targets().await`
- `client.trackTargets()` -> `client.track_targets().await`
- `client.getTarget(key)` -> `client.get_target(key)`
- `target.getParameters()` -> `target.get_parameters().await`
- `target.shell(cmd)` -> `target.shell(cmd).await`
- `target.sendFile(local, remote)` -> `target.send_file(local, remote).await`
- `target.recvFile(remote, local)` -> `target.recv_file(remote, local).await`
- `target.install(hap)` -> `target.install(hap).await`
- `target.uninstall(bundle)` -> `target.uninstall(bundle).await`
- `target.openHilog({ clear })` -> `target.open_hilog(clear).await`

## Minimal Example

```rust
use hdckit_rs::Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::from_env();
    let targets = client.list_targets().await?;

    if let Some(first) = targets.first() {
        let target = client.get_target(first.clone())?;
        let params = target.get_parameters().await?;
        println!("{:?}", params.get("const.product.name"));
    }

    Ok(())
}
```
