import { invoke } from "@tauri-apps/api/core";
import { LogicalSize } from "@tauri-apps/api/dpi";
import { getCurrentWindow } from "@tauri-apps/api/window";

type BlePhase = "disconnected" | "scanning" | "connected";
type SecureSessionState = "disconnected" | "handshaking" | "ready";
type EnrollmentPhase =
  | "idle"
  | "awaiting_device_pair"
  | "scanning"
  | "passkey_pending"
  | "no_pet_found"
  | "success";

type QueueEvent = {
  id?: string | null;
  event: string;
  state?: string | null;
  source?: string | null;
  ts?: string | null;
};

type KnownPet = {
  id: string;
  name: string;
};

type StatusSnapshot = {
  ble: BlePhase;
  secure_session: SecureSessionState;
  enrollment: EnrollmentPhase;
  device: string | null;
  queue: number;
  queue_items?: QueueEvent[];
  pairing_active: boolean;
  pairing_remaining_seconds?: number | null;
  last_event: string | null;
  last_error: string | null;
  pet_mode?: string | null;
  pet_state?: string | null;
  loopback_host?: string;
  loopback_port?: number;
  loopback_addr?: string;
  loopback_error?: string | null;
  loopback_note?: string | null;
  provider?: string;
  pets?: KnownPet[];
  active_pet_id?: string | null;
  ts: number;
};

let portHintOverride: string | null = null;
let committedPort = 8765;
let lastQueueJson = "";
let pollTimer = 0;

function setText(id: string, value: string) {
  const el = document.getElementById(id);
  if (el) {
    el.textContent = value;
  }
}

function setLoopbackHint(message: string, isError: boolean) {
  const hint = document.getElementById("loopback-hint");
  if (!hint) {
    return;
  }
  hint.classList.toggle("error", isError);
  hint.textContent = message;
}

function showPortError(message: string) {
  portHintOverride = message;
  setLoopbackHint(message, true);
}

function clearPortOverride() {
  portHintOverride = null;
}

function isQueueOpen(): boolean {
  return isOpen("queue-panel");
}

function isSettingsOpen(): boolean {
  return isOpen("settings-panel");
}

function isOpen(id: string): boolean {
  const panel = document.getElementById(id);
  return Boolean(panel && !panel.hasAttribute("hidden"));
}

function isForgetConfirmOpen(): boolean {
  return isOpen("forget-confirm");
}

function schedulePoll() {
  window.clearTimeout(pollTimer);
  pollTimer = window.setTimeout(() => {
    void refresh().finally(() => {
      schedulePoll();
    });
  }, isQueueOpen() ? 250 : 1000);
}

function setQueueOpen(open: boolean) {
  if (open) {
    setSettingsOpen(false);
  }
  const panel = document.getElementById("queue-panel");
  const trigger = document.getElementById("open-queue");
  panel?.toggleAttribute("hidden", !open);
  trigger?.setAttribute("aria-expanded", open ? "true" : "false");
  if (open) {
    lastQueueJson = "";
    void refresh();
  }
  schedulePoll();
}

function setSettingsOpen(open: boolean) {
  if (open) {
    const queue = document.getElementById("queue-panel");
    const queueTrigger = document.getElementById("open-queue");
    queue?.setAttribute("hidden", "");
    queueTrigger?.setAttribute("aria-expanded", "false");
  }
  const panel = document.getElementById("settings-panel");
  const trigger = document.getElementById("open-settings");
  const popover = document.querySelector(".popover");
  panel?.toggleAttribute("hidden", !open);
  trigger?.setAttribute("aria-expanded", open ? "true" : "false");
  popover?.classList.toggle("settings-open", open);
  requestAnimationFrame(() => {
    requestAnimationFrame(() => {
      void fitPopoverWindow();
    });
  });
}

function providerLabel(provider: string): string {
  switch (provider) {
    case "cursor":
      return "Cursor";
    case "codex":
      return "Codex";
    case "claude-code":
      return "Claude Code";
    case "pi":
      return "Pi";
    default:
      return provider;
  }
}

function sourceLabel(source: string): string {
  switch (source) {
    case "bridge":
      return "Bridge";
    case "test":
      return "Test";
    default:
      return providerLabel(source);
  }
}

function camelToTitle(value: string): string {
  return value
    .replace(/_/g, " ")
    .replace(/([a-z])([A-Z])/g, "$1 $2")
    .replace(/\b\w/g, (ch) => ch.toUpperCase());
}

