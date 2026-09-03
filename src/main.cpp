#include <Arduino.h>
#include <esp_mac.h>

#include "clawd_mochi_base/Display.h"
#include "clawd_mochi_base/Face.h"
#include "clawd_mochi_base/States.h"
#include "extensions/Attention.h"
#include "extensions/Quota.h"
#include "extensions/SecurityEvents.h"
#include "extensions/WanderScreens.h"
#include "transport/BleSecurity.h"
#include "transport/BleTransport.h"
#include "utils/Backlight.h"
#include "utils/Config.h"
#include "utils/JsonProtocol.h"
#include "utils/SerialDiag.h"

Display gDisplay;
Face gFace(gDisplay);
PetStateMachine gState;
Backlight gBacklight(gDisplay);
BleSecurity gBleSecurity(gState, gFace, gDisplay);
BleTransport gBleTransport(gState, gBleSecurity);
Attention gAttention(gState, gFace, gDisplay);
SecurityEvents gSecurity(gState, gFace, gDisplay);
Quota gQuota(gState, gDisplay);
WanderScreens gWander(gState, gFace, gDisplay);
SerialDiag gSerial(gState, gFace, gDisplay, &gBleSecurity, &gBleTransport);

enum class BootPhase : uint8_t {
  SPLASH = 0,
  LOGO_REVEAL,
  READY,
};

BootPhase gBootPhase = BootPhase::SPLASH;
uint32_t gBootPhaseStartMs = 0;
bool gRenderDirty = true;
PetMode gPrevMode = PetMode::WANDER;

uint32_t gBootButtonDownMs = 0;
bool gBootButtonWasDown = false;

void onAttentionRequiredHook() { gAttention.onAttentionRequired(); }
void onAttentionClearedHook() { gAttention.onAttentionCleared(); }
void onSessionStartHook() {
  gAttention.onSessionChange();
  gRenderDirty = true;
}
void onSessionEndHook() {
  gAttention.onSessionChange();
  gWander.reset();
  gRenderDirty = true;
}
void onSecurityEventHook(const char* type) { gSecurity.trigger(type); }
void onQuotaUpdateHook(uint8_t percent) { gQuota.setPercent(percent); }
void onPairingStartHook() { gBleSecurity.startPairingWindow(); }
void onPairingStopHook() { gBleSecurity.stopPairingWindow(); }
void onUnbondHook() { gBleSecurity.eraseBonds(); }
void onSecureBleWakeHook() {
  if (gState.onWake() == TransitionResult::OK) {
    gBacklight.onWake();
    gWander.reset();
    gRenderDirty = true;
  }
}

void onBleDisconnectHook() {
  if (gBleSecurity.shouldShowPairingScreen()) {
    gRenderDirty = true;
    return;
  }
  gAttention.onSessionChange();
  gWander.showLinkLost();
  gRenderDirty = true;
}

String deviceNameFromMac() {
  uint8_t mac[6] = {};
  esp_read_mac(mac, ESP_MAC_WIFI_STA);
  char name[16] = {};
  snprintf(name, sizeof(name), "DP-%02X%02X", mac[4], mac[5]);
  return String(name);
}

void markDirty() { gRenderDirty = true; }

void drawActiveScene() {
  if (gBleSecurity.shouldShowPairingScreen()) {
    gBacklight.setPercent(100);
    gBleSecurity.showPairingScreen();
    return;
  }

  if (gState.mode == PetMode::SLEEP) {
    gBacklight.onSleep();
    gDisplay.tft().fillScreen(gDisplay.colorDarkBg());
    return;
  }

  if (gState.securityOverlayActive) {
    gBacklight.setPercent(100);
    gSecurity.redraw();
    gQuota.drawBar();
    return;
  }

  uint8_t brightness = gState.backlightPercent;
  if (gState.attentionLevel >= AttentionLevel::SLOW_PULSE) {
    brightness = gAttention.pulseBrightness();
  } else if (gState.attentionLevel == AttentionLevel::URGENT) {
    brightness = 100;
  }
  gBacklight.setPercent(brightness);

  if (gState.mode == PetMode::WANDER) {
    return;
  }

  if (gState.mode == PetMode::AGENT) {
    if (gState.attentionLevel == AttentionLevel::URGENT) {
      gDisplay.setAnimBgColor(gDisplay.colorOrange());
    }
    gFace.drawExpression(gState.expression);
    gQuota.drawBar();
  }
}

