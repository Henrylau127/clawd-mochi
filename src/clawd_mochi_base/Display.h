#pragma once

#include <Adafruit_ST7789.h>
#include <stdint.h>

#include "../utils/Config.h"

class Display {
 public:
  Display();

  void begin();
  void initColours();
  void setBacklight(bool on);
  void setBacklightPercent(uint8_t percent);
  bool isBacklightOn() const { return backlightOn_; }

  Adafruit_ST7789& tft() { return tft_; }

  uint16_t colorOrange() const { return cOrange_; }
  uint16_t colorDarkBg() const { return cDarkBg_; }
  uint16_t colorMuted() const { return cMuted_; }
  uint16_t colorGreen() const { return cGreen_; }
  uint16_t animBgColor() const { return animBgColor_; }
  void setAnimBgColor(uint16_t color) { animBgColor_ = color; }

  static constexpr uint16_t kWhite = ST77XX_WHITE;
  static constexpr uint16_t kBlack = ST77XX_BLACK;

 private:
  Adafruit_ST7789 tft_;
  bool backlightOn_ = true;
  uint16_t cOrange_ = 0;
  uint16_t cDarkBg_ = 0;
  uint16_t cMuted_ = 0;
  uint16_t cGreen_ = 0;
  uint16_t animBgColor_ = 0;
};
