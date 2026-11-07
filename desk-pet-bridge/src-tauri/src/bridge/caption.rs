use serde_json::{json, Map, Value};

/// Hints copied off an incoming payload before sensitive keys are dropped.
/// `command` is classified locally and never written back onto the event.
#[derive(Debug, Default, Clone)]
pub struct ActivityHint {
    pub command: Option<String>,
    pub scene: Option<String>,
    pub failed: bool,
    pub caption: Option<String>,
    pub mood: Option<String>,
}

impl ActivityHint {
    pub fn from_value(body: &Value) -> Self {
        let command = body
            .get("command")
            .and_then(Value::as_str)
            .or_else(|| body.pointer("/tool_input/command").and_then(Value::as_str))
            .map(str::to_string);
        let failed = command_failed(body);
        Self {
            scene: body
                .get("scene")
                .and_then(Value::as_str)
                .map(str::to_string),
            command,
            failed,
            caption: body
                .get("caption")
                .and_then(Value::as_str)
                .map(str::to_string),
            mood: body.get("mood").and_then(Value::as_str).map(str::to_string),
        }
    }
}

const SENSITIVE_KEYS: &[&str] = &[
    "command",
    "file_path",
    "path",
    "prompt",
    "tool_input",
    "cwd",
    "args",
    "secret",
    "token",
    "query",
    "diff",
    "content",
    "text",
    "input",
    "exit_code",
    "exitCode",
    "scene",
    "transcript_path",
    "workspace_roots",
];

pub fn allowed_caption(text: &str) -> Option<&'static str> {
    match text {
        "waking up" => Some("waking up"),
        "thinking" => Some("thinking"),
        "working" => Some("working"),
        "reading" => Some("reading"),
        "editing" => Some("editing"),
        "using a tool" => Some("using a tool"),
        "waiting for you" => Some("waiting for you"),
        "something failed" => Some("something failed"),
        "committed" => Some("committed"),
        "commit failed" => Some("commit failed"),
        "tests passed" => Some("tests passed"),
        "tests failed" => Some("tests failed"),
        "nice work" => Some("nice work"),
        "idle" => Some("idle"),
        _ => None,
    }
}

pub fn allowed_mood(text: &str) -> Option<&'static str> {
    match text {
        "happy" => Some("happy"),
        "worried" => Some("worried"),
        "startled" => Some("startled"),
        _ => None,
    }
}

pub fn canonical_state(state: &str) -> &str {
    match state {
        "done" | "success" => "celebrate",
        other => other,
    }
}

pub fn caption_for_state(state: &str) -> Option<&'static str> {
    match canonical_state(state) {
        "coding" => Some("working"),
        "thinking" => Some("thinking"),
        "idle" => Some("idle"),
        "waiting" | "waiting_for_user" => Some("waiting for you"),
        "error" => Some("something failed"),
        "celebrate" => Some("nice work"),
        _ => None,
    }
}

pub fn decorate(body: &mut Value, hook: &str, hint: &ActivityHint) {
    let Some(obj) = body.as_object_mut() else {
        return;
    };
    strip_sensitive(obj);

    let event = obj
        .get("event")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if event != "sessionStart" && event != "stateUpdate" {
        obj.remove("caption");
        obj.remove("mood");
        return;
    }

    let state = obj
        .get("state")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let scene = resolve_scene(hint);
    let new_state = next_state(hook, &state, scene.as_deref());
    let caption = next_caption(hook, &new_state, scene.as_deref(), hint);
    let mood = hint.mood.as_deref().and_then(allowed_mood);

    if new_state.is_empty() {
        obj.remove("state");
    } else {
        obj.insert("state".to_string(), json!(new_state));
    }
    match caption {
        Some(text) => {
            obj.insert("caption".to_string(), json!(text));
        }
        None => {
            obj.remove("caption");
        }
    }
    match mood {
        Some(text) => {
            obj.insert("mood".to_string(), json!(text));
        }
        None => {
            obj.remove("mood");
        }
    }
}

fn strip_sensitive(obj: &mut Map<String, Value>) {
    for key in SENSITIVE_KEYS {
        obj.remove(*key);
    }
}

fn command_failed(body: &Value) -> bool {
    match body.get("exit_code").or_else(|| body.get("exitCode")) {
        Some(Value::Number(number)) => number.as_i64().unwrap_or(0) != 0,
        Some(Value::String(text)) => !text.is_empty() && text != "0",
        _ => false,
    }
}