void tickRender() {
  if (gBootPhase != BootPhase::READY) {
    return;
  }

  const uint32_t now = millis();
  const bool securityExpired = gSecurity.tick(now);
  if (securityExpired) {
    gRenderDirty = true;
    if (gState.mode == PetMode::WANDER) {
      gWander.reset();
    }
  }
  gQuota.tick();
  bool dirty = gRenderDirty || gSerial.needsRedraw() || gAttention.needsRedraw() ||
               gSecurity.needsRedraw() || gBleSecurity.needsRedraw();

  if (gState.mode == PetMode::WANDER && gPrevMode != PetMode::WANDER) {
    gWander.reset();
    dirty = true;
  }
  gPrevMode = gState.mode;

  if (gBleSecurity.shouldShowPairingScreen()) {
    dirty = true;
  } else if (gState.mode == PetMode::WANDER && !gState.securityOverlayActive) {
    gWander.tick(now);
    dirty = dirty || gWander.needsRedraw();
  } else if (gState.mode == PetMode::AGENT) {
    gAttention.tick(now);
    dirty = dirty || gAttention.needsRedraw();
  }

  if (dirty) {
    const bool pairingUi = gBleSecurity.shouldShowPairingScreen();
    if (pairingUi ||
        !(gState.mode == PetMode::WANDER && !gState.securityOverlayActive &&
          !gSerial.needsRedraw() && !gRenderDirty && !gSecurity.needsRedraw())) {
      drawActiveScene();
    }
    gRenderDirty = false;
    gSerial.clearRedraw();
    gAttention.clearRedraw();
    gSecurity.clearRedraw();
    gBleSecurity.clearRedraw();
    gWander.clearRedraw();
  }
}

void tickBootSequence() {
  const uint32_t now = millis();
  switch (gBootPhase) {
    case BootPhase::SPLASH:
      if ((now - gBootPhaseStartMs) >= 1200) {
        gFace.beginLogoReveal();
        gBootPhase = BootPhase::LOGO_REVEAL;
      }
      break;
    case BootPhase::LOGO_REVEAL:
      if (gFace.tickLogoReveal()) {
        gBootPhase = BootPhase::READY;
        gRenderDirty = true;
      }
      break;
    case BootPhase::READY:
      break;
  }
}

void tickBootButton() {
  const bool down = digitalRead(BOOT_BUTTON_PIN) == LOW;
  const uint32_t now = millis();

  if (down && !gBootButtonWasDown) {
    gBootButtonDownMs = now;
  }

  if (!down && gBootButtonWasDown) {
    if ((now - gBootButtonDownMs) >= BOOT_LONG_PRESS_MS) {
      gBleSecurity.startPairingWindow();
      gState.onPairingStart();
      gRenderDirty = true;
    }
    gBootButtonDownMs = 0;
  }

  gBootButtonWasDown = down;
}

void setup() {
  pinMode(BOOT_BUTTON_PIN, INPUT_PULLUP);

  gDisplay.begin();
  gBacklight.begin();
  gSerial.begin();

  JsonProtocol::setEventHooks(onAttentionRequiredHook, onAttentionClearedHook, onSecurityEventHook,
                              onQuotaUpdateHook);
  JsonProtocol::setSessionHooks(onSessionStartHook, onSessionEndHook);
  JsonProtocol::setPairingHooks(onPairingStartHook, onPairingStopHook, onUnbondHook);

  gBleSecurity.begin();
  const String deviceName = deviceNameFromMac();
  gBleTransport.begin(deviceName.c_str());
  BleTransport::setRedrawHook(markDirty);
  BleTransport::setSecureWakeHook(onSecureBleWakeHook);
  BleTransport::setDisconnectHook(onBleDisconnectHook);

  gBootPhaseStartMs = millis();
  gFace.showBootSplash();
  gWander.reset();
}

void loop() {
  gSerial.poll();
  gBleTransport.tick();
  tickBootSequence();
  tickBootButton();
  tickRender();
}
