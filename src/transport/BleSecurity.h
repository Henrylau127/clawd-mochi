#pragma once

#include <stdint.h>

#include "PairingWindow.h"

class Display;
class Face;
class PetStateMachine;

class BleSecurity {
 public:
  static constexpr uint32_t kPairingWindowMs = PairingWindow::kDurationMs;

  BleSecurity(PetStateMachine& state, Face& face, Display& display);

  void begin();
  void tick();

  bool isPairingActive() const;
  bool allowsNewBond() const;
  bool allowsConnection(bool bonded) const { return bonded || allowsNewBond(); }
  bool shouldShowPairingScreen() const { return pairingActive_ || passkeyVisible_; }
  bool needsRedraw() const { return needsRedraw_; }
  void clearRedraw() { needsRedraw_ = false; }
  uint32_t pairingRemainingMs() const;
  uint32_t passkey() const { return pairingWindow_.passkey(); }

  void startPairingWindow();
  void stopPairingWindow();
  void eraseBonds();

  void onPasskeyDisplay();
  void onAuthenticationComplete(bool success);
  void showPairingScreen();

 private:
  PetStateMachine& state_;
  Face& face_;
  Display& display_;
  bool pairingActive_ = false;
  bool passkeyVisible_ = false;
  bool needsRedraw_ = false;
  PairingWindow pairingWindow_;
};