function queueTitle(item: QueueEvent): string {
  switch (item.state) {
    case "thinking":
      return "Bot is thinking";
    case "coding":
      return "Bot is coding";
    case "idle":
      return "Bot is idle";
    case "waiting":
    case "waiting_for_user":
      return "Waiting for you";
    case "error":
      return "Bot hit an error";
    case "security":
      return "Security alert";
    default:
      if (item.state) {
        return `Bot is ${item.state.replace(/_/g, " ")}`;
      }
      break;
  }
  switch (item.event) {
    case "sessionStart":
      return "Session started";
    case "sessionEnd":
      return "Session ended";
    case "sleep":
      return "Sleeping";
    case "wake":
      return "Waking up";
    case "pairingStart":
      return "Pairing started";
    case "pairingStop":
      return "Pairing stopped";
    case "unbond":
      return "Device forgotten";
    case "attentionRequired":
      return "Needs attention";
    case "attentionCleared":
      return "Attention cleared";
    case "securityEvent":
      return "Security event";
    case "quotaUpdate":
      return "Quota updated";
    case "stateUpdate":
      return "State update";
    default:
      return camelToTitle(item.event);
  }
}

function renderQueue(items: QueueEvent[]) {
  if (!isQueueOpen()) {
    return;
  }
  const encoded = JSON.stringify(items);
  if (encoded === lastQueueJson) {
    return;
  }
  lastQueueJson = encoded;

  const list = document.getElementById("queue-list");
  const sub = document.getElementById("queue-panel-sub");
  if (!(list instanceof HTMLElement) || !sub) {
    return;
  }

  list.replaceChildren();
  if (items.length === 0) {
    list.hidden = true;
    sub.textContent = "No events waiting";
    return;
  }

  list.hidden = false;
  sub.textContent = items.length === 1 ? "1 event waiting" : `${items.length} events waiting`;

  for (const item of items) {
    const row = document.createElement("li");
    const title = document.createElement("span");
    title.className = "queue-item-event";
    title.textContent = queueTitle(item);
    const meta = document.createElement("span");
    meta.className = "queue-item-meta";
    const parts = [
      camelToTitle(item.event),
      item.source ? sourceLabel(item.source) : null,
      item.id ?? null,
    ].filter((part): part is string => typeof part === "string" && part.length > 0);
    meta.textContent = parts.join(" · ") || "pending";
    row.append(title, meta);
    list.append(row);
  }
}

function activePet(status: StatusSnapshot): KnownPet | null {
  const pets = status.pets ?? [];
  if (pets.length === 0) {
    return null;
  }
  const id = status.active_pet_id;
  return pets.find((pet) => pet.id === id || pet.name === id) ?? pets[0] ?? null;
}

function headerSubtitle(status: StatusSnapshot): string {
  const pet = activePet(status);
  if (status.secure_session === "handshaking" && status.device) {
    return `Securing ${status.device}`;
  }
  switch (status.ble) {
    case "connected":
      return status.device ?? pet?.name ?? "Connected";
    case "scanning":
      return pet ? `Looking for ${pet.name}` : "Looking for a pet";
    case "disconnected":
      return pet ? `${pet.name} is offline` : "No pet paired";
    default: {
      const _never: never = status.ble;
      return _never;
    }
  }
}

function reconnectHint(status: StatusSnapshot): string {
  const pet = activePet(status);
  switch (status.ble) {
    case "scanning":
      return pet ? `Scanning for ${pet.name}` : "Looking for a nearby pet";
    case "connected":
      return pet ? `Reconnect ${pet.name}` : "Reconnect the active pet";
    case "disconnected":
      return pet ? `Reconnect ${pet.name}` : "Pair a pet in Settings";
    default: {
      const _never: never = status.ble;
      return _never;
    }
  }
}

function renderPetSelect(status: StatusSnapshot) {
  const select = document.getElementById("active-pet");
  if (!(select instanceof HTMLSelectElement)) {
    return;
  }
  const pets = status.pets ?? [];
  const activeId = status.active_pet_id ?? "";
  if (document.activeElement === select) {
    return;
  }
  select.replaceChildren();
  if (pets.length === 0) {
    const option = document.createElement("option");
    option.value = "";
    option.textContent = "No pet paired";
    select.append(option);
    select.disabled = true;
    return;
  }
  select.disabled = false;
  for (const pet of pets) {
    const option = document.createElement("option");
    option.value = pet.id;
    option.textContent = pet.name;
    if (pet.id === activeId || pet.name === activeId) {
      option.selected = true;
    }
    select.append(option);
  }
}

function badgeFor(ble: BlePhase): { label: string; className: string } {
  switch (ble) {
    case "connected":
      return { label: "CONNECTED", className: "badge badge-ok" };
    case "scanning":
      return { label: "SCANNING", className: "badge badge-warn" };
    case "disconnected":
      return { label: "OFFLINE", className: "badge badge-off" };
    default: {
      const _never: never = ble;
      return _never;
    }
  }
}

