#pragma once

#include <stdint.h>

class Display;

enum class BacklightLevel : uint8_t {
  OFF = 0,
  DIM,
  NORMAL,
};

class Backlight {
 public:
  static constexpr uint8_t kNormalPercent = 100;
  static constexpr uint8_t kDimPercent = 15;

  explicit Backlight(Display& display);

  void begin();
  void setLevel(BacklightLevel level);
  void setPercent(uint8_t percent);
  uint8_t percent() const { return percent_; }
  BacklightLevel level() const { return level_; }

  void onSleep();
  void onWake();

 private:
  Display& display_;
  uint8_t percent_ = kNormalPercent;
  BacklightLevel level_ = BacklightLevel::NORMAL;
};
