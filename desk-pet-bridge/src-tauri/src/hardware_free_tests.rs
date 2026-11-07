use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;
use tokio::net::TcpListener;
use tokio::sync::mpsc;

use crate::ble::mock::{self, MOCK_DEVICE_NAME};
use crate::bridge::http;
use crate::bridge::protocol::{DeskPetEvent, EventDetails};
use crate::bridge::queue::EventQueue;
use crate::state::{SecureSessionState, Shared};

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
    let err = http::bind_exact(port)
        .await
        .expect_err("port should be busy");
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
async fn snapshot_includes_queued_events_without_draining() {
    let (tx, _rx) = mpsc::channel(8);
    let shared = Shared::new(tx);
    let mut first = DeskPetEvent::new("sessionStart").with_state("coding");
    first.ensure_id();
    let mut second = DeskPetEvent::new("stateUpdate").with_state("thinking");
    second.ensure_id();
    shared.enqueue(first.clone()).await;
    shared.enqueue(second.clone()).await;
    let snap = shared.snapshot().await;
    assert_eq!(snap.queue, 2);
    assert_eq!(snap.queue_items, vec![first.clone(), second.clone()]);
    assert_eq!(snap.provider, "cursor");
    assert_eq!(shared.drain_queue().await, vec![first, second]);
    assert_eq!(shared.snapshot().await.queue, 0);
}

#[tokio::test]
async fn public_status_hides_queued_event_contents() {
    let (tx, _rx) = mpsc::channel(8);
    let shared = Shared::new(tx);
    let event = DeskPetEvent {
        details: Some(EventDetails {
            r#type: Some("ssh_login".to_string()),
            message: Some("SSH login from 203.0.113.1".to_string()),
        }),
        ..DeskPetEvent::new("securityEvent")
    };
    shared.enqueue(event).await;

    let status = serde_json::to_value(shared.public_snapshot().await).expect("serialize status");
    assert_eq!(status["queue"], 1);
    assert!(status.get("queue_items").is_none());
    assert!(!status.to_string().contains("203.0.113.1"));
}

#[tokio::test(flavor = "multi_thread")]
async fn pairing_control_events_are_commands_not_queued_ble_events() {
    let (tx, mut rx) = mpsc::channel(8);
    let shared = Arc::new(Shared::new(tx));
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("loopback bind");
    let addr = listener.local_addr().expect("local addr");
    tokio::spawn(http::serve_with_listener(shared.clone(), listener));

    let response = curl_json(&[
        "-sS",
        "-X",
        "POST",
        &format!("http://{addr}/v1/cursor/event"),
        "-H",
        "Content-Type: application/json",
        "-d",
        r#"{"event":"pairingStart"}"#,
    ]);

    assert_eq!(response["ok"], true);
    assert_eq!(shared.snapshot().await.queue, 0);
    assert!(matches!(
        rx.recv().await,
        Some(crate::state::BridgeCommand::StartPairing)
    ));
}