function pairingHint(status: StatusSnapshot): string {
  const remaining = status.pairing_remaining_seconds;
  const countdown =
    typeof remaining === "number" ? ` · ${remaining}s remaining` : "";
  switch (status.enrollment) {
    case "awaiting_device_pair":
      return "Open PAIR over USB or hold GPIO9 on the pet, then wait for scanning.";
    case "scanning":
      return `Scanning for a device-confirmed pet${countdown}`;
    case "passkey_pending":
      return `Passkey pending: confirm the code shown on the pet${countdown}`;
    case "no_pet_found":
      return status.pairing_active
        ? `No pet found; open PAIR on the pet and try again${countdown}`
        : "No pet found. Open PAIR over USB or hold GPIO9, then try again.";
    case "success":
      return "Pet enrolled after secure session confirmation.";
    case "idle":
      return "Open PAIR over USB or hold GPIO9 to enroll a new pet.";
    default: {
      const _never: never = status.enrollment;
      return _never;
    }
  }
}

async function refresh() {
  try {
    const status = await invoke<StatusSnapshot>("get_status");
    setText("subtitle", headerSubtitle(status));
    setText("reconnect-sub", reconnectHint(status));
    setText("queue", String(status.queue));
    renderQueue(status.queue_items ?? []);
    setText("last-event", status.last_event ?? "—");
    setText("pet-mode", status.pet_mode ?? "—");
    setText("pet-state", status.pet_state ?? "—");
    setText("last-error", status.last_error ?? "none");

    if (typeof status.loopback_port === "number") {
      committedPort = status.loopback_port;
    }

    const provider = status.provider || "cursor";
    const providerSelect = document.getElementById("provider");
    if (providerSelect instanceof HTMLSelectElement && document.activeElement !== providerSelect) {
      providerSelect.value = provider;
    }

    if (!portHintOverride) {
      const addr = status.loopback_addr ?? "127.0.0.1:8765";
      setLoopbackHint(
        status.loopback_error ??
          status.loopback_note ??
          `${providerLabel(provider)} hooks POST to ${addr}`,
        Boolean(status.loopback_error),
      );
    }

    const portInput = document.getElementById("loopback-port");
    if (
      portInput instanceof HTMLInputElement &&
      document.activeElement !== portInput &&
      !portHintOverride
    ) {
      portInput.value = String(committedPort);
    }

    const badge = document.getElementById("badge");
    if (badge) {
      const next = badgeFor(status.ble);
      badge.className = next.className;
      badge.textContent = next.label;
    }

    const pairing = document.getElementById("pairing-toggle");
    if (pairing instanceof HTMLInputElement) {
      pairing.checked = status.pairing_active;
    }
    setText("pairing-hint", pairingHint(status));
    renderPetSelect(status);
    const forgetCopy = document.getElementById("forget-copy");
    const current = activePet(status);
    if (forgetCopy) {
      forgetCopy.textContent = current
        ? `Remove ${current.name} from the bridge? To remove its macOS Bluetooth bond too, use System Settings → Bluetooth → ${current.name} → Forget This Device.`
        : "Remove the active pet from the bridge? Remove its macOS Bluetooth bond in System Settings → Bluetooth → the pet → Forget This Device.";
    }
  } catch (err) {
    setText("last-error", String(err));
  }
}

function bind(id: string, handler: () => void) {
  document.getElementById(id)?.addEventListener("click", handler);
}

async function fitPopoverWindow() {
  const panel = document.querySelector(".popover");
  if (!(panel instanceof HTMLElement)) {
    return;
  }

  const bodyStyle = getComputedStyle(document.body);
  const padX = parseFloat(bodyStyle.paddingLeft) + parseFloat(bodyStyle.paddingRight);
  const padY = parseFloat(bodyStyle.paddingTop) + parseFloat(bodyStyle.paddingBottom);
  const width = Math.ceil(panel.offsetWidth + padX);

  if (isSettingsOpen()) {
    const sheet = document.getElementById("settings-panel");
    const head = sheet?.querySelector(".sheet-head");
    const body = sheet?.querySelector(".sheet-body");
    if (head instanceof HTMLElement && body instanceof HTMLElement) {
      const contentH = head.offsetHeight + 8 + body.scrollHeight + 20;
      const minH = Math.min(12 + contentH + 40, 720 - padY);
      panel.style.minHeight = `${Math.ceil(minH)}px`;
    }
  } else {
    panel.style.minHeight = "";
  }

  const height = Math.ceil(panel.offsetHeight + padY);

  try {
    await getCurrentWindow().setSize(new LogicalSize(width, height));
  } catch {
    // Browser preview is not a Tauri window.
  }
}

