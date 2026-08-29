#pragma once

#include <stdint.h>

class Display;
class Face;
class PetStateMachine;

class BleSecurity {
 public:
  static constexpr uint32_t kPairingWindowMs = 120000;

  BleSecurity(PetStateMachine& state, Face& face, Display& display);

  void begin();
  void tick();

  bool isPairingActive() const { return pairingActive_; }
  uint32_t pairingRemainingMs() const;

  void startPairingWindow();
  void stopPairingWindow();
  void eraseBonds();

  void onPasskeyDisplay(uint32_t passkey);
  void showPairingScreen();

 private:
  PetStateMachine& state_;
  Face& face_;
  Display& display_;
  bool pairingActive_ = false;
  uint32_t pairingDeadlineMs_ = 0;
  uint32_t displayedPasskey_ = 0;
  bool passkeyVisible_ = false;
};
