//! Native shutdown must observe blocked requests and transfer cleanup.

use super::*;
use crate::sftp_task::handle_limit_tests::{Gate, scratch, serve_gated_fixture};
use oneterm_core::{RemotePath, SftpBackend as _};
use std::time::Duration;

async fn blocked(operation: &'static str) {
    let root = scratch(operation);
    let local = scratch("native-owned-transfer");
    std::fs::write(root.0.join("source"), vec![7u8; 1024 * 1024]).unwrap();
    std::fs::write(root.0.join("target"), b"original remote").unwrap();
    let target = local.0.join("target");
    std::fs::write(&target, b"original local").unwrap();
    let source = local.0.join("source");
    std::fs::write(&source, vec![9u8; 1024 * 1024]).unwrap();
    let (entered, observed) = async_channel::bounded(1);
    let (release, gate) = async_channel::bounded(1);
    let (client, server) = tokio::io::duplex(1024 * 1024);
    serve_gated_fixture(
        server,
        &root.0,
        Some(Gate {
            operation,
            entered,
            release: gate,
        }),
    )
    .await;
    let (stream, transport) = owned_stream(client);
    let channel =
        russh_sftp::client::SftpSession::new_with_config(stream, crate::session::sftp_config())
            .await
            .unwrap();
    let owner = Owner::spawn(Prepared { channel, transport });
    let retained = owner.backend.clone();
    let mut request = None;
    let transfer = match operation {
        "stat" => {
            let backend = retained.clone();
            request = Some(tokio::spawn(async move {
                backend.stat(RemotePath::new("/source")).await
            }));
            None
        }
        "read" => Some(retained.download(1, RemotePath::new("/source"), target.clone())),
        "write" => Some(retained.upload(2, source, RemotePath::new("/target"))),
        "lookup" => None,
        _ => panic!("unknown gate"),
    };
    tokio::time::timeout(Duration::from_secs(2), observed.recv())
        .await
        .unwrap()
        .unwrap();
    tokio::time::timeout(Duration::from_secs(1), owner.complete())
        .await
        .expect("native cleanup cannot wait for server replies")
        .unwrap();
    assert!(!retained.alive());
    if let Some(request) = request {
        assert!(request.await.unwrap().is_err());
    }
    if let Some(transfer) = transfer {
        assert!(transfer.result.recv().await.unwrap().is_err());
    }
    assert_eq!(std::fs::read(&target).unwrap(), b"original local");
    assert_eq!(
        std::fs::read(root.0.join("target")).unwrap(),
        b"original remote"
    );
    assert!(
        std::fs::read_dir(&local.0).unwrap().all(|entry| !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".part")),
        "cancelled download removes its local staging file"
    );
    drop(release);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn blocked_lookup_is_observed_with_retained_backend() {
    blocked("lookup").await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn blocked_metadata_is_aborted_and_observed() {
    blocked("stat").await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn blocked_download_drains_and_removes_staging() {
    blocked("read").await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn blocked_upload_drains_and_preserves_target() {
    blocked("write").await;
}
