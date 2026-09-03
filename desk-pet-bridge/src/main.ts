import { invoke } from "@tauri-apps/api/core";

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
  ts: number;
};

function setText(id: string, value: string) {
  const el = document.getElementById(id);
  if (el) {
    el.textContent = value;
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
    if (event.key === "Escape") {
      void invoke("hide_status");
    }
  });

  void refresh();
  window.setInterval(() => {
    void refresh();
  }, 1000);
});
