#include "Backlight.h"

#include "../clawd_mochi_base/Display.h"

Backlight::Backlight(Display& display) : display_(display) {}

void Backlight::begin() {
  setLevel(BacklightLevel::NORMAL);
}

void Backlight::setLevel(BacklightLevel level) {
  level_ = level;
  switch (level) {
    case BacklightLevel::OFF:
      percent_ = 0;
      break;
    case BacklightLevel::DIM:
      percent_ = kDimPercent;
      break;
    case BacklightLevel::NORMAL:
      percent_ = kNormalPercent;
      break;
  }
  display_.setBacklightPercent(percent_);
}

void Backlight::setPercent(uint8_t percent) {
  percent_ = percent;
  if (percent == 0) {
    level_ = BacklightLevel::OFF;
  } else if (percent <= kDimPercent) {
    level_ = BacklightLevel::DIM;
  } else {
    level_ = BacklightLevel::NORMAL;
  }
  display_.setBacklightPercent(percent_);
}

void Backlight::onSleep() { setLevel(BacklightLevel::OFF); }

void Backlight::onWake() { setLevel(BacklightLevel::NORMAL); }
