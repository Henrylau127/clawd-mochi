#pragma once

#include <stdint.h>

class Display;
class Face;
class PetStateMachine;

enum class WanderScreen : uint8_t {
  IDLE = 0,
  AMBIENT,
};

class WanderScreens {
 public:
  static constexpr uint32_t kCycleMs = 15000;

  WanderScreens(PetStateMachine& state, Face& face, Display& display);

  void reset();
  void tick(uint32_t nowMs);
  bool needsRedraw() const { return needsRedraw_; }
  void clearRedraw() { needsRedraw_ = false; }
  WanderScreen currentScreen() const { return screen_; }

 private:
  PetStateMachine& state_;
  Face& face_;
  Display& display_;
  WanderScreen screen_ = WanderScreen::IDLE;
  uint32_t screenStartMs_ = 0;
  uint32_t lastAnimMs_ = 0;
  bool blinkOpen_ = true;
  int8_t lookOffset_ = 0;
  int8_t lookDir_ = 1;
  bool needsRedraw_ = false;
  bool idlePrimed_ = false;

  struct AmbientStar {
    int16_t x;
    int16_t y;
    uint8_t phase;
  };

  static constexpr uint8_t kStarCount = 28;
  AmbientStar stars_[kStarCount]{};

  void advanceScreen(uint32_t nowMs);
  void regenerateAmbientStars();
  void drawCurrent(uint32_t nowMs);
  void drawAmbientScreen(uint32_t nowMs, bool fullRedraw);
};
