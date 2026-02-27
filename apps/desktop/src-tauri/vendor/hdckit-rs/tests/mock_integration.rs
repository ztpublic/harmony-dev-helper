use std::path::PathBuf;
use std::time::Duration;

use hdckit_rs::{Client, ClientOptions, TargetEvent};
use tempfile::tempdir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;

async fn write_frame(socket: &mut TcpStream, payload: &[u8]) {
    socket
        .write_all(&(payload.len() as u32).to_be_bytes())
        .await
        .unwrap();
    socket.write_all(payload).await.unwrap();
}

async fn read_frame(socket: &mut TcpStream) -> Option<Vec<u8>> {
    let mut len_buf = [0u8; 4];
    if socket.read_exact(&mut len_buf).await.is_err() {
        return None;
    }

    let len = u32::from_be_bytes(len_buf) as usize;
    let mut payload = vec![0u8; len];
    if socket.read_exact(&mut payload).await.is_err() {
        return None;
    }

    Some(payload)
}

async fn do_handshake(socket: &mut TcpStream) {
    let mut hello = Vec::new();
    hello.extend_from_slice(b"OHOS HDC1234");
    hello.extend_from_slice(&1u32.to_be_bytes());
    write_frame(socket, &hello).await;

    let response = read_frame(socket).await.unwrap();
    assert_eq!(response.len(), 44);
    assert_eq!(&response[0..12], b"OHOS HDC1234");
}

fn client_options(port: u16) -> ClientOptions {
    ClientOptions {
        host: "127.0.0.1".to_string(),
        port,
        bin: PathBuf::from("hdc"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn client_lists_targets_forwards_and_reverses() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    let server = tokio::spawn(async move {
        for i in 0..3 {
            let (mut socket, _) = listener.accept().await.unwrap();
            do_handshake(&mut socket).await;

            let command = String::from_utf8(read_frame(&mut socket).await.unwrap()).unwrap();
            match i {
                0 => {
                    assert_eq!(command, "list targets");
                    write_frame(&mut socket, b"dev1\ndev2\n").await;
                }
                1 => {
                    assert_eq!(command, "fport ls");
                    write_frame(&mut socket, b"dev1 tcp:1000 tcp:2000 Forward\n").await;
                }
                _ => {
                    assert_eq!(command, "fport ls");
                    write_frame(&mut socket, b"dev1 tcp:3000 tcp:4000 Reverse\n").await;
                }
            }
        }
    });

    let client = Client::new(client_options(port));

    let targets = client.list_targets().await.unwrap();
    assert_eq!(targets, vec!["dev1".to_string(), "dev2".to_string()]);

    let forwards = client.list_forwards().await.unwrap();
    assert_eq!(forwards[0].local, "tcp:1000");
    assert_eq!(forwards[0].remote, "tcp:2000");

    let reverses = client.list_reverses().await.unwrap();
    assert_eq!(reverses[0].local, "tcp:4000");
    assert_eq!(reverses[0].remote, "tcp:3000");

    server.await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn target_parameters_and_shell_read_all() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    let server = tokio::spawn(async move {
        for i in 0..3 {
            let (mut socket, _) = listener.accept().await.unwrap();
            do_handshake(&mut socket).await;

            let command = String::from_utf8(read_frame(&mut socket).await.unwrap()).unwrap();
            match i {
                0 => {
                    assert_eq!(command, "shell echo ready\n");
                    write_frame(&mut socket, b"ready").await;
                }
                1 => {
                    assert_eq!(command, "shell param get");
                    write_frame(&mut socket, b"a = b\n").await;
                    write_frame(&mut socket, b"c = d\r\n").await;
                }
                _ => {
                    assert_eq!(command, "shell echo hi");
                    write_frame(&mut socket, b"hello ").await;
                    write_frame(&mut socket, b"world").await;
                }
            }
        }
    });

    let client = Client::new(client_options(port));
    let target = client.get_target("dev1").unwrap();

    let params = target.get_parameters().await.unwrap();
    assert_eq!(params.get("a"), Some(&"b".to_string()));
    assert_eq!(params.get("c"), Some(&"d".to_string()));

    let mut shell = target.shell("echo hi").await.unwrap();
    let output = shell.read_all_string().await.unwrap();
    assert_eq!(output, "hello world");

    server.await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn target_reverse_remove_uses_forward_remove_command_shape() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    let server = tokio::spawn(async move {
        for i in 0..3 {
            let (mut socket, _) = listener.accept().await.unwrap();
            do_handshake(&mut socket).await;

            let command = String::from_utf8(read_frame(&mut socket).await.unwrap()).unwrap();
            match i {
                0 => {
                    assert_eq!(command, "shell echo ready\n");
                    write_frame(&mut socket, b"ready").await;
                }
                1 => {
                    assert_eq!(command, "rport tcp:9222 tcp:9223");
                    write_frame(&mut socket, b"OK").await;
                }
                _ => {
                    assert_eq!(command, "fport rm tcp:9222 tcp:9223");
                    write_frame(&mut socket, b"success").await;
                }
            }
        }
    });

    let client = Client::new(client_options(port));
    let target = client.get_target("dev1").unwrap();

    target.reverse("tcp:9222", "tcp:9223").await.unwrap();
    target.remove_reverse("tcp:9222", "tcp:9223").await.unwrap();

    server.await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn tracker_emits_diff_events() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        do_handshake(&mut socket).await;

        let alive = String::from_utf8(read_frame(&mut socket).await.unwrap()).unwrap();
        assert_eq!(alive, "alive");

        let cmd1 = String::from_utf8(read_frame(&mut socket).await.unwrap()).unwrap();
        assert_eq!(cmd1, "list targets");
        write_frame(&mut socket, b"dev1\n").await;

        let cmd2 = String::from_utf8(read_frame(&mut socket).await.unwrap()).unwrap();
        assert_eq!(cmd2, "list targets");
        write_frame(&mut socket, b"dev1\ndev2\n").await;
    });

    let client = Client::new(client_options(port));
    let mut tracker = client.track_targets().await.unwrap();

    let event1 = timeout(Duration::from_secs(3), tracker.next_event())
        .await
        .unwrap()
        .unwrap();
    match event1 {
        TargetEvent::Added(target) => assert_eq!(target, "dev1"),
        _ => panic!("expected added event"),
    }

    let event2 = timeout(Duration::from_secs(4), tracker.next_event())
        .await
        .unwrap()
        .unwrap();
    match event2 {
        TargetEvent::Added(target) => assert_eq!(target, "dev2"),
        _ => panic!("expected added event"),
    }

    tracker.end();
    server.await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn retries_server_start_once_on_connection_refused() {
    let port = {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap().port()
    };

    let temp = tempdir().unwrap();
    let log_path = temp.path().join("start.log");
    let script_path = temp.path().join("fake-hdc.sh");

    std::fs::write(
        &script_path,
        format!("#!/bin/sh\necho start >> {}\n", log_path.display()),
    )
    .unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&script_path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script_path, perms).unwrap();
    }

    let client = Client::new(ClientOptions {
        host: "127.0.0.1".to_string(),
        port,
        bin: script_path,
    });

    let _ = client.list_targets().await;

    let log = std::fs::read_to_string(log_path).unwrap_or_default();
    assert_eq!(log.lines().count(), 1);
}
