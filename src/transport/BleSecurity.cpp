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
  pairingWindow_.close();
  passkeyVisible_ = false;
  state_.pairingActive = false;
}

void BleSecurity::startPairingWindow() {
  pairingActive_ = true;
  pairingWindow_.open(millis(), esp_random() % 1000000UL);
  state_.pairingActive = true;
  passkeyVisible_ = true;
  needsRedraw_ = true;
}

void BleSecurity::stopPairingWindow() {
  pairingActive_ = false;
  pairingWindow_.close();
  state_.pairingActive = false;
  passkeyVisible_ = false;
  needsRedraw_ = true;
}

void BleSecurity::eraseBonds() {
  NimBLEDevice::deleteAllBonds();
  stopPairingWindow();
  state_.onUnbond();
}

bool BleSecurity::isPairingActive() const { return pairingWindow_.isOpen(millis()); }

bool BleSecurity::allowsNewBond() const { return pairingWindow_.allowsNewBond(millis()); }

void BleSecurity::onPasskeyDisplay() {
  if (!allowsNewBond()) {
    return;
  }
  passkeyVisible_ = true;
  needsRedraw_ = true;
  showPairingScreen();
}

void BleSecurity::onAuthenticationComplete(bool success) {
  if (success && pairingActive_) {
    stopPairingWindow();
  } else {
    needsRedraw_ = true;
  }
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
  if (passkeyVisible_) {
    snprintf(code, sizeof(code), "%06lu",
             static_cast<unsigned long>(pairingWindow_.passkey()));
  } else {
    snprintf(code, sizeof(code), "------");
  }
  tft.setCursor(54, 145);
  tft.print(code);
}

uint32_t BleSecurity::pairingRemainingMs() const {
  return pairingWindow_.remainingMs(millis());
}

void BleSecurity::tick() {
  if (pairingActive_ && !pairingWindow_.isOpen(millis())) {
    stopPairingWindow();
  }
}
