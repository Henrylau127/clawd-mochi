#include "Display.h"

#include <Arduino.h>
#include <SPI.h>

namespace {

constexpr uint32_t kBacklightPwmHz = 5000;
constexpr uint8_t kBacklightPwmResolution = 12;
constexpr uint32_t kBacklightPwmMaxDuty = (1U << kBacklightPwmResolution) - 1;

}  // namespace

Display::Display() : tft_(TFT_CS, TFT_DC, TFT_RST) {}

void Display::begin() {
  ledcAttach(TFT_BLK, kBacklightPwmHz, kBacklightPwmResolution);
  setBacklightPercent(100);

  SPI.begin(8, -1, 10, TFT_CS);
  tft_.init(DISP_W, DISP_H);
  tft_.setSPISpeed(40000000);
  tft_.setRotation(1);
  initColours();
}

void Display::initColours() {
  cOrange_ = tft_.color565(218, 17, 0);
  cDarkBg_ = tft_.color565(10, 12, 16);
  cMuted_ = tft_.color565(90, 88, 86);
  cGreen_ = tft_.color565(80, 220, 130);
  animBgColor_ = cOrange_;
}

void Display::setBacklight(bool on) {
  setBacklightPercent(on ? 100 : 0);
}

void Display::setBacklightPercent(uint8_t percent) {
  const uint8_t clamped = percent > 100 ? 100 : percent;
  const uint32_t duty = (static_cast<uint32_t>(clamped) * kBacklightPwmMaxDuty) / 100;
  backlightOn_ = clamped != 0;
  ledcWrite(TFT_BLK, duty);
}