window.addEventListener("DOMContentLoaded", () => {
  bind("open-queue", () => {
    setQueueOpen(!isQueueOpen());
  });
  bind("close-queue", () => {
    setQueueOpen(false);
  });
  bind("open-settings", () => {
    setSettingsOpen(!isSettingsOpen());
  });
  bind("close-settings", () => {
    setSettingsOpen(false);
  });
  const invokeAction = (command: string) =>
    invoke(command).catch((err: unknown) => {
      setText("last-error", String(err));
    });

  bind("add-pet", () => {
    const button = document.getElementById("add-pet");
    if (button instanceof HTMLButtonElement) {
      button.disabled = true;
      button.setAttribute("aria-busy", "true");
    }
    void invokeAction("add_pet").finally(() => {
      if (button instanceof HTMLButtonElement) {
        button.disabled = false;
        button.removeAttribute("aria-busy");
      }
      void refresh();
    });
  });
  bind("reconnect", () => {
    void invokeAction("reconnect");
  });
  bind("test-pet", () => {
    void invokeAction("test_pet");
  });
  bind("quit", () => {
    void invoke("quit");
  });

  const applyPort = () => {
    const portInput = document.getElementById("loopback-port");
    if (!(portInput instanceof HTMLInputElement)) {
      return;
    }
    const port = Number.parseInt(portInput.value, 10);
    if (!Number.isInteger(port) || port < 1 || port > 65535) {
      showPortError("Use a port from 1 to 65535.");
      portInput.setCustomValidity("Use a port from 1 to 65535.");
      portInput.reportValidity();
      return;
    }
    portInput.setCustomValidity("");
    void invoke<number>("set_loopback_port", { port })
      .then(() => {
        clearPortOverride();
        return refresh().then(() => fitPopoverWindow());
      })
      .catch((err: unknown) => {
        showPortError(String(err));
      });
  };

  const revertPortEdit = () => {
    const portInput = document.getElementById("loopback-port");
    if (portInput instanceof HTMLInputElement) {
      portInput.value = String(committedPort);
      portInput.setCustomValidity("");
      portInput.blur();
    }
    clearPortOverride();
    void refresh();
  };

  bind("apply-port", applyPort);
  bind("cancel-port", revertPortEdit);
  const portField = document.getElementById("loopback-port");
  portField?.addEventListener("input", () => {
    clearPortOverride();
    void refresh();
  });
  portField?.addEventListener("keydown", (event) => {
    if (!(event instanceof KeyboardEvent)) {
      return;
    }
    if (event.key === "Enter") {
      event.preventDefault();
      applyPort();
      return;
    }
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      revertPortEdit();
    }
  });

  const petSelect = document.getElementById("active-pet");
  petSelect?.addEventListener("change", () => {
    if (!(petSelect instanceof HTMLSelectElement) || petSelect.value.length === 0) {
      return;
    }
    void invoke<string>("set_active_pet", { id: petSelect.value })
      .then(() => refresh())
      .catch((err: unknown) => {
        setText("last-error", String(err));
      });
  });

  const pairing = document.getElementById("pairing-toggle");
  pairing?.addEventListener("change", () => {
    if (!(pairing instanceof HTMLInputElement)) {
      return;
    }
    const requested = pairing.checked;
    pairing.disabled = true;
    void invoke(requested ? "start_pairing" : "stop_pairing")
      .catch((err: unknown) => {
        pairing.checked = !requested;
        setText("last-error", String(err));
      })
      .finally(() => {
        pairing.disabled = false;
        void refresh();
      });
  });

  const providerSelect = document.getElementById("provider");
  providerSelect?.addEventListener("change", () => {
    if (!(providerSelect instanceof HTMLSelectElement)) {
      return;
    }
    void invoke<string>("set_provider", { provider: providerSelect.value })
      .then(() => refresh())
      .catch((err: unknown) => {
        setText("last-error", String(err));
        providerSelect.value = "cursor";
      });
  });

  const confirmBox = document.getElementById("forget-confirm");
  bind("forget", () => {
    confirmBox?.removeAttribute("hidden");
  });
  bind("forget-no", () => {
    confirmBox?.setAttribute("hidden", "");
  });
  bind("forget-yes", () => {
    confirmBox?.setAttribute("hidden", "");
    void invoke("forget_device");
  });

  window.addEventListener("keydown", (event) => {
    if (event.key !== "Escape") {
      return;
    }
    const portInput = document.getElementById("loopback-port");
    if (event.target === portInput) {
      return;
    }
    if (isForgetConfirmOpen()) {
      document.getElementById("forget-confirm")?.setAttribute("hidden", "");
      return;
    }
    if (isQueueOpen()) {
      setQueueOpen(false);
      return;
    }
    if (isSettingsOpen()) {
      setSettingsOpen(false);
      return;
    }
    void invoke("hide_status");
  });

  void refresh().then(() => fitPopoverWindow());
  schedulePoll();
});
