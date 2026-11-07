#include "Quota.h"

#include <Arduino.h>
#include <stdio.h>

#include "../clawd_mochi_base/Display.h"
#include "../clawd_mochi_base/States.h"

namespace {

constexpr int16_t kRingCx = 214;
constexpr int16_t kRingCy = 22;

// 24 points, clockwise from the top.
constexpr int8_t kRing[24][2] = {
    {0, -14},  {4, -14},  {7, -12},   {10, -10}, {12, -7},  {14, -4},  {14, 0},   {14, 4},
    {12, 7},   {10, 10},  {7, 12},    {4, 13},   {0, 14},   {-4, 13},  {-7, 12},  {-10, 10},
    {-12, 7},  {-14, 4},  {-14, 0},   {-14, -4}, {-12, -7}, {-10, -10}, {-7, -12}, {-4, -13},
};

void drawRing(Adafruit_ST7789& tft, uint8_t points, uint16_t color) {
  const uint8_t count = points > 24 ? 24 : points;
  for (uint8_t i = 0; i < count; i++) {
    tft.fillCircle(kRingCx + kRing[i][0], kRingCy + kRing[i][1], 2, color);
  }
}

}  // namespace

Quota::Quota(PetStateMachine& state, Display& display) : state_(state), display_(display) {}

uint16_t Quota::barColor(Display& display, uint8_t percent) {
  Adafruit_ST7789& tft = display.tft();
  if (percent > 50) {
    return tft.color565(60, 180, 90);
  }
  if (percent >= 20) {
    return tft.color565(220, 180, 40);
  }
  return tft.color565(200, 50, 50);
}

void Quota::setPercent(uint8_t percent) {
  state_.quotaPercent = percent > 100 ? 100 : percent;
  state_.quotaVisible = true;
  needsRedraw_ = true;
}

void Quota::setVisible(bool visible) {
  state_.quotaVisible = visible;
  needsRedraw_ = true;
}

void Quota::drawBar() {
  if (!state_.quotaVisible) {
    return;
  }
  Adafruit_ST7789& tft = display_.tft();
  tft.fillRect(156, 2, 80, 40, display_.animBgColor());
  drawRing(tft, 24, display_.colorDarkBg());
  const uint8_t lit =
      state_.quotaPercent >= 100 ? 24 : static_cast<uint8_t>((state_.quotaPercent * 24) / 100);
  if (lit > 0) {
    drawRing(tft, lit, barColor(display_, state_.quotaPercent));
  }
  tft.fillRoundRect(158, 12, 40, 16, 4, Display::kWhite);
  tft.setTextColor(Display::kBlack);
  tft.setTextSize(1);
  char label[8] = {};
  snprintf(label, sizeof(label), "%u%%", static_cast<unsigned>(state_.quotaPercent));
  tft.setCursor(164, 16);
  tft.print(label);
}

void Quota::tick() {
  if (needsRedraw_) {
    drawBar();
    needsRedraw_ = false;
  }
}
