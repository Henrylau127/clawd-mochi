#include "../clawd_mochi_base/States.h"
#include "SecurityEvents.h"

#include <Arduino.h>
#include <string.h>

#include "../clawd_mochi_base/Display.h"
#include "../clawd_mochi_base/Face.h"

SecurityEvents::SecurityEvents(PetStateMachine& state, Face& face, Display& display)
    : state_(state), face_(face), display_(display) {}

bool SecurityEvents::isActive() const { return state_.securityOverlayActive; }

const char* SecurityEvents::sanitizeTypeLabel(const char* type) {
  if (type == nullptr || type[0] == '\0') {
    return "Security alert";
  }
  if (strcmp(type, "ssh_login") == 0) {
    return "SSH login";
  }
  if (strcmp(type, "github_login") == 0) {
    return "GitHub login";
  }
  if (strcmp(type, "deployment") == 0) {
    return "Deployment";
  }
  if (strcmp(type, "sudo") == 0) {
    return "Privileged action";
  }
  return "Security alert";
}

void SecurityEvents::trigger(const char* typeLabel) {
  const char* label = sanitizeTypeLabel(typeLabel);
  strncpy(state_.securityTypeLabel, label, sizeof(state_.securityTypeLabel) - 1);
  state_.securityTypeLabel[sizeof(state_.securityTypeLabel) - 1] = '\0';
  state_.securityOverlayActive = true;
  const uint32_t duration = kMinOverlayMs + (esp_random() % (kMaxOverlayMs - kMinOverlayMs + 1));
  overlayEndMs_ = millis() + duration;
  needsRedraw_ = true;
  drawOverlay();
}

void SecurityEvents::redraw() { drawOverlay(); }

void SecurityEvents::drawOverlay() {
  Adafruit_ST7789& tft = display_.tft();
  const uint16_t bg = display_.tft().color565(120, 20, 80);
  display_.setAnimBgColor(bg);
  tft.fillScreen(bg);
  face_.drawExpression(AgentExpression::SECURITY);
  tft.setTextColor(Display::kWhite);
  tft.setTextSize(2);
  tft.setCursor(24, 190);
  tft.print(state_.securityTypeLabel);
}

bool SecurityEvents::tick(uint32_t nowMs) {
  if (!state_.securityOverlayActive) {
    return false;
  }
  if (nowMs >= overlayEndMs_) {
    state_.securityOverlayActive = false;
    state_.securityTypeLabel[0] = '\0';
    needsRedraw_ = true;
    return true;
  }
  return false;
}