fn resolve_scene(hint: &ActivityHint) -> Option<String> {
    let mut scene = hint.scene.clone().or_else(|| {
        hint.command
            .as_deref()
            .and_then(scene_from_command)
            .map(str::to_string)
    });
    if hint.failed {
        scene = match scene.as_deref() {
            Some("commit") => Some("commit_failed".to_string()),
            Some("test") => Some("test_failed".to_string()),
            other => other.map(str::to_string),
        };
    }
    scene
}

fn next_state(hook: &str, state: &str, scene: Option<&str>) -> String {
    match scene_outcome(hook, scene) {
        Some(SceneOutcome::Celebrate) => "celebrate".to_string(),
        Some(SceneOutcome::Failed) => "error".to_string(),
        Some(SceneOutcome::Working) => {
            if state.is_empty() {
                "coding".to_string()
            } else {
                canonical_state(state).to_string()
            }
        }
        None => canonical_state(state).to_string(),
    }
}

fn next_caption(
    hook: &str,
    state: &str,
    scene: Option<&str>,
    hint: &ActivityHint,
) -> Option<&'static str> {
    if let Some(text) = scene_caption(hook, scene) {
        return Some(text);
    }
    if let Some(text) = hint.caption.as_deref().and_then(allowed_caption) {
        return Some(text);
    }
    if let Some(text) = hook_caption(hook) {
        return Some(text);
    }
    caption_for_state(state)
}

fn hook_caption(hook: &str) -> Option<&'static str> {
    match hook {
        "sessionStart" => Some("waking up"),
        "afterAgentThought" => Some("thinking"),
        "beforeReadFile" => Some("reading"),
        "afterFileEdit" => Some("editing"),
        "beforeMCPExecution" => Some("using a tool"),
        "preToolUse" | "beforeShellExecution" | "afterShellExecution" | "postToolUse" => {
            Some("working")
        }
        "afterAgentResponse" => Some("waiting for you"),
        "postToolUseFailure" => Some("something failed"),
        _ => None,
    }
}

enum SceneOutcome {
    Celebrate,
    Failed,
    Working,
}

fn scene_kind(scene: &str) -> Option<(&'static str, bool)> {
    match scene {
        "commit" => Some(("commit", false)),
        "commit_failed" => Some(("commit", true)),
        "test" => Some(("test", false)),
        "test_failed" => Some(("test", true)),
        _ => None,
    }
}

fn scene_outcome(hook: &str, scene: Option<&str>) -> Option<SceneOutcome> {
    let (_, marked_failed) = scene_kind(scene?)?;
    let failed = marked_failed || hook == "postToolUseFailure";
    if failed {
        return Some(SceneOutcome::Failed);
    }
    if matches!(
        hook,
        "beforeShellExecution" | "preToolUse" | "beforeMCPExecution"
    ) {
        return Some(SceneOutcome::Working);
    }
    if matches!(hook, "afterShellExecution" | "postToolUse") {
        return Some(SceneOutcome::Celebrate);
    }
    None
}

fn scene_caption(hook: &str, scene: Option<&str>) -> Option<&'static str> {
    let (kind, _) = scene_kind(scene?)?;
    match scene_outcome(hook, scene)? {
        SceneOutcome::Celebrate => Some(if kind == "test" {
            "tests passed"
        } else {
            "committed"
        }),
        SceneOutcome::Failed => Some(if kind == "test" {
            "tests failed"
        } else {
            "commit failed"
        }),
        SceneOutcome::Working => Some("working"),
    }
}

fn scene_from_command(command: &str) -> Option<&'static str> {
    let lower = command.to_ascii_lowercase();
    for segment in split_shell_segments(&lower) {
        if segment_is_git_commit(segment) {
            return Some("commit");
        }
        if segment_is_test(segment) {
            return Some("test");
        }
    }
    None
}

fn split_shell_segments(command: &str) -> Vec<&str> {
    let bytes = command.as_bytes();
    let mut parts = Vec::new();
    let mut start = 0;
    let mut index = 0;
    while index < bytes.len() {
        let is_break = bytes[index] == b';'
            || bytes[index] == b'|'
            || bytes[index] == b'&'
            || bytes[index] == b'\n';
        if is_break {
            let segment = command[start..index].trim();
            if !segment.is_empty() {
                parts.push(segment);
            }
            index += 1;
            start = index;
            continue;
        }
        index += 1;
    }
    let tail = command[start..].trim();
    if !tail.is_empty() {
        parts.push(tail);
    }
    parts
}

fn command_words(segment: &str) -> Vec<&str> {
    segment
        .split_whitespace()
        .filter(|word| !is_env_assignment(word))
        .collect()
}

