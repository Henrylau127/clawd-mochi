#include "Display.h"

#include <SPI.h>

Display::Display() : tft_(TFT_CS, TFT_DC, TFT_RST) {}

void Display::begin() {
  pinMode(TFT_BLK, OUTPUT);
  setBacklight(true);

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
  backlightOn_ = on;
  digitalWrite(TFT_BLK, on ? HIGH : LOW);
}

void Display::setBacklightPercent(uint8_t percent) {
  if (percent == 0) {
    setBacklight(false);
    return;
  }
  setBacklight(true);
}
