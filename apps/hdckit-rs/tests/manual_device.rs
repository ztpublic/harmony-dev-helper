use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use hdckit_rs::{Client, ClientOptions, HdcError, Target};
use tempfile::tempdir;
use tokio::time::{sleep, timeout, Instant};

const HDC_BIN: &str =
    "/Applications/DevEco-Studio.app/Contents/sdk/default/openharmony/toolchains/hdc";

fn client() -> Client {
    assert!(
        Path::new(HDC_BIN).exists(),
        "hdc binary not found at expected path: {HDC_BIN}"
    );

    Client::new(ClientOptions {
        host: "127.0.0.1".to_string(),
        port: std::env::var("OHOS_HDC_SERVER_PORT")
            .ok()
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(8710),
        bin: PathBuf::from(HDC_BIN),
    })
}

fn unique_suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards")
        .as_nanos()
}

fn find_free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("failed to bind ephemeral port")
        .local_addr()
        .expect("failed to read local addr")
        .port()
}

async fn first_target(client: &Client) -> Target {
    let deadline = Instant::now() + Duration::from_secs(10);
    let target_key = loop {
        match client.list_targets().await {
            Ok(targets) => match targets.into_iter().next() {
                Some(target) => break target,
                None => panic!("expected at least one connected HarmonyOS device"),
            },
            Err(HdcError::Io(err))
                if err.kind() == std::io::ErrorKind::ConnectionRefused
                    && Instant::now() < deadline =>
            {
                sleep(Duration::from_millis(300)).await;
            }
            Err(err) => panic!("list targets failed: {err:?}"),
        }
    };

    client
        .get_target(target_key)
        .expect("failed to create target from connect key")
}

async fn wait_for_forward_state(
    target: &Target,
    local: &str,
    remote: &str,
    expected_present: bool,
) {
    let deadline = Instant::now() + Duration::from_secs(6);

    loop {
        let found = target
            .list_forwards()
            .await
            .expect("list forwards failed")
            .iter()
            .any(|mapping| mapping.local == local && mapping.remote == remote);

        if found == expected_present {
            return;
        }

        if Instant::now() >= deadline {
            panic!(
                "timed out waiting for forward {local} -> {remote}, expected_present={expected_present}"
            );
        }

        sleep(Duration::from_millis(250)).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn integration_core_parity() {
    let client = client();
    let target = first_target(&client).await;

    let params = target
        .get_parameters()
        .await
        .expect("get parameters failed");
    assert!(!params.is_empty(), "expected non-empty parameter map");

    let token = format!("hdc-lib-it-{}", unique_suffix());
    let mut shell = target
        .shell(&format!("echo {token}"))
        .await
        .expect("shell failed");
    let output = shell.read_all_string().await.expect("shell read failed");
    assert!(
        output.contains(&token),
        "shell output did not contain token: {output:?}"
    );

    let _ = client.list_forwards().await.expect("list forwards failed");
    let _ = client.list_reverses().await.expect("list reverses failed");
}

#[tokio::test(flavor = "multi_thread")]
async fn integration_file_send_recv_roundtrip() {
    let client = client();
    let target = first_target(&client).await;

    let temp = tempdir().expect("create temp dir failed");
    let source_path = temp.path().join("source.txt");
    let roundtrip_path = temp.path().join("roundtrip.txt");
    let content = format!("hdc-lib-roundtrip-{}", unique_suffix());
    std::fs::write(&source_path, &content).expect("write source file failed");

    let remote_path = format!("/data/local/tmp/hdc_lib_it_{}.txt", unique_suffix());
    target
        .send_file(&source_path, &remote_path)
        .await
        .expect("send_file failed");

    target
        .recv_file(&remote_path, &roundtrip_path)
        .await
        .expect("recv_file failed");

    let received = std::fs::read_to_string(&roundtrip_path).expect("read roundtrip file failed");
    assert_eq!(received, content);

    let mut cleanup = target
        .shell(&format!("rm -f {remote_path}"))
        .await
        .expect("open cleanup shell failed");
    let _ = cleanup.read_all().await.expect("cleanup shell read failed");
}

#[tokio::test(flavor = "multi_thread")]
async fn integration_forward_lifecycle() {
    let client = client();
    let target = first_target(&client).await;

    let local = format!("tcp:{}", find_free_port());
    let remote = format!("tcp:{}", find_free_port());

    target
        .forward(&local, &remote)
        .await
        .expect("forward command failed");

    wait_for_forward_state(&target, &local, &remote, true).await;

    target
        .remove_forward(&local, &remote)
        .await
        .expect("remove_forward failed");

    wait_for_forward_state(&target, &local, &remote, false).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn integration_hilog_stream_receives_entry() {
    let client = client();
    let target = first_target(&client).await;

    let mut hilog = target.open_hilog(false).await.expect("open_hilog failed");

    let entry = timeout(Duration::from_secs(15), hilog.next_entry())
        .await
        .expect("timed out waiting for hilog entry")
        .expect("hilog stream ended before producing any entry")
        .expect("failed to parse hilog entry");

    assert!(!entry.tag.is_empty(), "hilog entry tag should not be empty");
    hilog.end();
}