fn is_env_assignment(word: &str) -> bool {
    match word.split_once('=') {
        Some((name, _)) => {
            !name.is_empty()
                && !name.starts_with('-')
                && name
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        }
        None => false,
    }
}

fn segment_is_git_commit(segment: &str) -> bool {
    let words = command_words(segment);
    let mut index = usize::from(words.first() == Some(&"sudo"));
    if words.get(index) != Some(&"git") {
        return false;
    }
    index += 1;
    while index < words.len() {
        let word = words[index];
        // The command is lowercased, so `git -C <path>` arrives as `-c`.
        if word == "-c" {
            index += 2;
            continue;
        }
        if word.starts_with('-') {
            index += 1;
            continue;
        }
        return word == "commit";
    }
    false
}

fn segment_is_test(segment: &str) -> bool {
    let words = command_words(segment);
    let index = usize::from(words.first() == Some(&"sudo"));
    let command = words.get(index).copied().unwrap_or("");
    let next = words.get(index + 1).copied().unwrap_or("");
    match command {
        "pytest" => true,
        "cargo" | "npm" | "pnpm" | "yarn" | "pio" | "go" => next == "test",
        "python" | "python3" => next == "-m" && words.get(index + 2) == Some(&"pytest"),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_commit_and_test_commands_without_keeping_them() {
        assert_eq!(
            scene_from_command("git commit -m 'fix parser /tmp/secret'"),
            Some("commit")
        );
        assert_eq!(
            scene_from_command("FOO=1 git -C /tmp/repo commit"),
            Some("commit")
        );
        assert_eq!(scene_from_command("git status"), None);
        assert_eq!(scene_from_command("echo git commit"), None);
        assert_eq!(scene_from_command("cargo test --lib"), Some("test"));
        assert_eq!(scene_from_command("python -m pytest tests"), Some("test"));
    }

    #[test]
    fn decorate_turns_a_finished_commit_into_a_caption_and_drops_the_command() {
        let source = json!({
            "hook": "afterShellExecution",
            "command": "git commit -m 'fix /tmp/secret'",
            "file_path": "/tmp/secret"
        });
        let hint = ActivityHint::from_value(&source);
        let mut mapped = json!({"event": "stateUpdate", "state": "coding", "source": "cursor"});
        decorate(&mut mapped, "afterShellExecution", &hint);
        assert_eq!(mapped["state"], "celebrate");
        assert_eq!(mapped["caption"], "committed");
        let blob = mapped.to_string();
        assert!(!blob.contains("secret"));
        assert!(!blob.contains("git commit"));
        assert!(mapped.get("command").is_none());
        assert!(mapped.get("file_path").is_none());
    }

    #[test]
    fn decorate_waits_to_celebrate_until_the_command_finishes() {
        let source = json!({"command": "git commit -m hello"});
        let hint = ActivityHint::from_value(&source);
        let mut mapped = json!({"event": "stateUpdate", "state": "coding"});
        decorate(&mut mapped, "beforeShellExecution", &hint);
        assert_eq!(mapped["state"], "coding");
        assert_eq!(mapped["caption"], "working");
        assert!(!mapped.to_string().contains("hello"));
    }

    #[test]
    fn decorate_marks_a_failed_test_without_keeping_the_command() {
        let source = json!({
            "command": "cargo test -p desk-pet",
            "exit_code": 1
        });
        let hint = ActivityHint::from_value(&source);
        let mut mapped = json!({"event": "stateUpdate", "state": "coding"});
        decorate(&mut mapped, "afterShellExecution", &hint);
        assert_eq!(mapped["state"], "error");
        assert_eq!(mapped["caption"], "tests failed");
        assert!(!mapped.to_string().contains("cargo"));
    }

    #[test]
    fn decorate_drops_a_caption_that_is_not_on_the_allowlist() {
        let source = json!({"caption": "/Users/me/secret.env", "mood": "sudo rm"});
        let hint = ActivityHint::from_value(&source);
        let mut mapped = json!({"event": "stateUpdate", "state": "coding"});
        decorate(&mut mapped, "stateUpdate", &hint);
        assert_eq!(mapped["caption"], "working");
        assert!(mapped.get("mood").is_none());
        assert!(!mapped.to_string().contains("secret"));
    }

    #[test]
    fn decorate_keeps_an_allowlisted_mood() {
        let source = json!({"mood": "startled", "caption": "thinking"});
        let hint = ActivityHint::from_value(&source);
        let mut mapped = json!({"event": "stateUpdate", "state": "thinking"});
        decorate(&mut mapped, "afterAgentThought", &hint);
        assert_eq!(mapped["caption"], "thinking");
        assert_eq!(mapped["mood"], "startled");
    }
}
