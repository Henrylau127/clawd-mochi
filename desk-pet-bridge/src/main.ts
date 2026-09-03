import { invoke } from "@tauri-apps/api/core";

type StatusSnapshot = {
  ble: "disconnected" | "scanning" | "connected";
  device: string | null;
  queue: number;
  pairing_active: boolean;
  last_event: string | null;
  last_error: string | null;
  ts: number;
};

function setText(id: string, value: string) {
  const el = document.getElementById(id);
  if (el) {
    el.textContent = value;
  }
}

async function refresh() {
  try {
    const status = await invoke<StatusSnapshot>("get_status");
    setText("ble", status.ble);
    setText("device", status.device ?? "—");
    setText("queue", String(status.queue));
    setText("pairing", status.pairing_active ? "window open" : "off");
    setText("last-event", status.last_event ?? "—");
    setText("last-error", status.last_error ?? "—");
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
  bind("start-pairing", () => {
    void invoke("start_pairing");
  });
  bind("stop-pairing", () => {
    void invoke("stop_pairing");
  });
  bind("test-pet", () => {
    void invoke("test_pet");
  });
  bind("forget", () => {
    if (window.confirm("Erase desk pet bonds? You will need to pair again.")) {
      void invoke("forget_device");
    }
  });
  bind("hide", () => {
    void invoke("hide_status");
  });
  void refresh();
  window.setInterval(() => {
    void refresh();
  }, 1000);
});
