import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

// oh-my-pi extension: https://omp.sh/docs/hooks
// Forwards event names and an allowlisted outcome only. Prompts, tool
// arguments, paths, and message text stay in the omp process.

function scriptPath() {
  const fromEnv = process.env.DESKPET_HOOK_SCRIPT;
  if (fromEnv && existsSync(fromEnv)) {
    return fromEnv;
  }
  try {
    const here = dirname(fileURLToPath(import.meta.url));
    const sibling = resolve(here, "../../../scripts/agent-hook.sh");
    if (existsSync(sibling)) {
      return sibling;
    }
  } catch {
    // Some loaders do not expose import.meta.url.
  }
  let dir = process.cwd();
  for (let hop = 0; hop < 8; hop += 1) {
    const candidate = resolve(dir, "scripts/agent-hook.sh");
    if (existsSync(candidate)) {
      return candidate;
    }
    const parent = resolve(dir, "..");
    if (parent === dir) {
      break;
    }
    dir = parent;
  }
  return null;
}

function childEnv() {
  const env = { PATH: process.env.PATH ?? "" };
  for (const key of [
    "HOME",
    "USERPROFILE",
    "DESKPET_BRIDGE_URL",
    "DESKPET_BRIDGE_PORT",
    "DESKPET_HOOK_DRY_RUN",
    "DESKPET_HOOK_SCRIPT",
  ]) {
    const value = process.env[key];
    if (value) {
      env[key] = value;
    }
  }
  return env;
}

function notify(hook, outcome) {
  const script = scriptPath();
  if (!script) {
    return Promise.resolve();
  }
  const args = [script, "omp", hook];
  if (outcome) {
    args.push(outcome);
  }
  const dry = process.env.DESKPET_HOOK_DRY_RUN === "1";
  return new Promise((resolvePromise) => {
    let child;
    try {
      child = spawn("bash", args, {
        stdio: dry ? ["ignore", "inherit", "inherit"] : "ignore",
        detached: !dry,
        env: childEnv(),
      });
    } catch {
      resolvePromise();
      return;
    }
    child.on("error", () => resolvePromise());
    if (dry) {
      child.on("close", () => resolvePromise());
    } else {
      child.unref();
      resolvePromise();
    }
  });
}

function listen(pi, name, handler) {
  try {
    pi.on(name, (event) => {
      try {
        return handler(event);
      } catch {
        return undefined;
      }
    });
  } catch {
    // Older omp builds omit some events. Rust still accepts the name if a
    // newer build emits it and this file is updated to listen.
  }
}

export default function deskPet(pi) {
  listen(pi, "session_start", () => notify("session_start"));
  listen(pi, "session_switch", () => notify("session_switch"));
  listen(pi, "session_shutdown", () => notify("session_shutdown"));
  listen(pi, "agent_start", () => notify("agent_start"));
  listen(pi, "before_agent_start", () => notify("before_agent_start"));
  listen(pi, "turn_start", () => notify("turn_start"));
  listen(pi, "tool_call", () => notify("tool_call"));
  listen(pi, "tool_execution_start", () => notify("tool_execution_start"));
  listen(pi, "tool_execution_end", (event) =>
    notify("tool_execution_end", event != null && event.isError === true ? "error" : "ok"),
  );
  listen(pi, "agent_end", (event) =>
    notify("agent_end", event != null && event.willContinue === true ? "continue" : "done"),
  );
  listen(pi, "session_stop", () => notify("session_stop"));
}
