#include "BleSecurity.h"

#include <Arduino.h>
#include <NimBLEDevice.h>

#include "../clawd_mochi_base/Display.h"
#include "../clawd_mochi_base/Face.h"
#include "../clawd_mochi_base/States.h"

BleSecurity::BleSecurity(PetStateMachine& state, Face& face, Display& display)
    : state_(state), face_(face), display_(display) {}

void BleSecurity::begin() {
  pairingActive_ = false;
  state_.pairingActive = false;
}

void BleSecurity::startPairingWindow() {
  pairingActive_ = true;
  pairingDeadlineMs_ = millis() + kPairingWindowMs;
  state_.pairingActive = true;
  displayedPasskey_ = 0;
  passkeyVisible_ = false;
}

void BleSecurity::stopPairingWindow() {
  pairingActive_ = false;
  pairingDeadlineMs_ = 0;
  state_.pairingActive = false;
  passkeyVisible_ = false;
}

void BleSecurity::eraseBonds() {
  NimBLEDevice::deleteAllBonds();
  stopPairingWindow();
  state_.onUnbond();
}

void BleSecurity::onPasskeyDisplay(uint32_t passkey) {
  if (!pairingActive_) {
    return;
  }
  displayedPasskey_ = passkey;
  passkeyVisible_ = true;
  showPairingScreen();
}

void BleSecurity::showPairingScreen() {
  Adafruit_ST7789& tft = display_.tft();
  tft.fillScreen(display_.colorDarkBg());
  tft.setTextColor(Display::kWhite);
  tft.setTextSize(2);
  tft.setCursor(36, 70);
  tft.print("PAIRING");
  tft.setTextSize(1);
  tft.setCursor(24, 110);
  tft.print("Enter passkey on Mac:");
  tft.setTextSize(3);
  char code[8] = {};
  snprintf(code, sizeof(code), "%06lu", static_cast<unsigned long>(displayedPasskey_ % 1000000UL));
  tft.setCursor(54, 145);
  tft.print(code);
}

uint32_t BleSecurity::pairingRemainingMs() const {
  if (!pairingActive_) {
    return 0;
  }
  const uint32_t now = millis();
  if (now >= pairingDeadlineMs_) {
    return 0;
  }
  return pairingDeadlineMs_ - now;
}

void BleSecurity::tick() {
  if (!pairingActive_) {
    return;
  }
  if (millis() >= pairingDeadlineMs_) {
    stopPairingWindow();
  }
}
