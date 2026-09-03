import { invoke } from "@tauri-apps/api/core";
import { LogicalSize } from "@tauri-apps/api/dpi";
import { getCurrentWindow } from "@tauri-apps/api/window";

type BlePhase = "disconnected" | "scanning" | "connected";

type StatusSnapshot = {
  ble: BlePhase;
  device: string | null;
  queue: number;
  pairing_active: boolean;
  last_event: string | null;
  last_error: string | null;
  pet_mode?: string | null;
  pet_state?: string | null;
  loopback_host?: string;
  loopback_port?: number;
  loopback_addr?: string;
  loopback_error?: string | null;
  loopback_note?: string | null;
  ts: number;
};

let portHintOverride: string | null = null;
let committedPort = 8765;

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

async function refresh() {
  try {
    const status = await invoke<StatusSnapshot>("get_status");
    const subtitle = status.device ?? "Waiting for device";
    setText("subtitle", subtitle);
    setText("queue", String(status.queue));
    setText("last-event", status.last_event ?? "—");
    setText("pet-mode", status.pet_mode ?? "—");
    setText("pet-state", status.pet_state ?? "—");
    setText("last-error", status.last_error ?? "none");

    if (typeof status.loopback_port === "number") {
      committedPort = status.loopback_port;
    }

    if (!portHintOverride) {
      setLoopbackHint(
        status.loopback_error ??
          status.loopback_note ??
          `Cursor hooks POST to ${status.loopback_addr ?? "127.0.0.1:8765"}`,
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
  const width = Math.ceil(
    panel.offsetWidth + parseFloat(bodyStyle.paddingLeft) + parseFloat(bodyStyle.paddingRight),
  );
  const height = Math.ceil(
    panel.offsetHeight + parseFloat(bodyStyle.paddingTop) + parseFloat(bodyStyle.paddingBottom),
  );

  try {
    await getCurrentWindow().setSize(new LogicalSize(width, height));
  } catch {
    // Browser preview is not a Tauri window.
  }
}

window.addEventListener("DOMContentLoaded", () => {
  bind("reconnect", () => {
    void invoke("reconnect");
  });
  bind("test-pet", () => {
    void invoke("test_pet");
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

  const pairing = document.getElementById("pairing-toggle");
  pairing?.addEventListener("change", () => {
    if (!(pairing instanceof HTMLInputElement)) {
      return;
    }
    void invoke(pairing.checked ? "start_pairing" : "stop_pairing");
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
    void invoke("hide_status");
  });

  void refresh().then(() => fitPopoverWindow());
  window.setInterval(() => {
    void refresh();
  }, 1000);
});
