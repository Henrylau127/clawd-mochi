#include "Quota.h"

#include "../clawd_mochi_base/Display.h"
#include "../clawd_mochi_base/States.h"

namespace {

constexpr int16_t kBarX = 20;
constexpr int16_t kBarY = 220;
constexpr int16_t kBarW = 200;
constexpr int16_t kBarH = 10;

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
  tft.fillRect(kBarX, kBarY, kBarW, kBarH, display_.colorDarkBg());
  const int16_t fillW = static_cast<int16_t>((kBarW * state_.quotaPercent) / 100);
  if (fillW > 0) {
    tft.fillRect(kBarX, kBarY, fillW, kBarH, barColor(display_, state_.quotaPercent));
  }
  tft.drawRect(kBarX, kBarY, kBarW, kBarH, Display::kWhite);
}

void Quota::tick() {
  if (needsRedraw_) {
    drawBar();
    needsRedraw_ = false;
  }
}
