#pragma once

#include <stdint.h>

class Display;
class PetStateMachine;

class Quota {
 public:
  Quota(PetStateMachine& state, Display& display);

  void setPercent(uint8_t percent);
  void setVisible(bool visible);
  void tick();
  void drawBar();
  bool needsRedraw() const { return needsRedraw_; }
  void clearRedraw() { needsRedraw_ = false; }

 private:
  PetStateMachine& state_;
  Display& display_;
  bool needsRedraw_ = false;

  static uint16_t barColor(Display& display, uint8_t percent);
};
