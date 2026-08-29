#pragma once

#include <stdint.h>

#include "../clawd_mochi_base/States.h"

class Display;
class Face;
class PetStateMachine;

class Attention {
 public:
  static constexpr uint32_t kUrgentMs = 120000;
  static constexpr uint32_t kSlowPulseMs = 300000;
  static constexpr uint32_t kFastPulseMs = 600000;
  static constexpr uint32_t kSlowPulsePeriodMs = 1000;
  static constexpr uint32_t kFastPulsePeriodMs = 500;

  Attention(PetStateMachine& state, Face& face, Display& display);

  void reset();
  void onAttentionRequired();
  void onAttentionCleared();
  void onSessionChange();
  void tick(uint32_t nowMs);

  bool needsRedraw() const { return needsRedraw_; }
  void clearRedraw() { needsRedraw_ = false; }
  uint8_t pulseBrightness() const { return pulseBrightness_; }

 private:
  PetStateMachine& state_;
  Face& face_;
  Display& display_;
  uint32_t attentionStartMs_ = 0;
  bool active_ = false;
  bool needsRedraw_ = false;
  uint8_t pulseBrightness_ = 100;
  bool pulseHigh_ = true;
  uint32_t lastPulseMs_ = 0;

  void updateLevel(uint32_t nowMs);
};