#[tokio::test]
async fn roster_keeps_one_active_pet() {
    let (tx, _rx) = mpsc::channel(8);
    let shared = Shared::new(tx);
    assert!(shared.snapshot().await.pets.is_empty());
    shared
        .remember_pet("aaa".to_string(), "DP-A1B2".to_string())
        .await;
    shared
        .remember_pet("bbb".to_string(), "DP-C3D4".to_string())
        .await;
    let snap = shared.snapshot().await;
    assert_eq!(snap.pets.len(), 2);
    assert_eq!(snap.active_pet_id.as_deref(), Some("bbb"));
    shared
        .set_active_pet("aaa".to_string())
        .await
        .expect("switch active pet");
    assert_eq!(
        shared.snapshot().await.active_pet_id.as_deref(),
        Some("aaa")
    );
    shared.forget_active_pet().await;
    let snap = shared.snapshot().await;
    assert_eq!(snap.pets.len(), 1);
    assert_eq!(snap.active_pet_id.as_deref(), Some("bbb"));
    assert_eq!(snap.pets[0].name, "DP-C3D4");
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
async fn mock_add_pet_enrolls_only_after_secure_session_ready() {
    let (base, shared) = spawn_mock_bridge().await;
    shared
        .command_tx
        .send(crate::state::BridgeCommand::AddPet)
        .await
        .expect("send add-pet command");

    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let status = shared.snapshot().await;
            if status.enrollment == crate::state::EnrollmentPhase::Success {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("mock enrollment did not complete");

    let status = curl_json(&["-sS", &format!("{base}/v1/status")]);
    assert_eq!(status["secure_session"], "ready");
    assert_eq!(status["enrollment"], "success");
    assert_eq!(status["pets"][0]["id"], "mock");
    assert_ne!(status["last_event"], "pairingStart");
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
    let remaining = status["pairing_remaining_seconds"]
        .as_u64()
        .expect("pairing countdown should be reported");
    assert!(
        (1..=120).contains(&remaining),
        "unexpected countdown: {remaining}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn cursor_hook_script_posts_to_simulated_bridge() {
    let (base, _) = spawn_mock_bridge().await;
    let hook = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scripts/cursor-hook.sh");
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

#[tokio::test]
async fn queued_events_are_gated_until_secure_session_is_ready() {
    let (tx, _rx) = mpsc::channel(8);
    let shared = Shared::new(tx);
    let event = DeskPetEvent::new("sessionStart").with_state("coding");

    shared.enqueue(event).await;
    assert!(!shared.is_connected().await);
    assert_eq!(shared.snapshot().await.queue, 1);
    assert_eq!(
        shared.snapshot().await.secure_session,
        SecureSessionState::Disconnected
    );

    shared
        .begin_secure_session("cached-id", "DP-A1B2", false)
        .await;
    assert!(!shared.is_connected().await);
    assert_eq!(
        shared.snapshot().await.secure_session,
        SecureSessionState::Handshaking
    );

    shared
        .mark_secure_session_ready("cached-id", "DP-A1B2")
        .await;
    assert!(shared.is_connected().await);
}

#[tokio::test]
async fn disconnect_resets_secure_session_and_live_status() {
    let (tx, _rx) = mpsc::channel(8);
    let shared = Shared::new(tx);
    shared
        .begin_secure_session("cached-id", "DP-A1B2", false)
        .await;
    shared
        .mark_secure_session_ready("cached-id", "DP-A1B2")
        .await;
    shared
        .set_pet_status(Some("agent".to_string()), Some("thinking".to_string()))
        .await;

    shared.reset_secure_session(Some("cached-id")).await;

    let snapshot = shared.snapshot().await;
    assert!(!shared.is_connected().await);
    assert_eq!(snapshot.ble, crate::state::BlePhase::Disconnected);
    assert_eq!(snapshot.secure_session, SecureSessionState::Disconnected);
    assert_eq!(snapshot.pet_mode, None);
    assert_eq!(snapshot.pet_state, None);
}

#[tokio::test]
async fn stale_status_updates_cannot_overwrite_a_new_session() {
    let (tx, _rx) = mpsc::channel(8);
    let shared = Shared::new(tx);
    shared
        .begin_secure_session("current-id", "DP-A1B2", false)
        .await;
    shared
        .mark_secure_session_ready("current-id", "DP-A1B2")
        .await;

    assert!(
        !shared
            .set_pet_status_for_session(
                "stale-id",
                Some("sleep".to_string()),
                Some("idle".to_string()),
            )
            .await
    );
    assert_eq!(shared.snapshot().await.pet_mode, None);
}

fn agent_hook_script() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scripts/agent-hook.sh")
}

fn dry_run_hook(provider: &str, hook: &str, extra: &[&str], stdin_json: &str) -> String {
    let mut command = Command::new("bash");
    command
        .arg(agent_hook_script())
        .arg(provider)
        .arg(hook)
        .args(extra)
        .env("DESKPET_HOOK_DRY_RUN", "1")
        .env_remove("DESKPET_BRIDGE_URL")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("spawn agent-hook.sh");
    {
        let mut stdin = child.stdin.take().expect("stdin");
        stdin
            .write_all(stdin_json.as_bytes())
            .expect("write hook stdin");
    }
    let output = child.wait_with_output().expect("wait agent-hook.sh");
    assert!(
        output.status.success(),
        "agent-hook.sh failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("utf8 dry-run payload")
}

#[test]
fn agent_hook_dry_run_keeps_status_enum_and_drops_secrets() {
    let secret = r#"{
      "hook_event_name": "PostModelRequest",
      "status": "failed",
      "error": "API key sk-live-SECRET",
      "prompt": "SECRET_PROMPT",
      "cwd": "/Users/me/SECRET_PATH",
      "transcript_path": "/tmp/SECRET_TRANSCRIPT.jsonl",
      "tool_input": {"command": "SECRET_COMMAND"},
      "last_assistant_message": "SECRET_REPLY"
    }"#;
    let payload = dry_run_hook("codex", "PostModelRequest", &[], secret);
    let value: Value = serde_json::from_str(payload.trim()).expect("dry-run json");
    assert_eq!(value["hook"], "PostModelRequest");
    assert_eq!(value["source"], "codex");
    assert_eq!(value["outcome"], "failed");
    assert!(value.get("prompt").is_none());
    assert!(!payload.contains("SECRET"));
    assert!(!payload.contains("sk-live"));

    let ignored = dry_run_hook("codex", "PreToolUse", &["SECRET_COMMAND"], "");
    assert!(!ignored.contains("SECRET_COMMAND"));
    assert!(ignored.contains("\"hook\":\"PreToolUse\""));

    let dropped = dry_run_hook(
        "codex",
        "notARealHook",
        &[],
        r#"{"prompt":"SECRET_PROMPT"}"#,
    );
    assert!(dropped.is_empty(), "{dropped}");
}

#[test]
fn omp_extension_forwards_labels_only() {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let script = agent_hook_script();
    let output = Command::new("node")
        .current_dir(&repo)
        .arg("--input-type=module")
        .arg("-e")
        .arg(
            r#"
import hook from "./.omp/hooks/post/desk-pet.js";
const handlers = new Map();
hook({ on(name, fn) { handlers.set(name, fn); } });
const secret = {
  prompt: "SECRET_PROMPT",
  text: "SECRET_TEXT",
  args: { command: "SECRET_CMD" },
  messages: ["SECRET_MSG"],
  cwd: "/secret/path",
  isError: true,
  willContinue: true,
  error: "SECRET_ERR",
  result: "SECRET_RESULT",
};
await handlers.get("before_agent_start")(secret, secret);
await handlers.get("tool_execution_end")(secret, secret);
await handlers.get("agent_end")(secret, secret);
"#,
        )
        .env("DESKPET_HOOK_DRY_RUN", "1")
        .env("DESKPET_HOOK_SCRIPT", &script)
        .output()
        .expect("node");
    assert!(
        output.status.success(),
        "omp hook failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    assert!(
        stdout.contains("\"hook\":\"before_agent_start\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"hook\":\"tool_execution_end\""),
        "{stdout}"
    );
    assert!(stdout.contains("\"outcome\":\"error\""), "{stdout}");
    assert!(stdout.contains("\"hook\":\"agent_end\""), "{stdout}");
    assert!(stdout.contains("\"outcome\":\"continue\""), "{stdout}");
    assert!(!stdout.contains("SECRET"), "{stdout}");
}

#[tokio::test(flavor = "multi_thread")]
async fn codex_and_omp_hooks_drive_simulated_pet() {
    let (base, _) = spawn_mock_bridge().await;
    let script = agent_hook_script();
    let post = |provider: &str, hook: &str, outcome: Option<&str>| {
        let url = format!("{base}/v1/{provider}/event");
        let mut command = Command::new("bash");
        command
            .arg(&script)
            .arg(provider)
            .arg(hook)
            .env("DESKPET_BRIDGE_URL", &url)
            .stdin(Stdio::null());
        if let Some(outcome) = outcome {
            command.arg(outcome);
        }
        let status = command.status().expect("run agent-hook.sh");
        assert!(status.success());
    };

    post("codex", "SessionStart", None);
    post("codex", "PreToolUse", None);
    post("codex", "Stop", None);
    tokio::time::sleep(Duration::from_millis(100)).await;
    let snapshot = curl_json(&["-sS", &format!("{base}/v1/status")]);
    assert_eq!(snapshot["last_event"], "stateUpdate");
    assert_eq!(snapshot["pet_mode"], "agent");
    assert_eq!(snapshot["pet_state"], "waiting_for_user");

    post("codex", "SessionEnd", None);
    tokio::time::sleep(Duration::from_millis(100)).await;
    let snapshot = curl_json(&["-sS", &format!("{base}/v1/status")]);
    assert_eq!(snapshot["last_event"], "sessionEnd");
    assert_eq!(snapshot["pet_mode"], "wander");
    assert_eq!(snapshot["pet_state"], "idle");

    post("omp", "session_start", None);
    post("omp", "tool_execution_end", Some("error"));
    tokio::time::sleep(Duration::from_millis(100)).await;
    let snapshot = curl_json(&["-sS", &format!("{base}/v1/status")]);
    assert_eq!(snapshot["pet_mode"], "agent");
    assert_eq!(snapshot["pet_state"], "error");

    post("omp", "session_shutdown", None);
    tokio::time::sleep(Duration::from_millis(100)).await;
    let snapshot = curl_json(&["-sS", &format!("{base}/v1/status")]);
    assert_eq!(snapshot["last_event"], "sessionEnd");
    assert_eq!(snapshot["pet_mode"], "wander");
}
