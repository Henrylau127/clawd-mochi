use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;
use tokio::net::TcpListener;
use tokio::sync::mpsc;

use crate::ble::mock::{self, MOCK_DEVICE_NAME};
use crate::bridge::http;
use crate::bridge::protocol::DeskPetEvent;
use crate::bridge::queue::EventQueue;
use crate::state::Shared;

async fn wait_connected(shared: &Shared) {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if shared.is_connected().await {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("mock pet did not connect");
}

fn curl_json(args: &[&str]) -> Value {
    let mut last_stderr = String::new();
    for _ in 0..50 {
        let output = Command::new("curl")
            .args(args)
            .output()
            .expect("curl must be available for hardware-free tests");
        if output.status.success() {
            if let Ok(value) = serde_json::from_slice(&output.stdout) {
                return value;
            }
        }
        last_stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("curl {:?} failed: {last_stderr}", args);
}

async fn spawn_mock_bridge() -> (String, Arc<Shared>) {
    let (tx, rx) = mpsc::channel(32);
    let shared = Arc::new(Shared::new(tx));
    tokio::spawn(mock::run(shared.clone(), rx));
    wait_connected(&shared).await;

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("loopback bind");
    let addr = listener.local_addr().expect("local addr");
    assert!(addr.ip().is_loopback(), "API must bind loopback only");
    tokio::spawn(http::serve_with_listener(shared.clone(), listener));
    let base = format!("http://{addr}");
    let _ = curl_json(&["-sS", &format!("{base}/v1/status")]);
    (base, shared)
}

#[tokio::test]
async fn bind_exact_fails_when_port_in_use() {
    let held = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("hold a loopback port");
    let port = held.local_addr().expect("held addr").port();
    let err = http::bind_exact(port).await.expect_err("port should be busy");
    assert_eq!(err.kind(), std::io::ErrorKind::AddrInUse);
}

async fn wait_listening(shared: &Shared, port: u16) {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if shared.is_listening_on(port).await {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("loopback server did not start");
}

async fn free_loopback_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("ephemeral loopback");
    listener.local_addr().expect("local addr").port()
}

#[tokio::test(flavor = "multi_thread")]
async fn port_change_keeps_a_stable_listener() {
    let (tx, _rx) = mpsc::channel(8);
    let shared = Arc::new(Shared::new(tx));
    let first = free_loopback_port().await;
    shared.request_port(first).expect("request first port");
    tokio::spawn(http::serve(shared.clone()));
    wait_listening(&shared, first).await;

    let second = free_loopback_port().await;
    shared.request_port(second).expect("request second port");
    wait_listening(&shared, second).await;

    tokio::time::sleep(Duration::from_millis(120)).await;
    assert!(
        shared.is_listening_on(second).await,
        "listener must stay up after a successful port change"
    );

    let url = format!("http://127.0.0.1:{second}/v1/status");
    let status = curl_json(&["-sS", &url]);
    assert_eq!(status["loopback_port"], second);
    tokio::time::sleep(Duration::from_millis(80)).await;
    let status = curl_json(&["-sS", &url]);
    assert_eq!(status["loopback_port"], second);
}

#[tokio::test]
async fn occupied_port_is_rejected_before_rebind() {
    let (tx, _rx) = mpsc::channel(8);
    let shared = Shared::new(tx);
    let held = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("hold a loopback port");
    let port = held.local_addr().expect("held addr").port();
    let listening = shared.is_listening_on(port).await;
    let before = shared.snapshot().await.loopback_port;
    let err = crate::bridge::loopback::reject_if_occupied(port, listening)
        .expect_err("occupied port must be rejected");
    assert!(err.contains("already in use"), "{err}");
    assert_eq!(shared.snapshot().await.loopback_port, before);
}

#[tokio::test]
async fn bind_preferred_moves_off_busy_port() {
    let held = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("hold a loopback port");
    let port = held.local_addr().expect("held addr").port();
    let (_listener, actual, moved) = http::bind_preferred(port)
        .await
        .expect("nearby port should bind");
    assert!(moved, "should skip the occupied port");
    assert_ne!(actual, port);
}

#[tokio::test(flavor = "multi_thread")]
async fn mock_status_is_connected_without_hardware() {
    let (base, _) = spawn_mock_bridge().await;
    let status = curl_json(&["-sS", &format!("{base}/v1/status")]);
    assert_eq!(status["ble"], "connected");
    assert_eq!(status["device"], MOCK_DEVICE_NAME);
    assert_eq!(status["pet_mode"], "wander");
    assert_eq!(status["pet_state"], "idle");
}

#[tokio::test(flavor = "multi_thread")]
async fn mock_test_endpoint_drives_simulated_face() {
    let (base, _) = spawn_mock_bridge().await;
    let posted = curl_json(&[
        "-sS",
        "-X",
        "POST",
        &format!("{base}/v1/test"),
        "-H",
        "Content-Type: application/json",
        "-d",
        r#"{"state":"thinking"}"#,
    ]);
    assert_eq!(posted["ok"], true);
    assert_eq!(posted["queued"], false);

    tokio::time::sleep(Duration::from_millis(50)).await;
    let status = curl_json(&["-sS", &format!("{base}/v1/status")]);
    assert_eq!(status["last_event"], "sessionStart");
    assert_eq!(status["pet_mode"], "agent");
    assert_eq!(status["pet_state"], "thinking");
}

#[tokio::test(flavor = "multi_thread")]
async fn mock_rejects_unknown_event_and_accepts_pairing() {
    let (base, _) = spawn_mock_bridge().await;
    let output = Command::new("curl")
        .args([
            "-sS",
            "-X",
            "POST",
            &format!("{base}/v1/cursor/event"),
            "-H",
            "Content-Type: application/json",
            "-d",
            r#"{"event":"dropTable"}"#,
        ])
        .output()
        .unwrap();
    let body: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(body["ok"], false);

    let paired = curl_json(&["-sS", "-X", "POST", &format!("{base}/v1/pairing/start")]);
    assert_eq!(paired["ok"], true);
    tokio::time::sleep(Duration::from_millis(50)).await;
    let status = curl_json(&["-sS", &format!("{base}/v1/status")]);
    assert_eq!(status["pairing_active"], true);
}

#[tokio::test(flavor = "multi_thread")]
async fn cursor_hook_script_posts_to_simulated_bridge() {
    let (base, _) = spawn_mock_bridge().await;
    let hook = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../scripts/cursor-hook.sh");
    let url = format!("{base}/v1/cursor/event");
    let status = Command::new(&hook)
        .arg("sessionStart")
        .env("DESKPET_BRIDGE_URL", &url)
        .status()
        .expect("run cursor-hook.sh sessionStart");
    assert!(status.success());
    let status = Command::new(&hook)
        .arg("afterAgentThought")
        .env("DESKPET_BRIDGE_URL", &url)
        .status()
        .expect("run cursor-hook.sh afterAgentThought");
    assert!(status.success());
    tokio::time::sleep(Duration::from_millis(80)).await;
    let snapshot = curl_json(&["-sS", &format!("{base}/v1/status")]);
    assert_eq!(snapshot["last_event"], "stateUpdate");
    assert_eq!(snapshot["pet_state"], "thinking");
}

#[tokio::test]
async fn queued_events_flush_into_simulated_pet() {
    let mut queue = EventQueue::default();
    let mut event = DeskPetEvent::new("sessionStart").with_state("coding");
    event.ensure_id();
    queue.push(event);
    let drained = queue.drain();
    let mut pet = mock::SimulatedPet::default();
    for item in drained {
        pet.apply(&item);
    }
    assert_eq!(pet.mode, "agent");
    assert_eq!(pet.state, "coding");
}
