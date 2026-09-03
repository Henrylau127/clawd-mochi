#include "WanderScreens.h"

#include <Arduino.h>

#include "../clawd_mochi_base/Display.h"
#include "../clawd_mochi_base/Face.h"
#include "../clawd_mochi_base/States.h"

WanderScreens::WanderScreens(PetStateMachine& state, Face& face, Display& display)
    : state_(state), face_(face), display_(display) {}

void WanderScreens::reset() {
  screen_ = WanderScreen::IDLE;
  screenStartMs_ = millis();
  lastAnimMs_ = screenStartMs_;
  blinkOpen_ = true;
  lookOffset_ = 0;
  lookDir_ = 1;
  idlePrimed_ = false;
  needsRedraw_ = true;
}

void WanderScreens::advanceScreen(uint32_t nowMs) {
  screenStartMs_ = nowMs;
  idlePrimed_ = false;
  switch (screen_) {
    case WanderScreen::IDLE:
      screen_ = WanderScreen::AMBIENT;
      regenerateAmbientStars();
      break;
    case WanderScreen::AMBIENT:
      screen_ = WanderScreen::IDLE;
      break;
  }
  needsRedraw_ = true;
}

void WanderScreens::regenerateAmbientStars() {
  for (uint8_t i = 0; i < kStarCount; i++) {
    stars_[i].x = static_cast<int16_t>(esp_random() % DISP_W);
    stars_[i].y = static_cast<int16_t>(esp_random() % DISP_H);
    stars_[i].phase = static_cast<uint8_t>(esp_random());
  }
}

void WanderScreens::drawAmbientScreen(uint32_t nowMs, bool fullRedraw) {
  Adafruit_ST7789& tft = display_.tft();
  const uint16_t bg = tft.color565(12, 18, 40);
  display_.setAnimBgColor(bg);
  if (fullRedraw) {
    tft.fillScreen(bg);
  }
  for (uint8_t i = 0; i < kStarCount; i++) {
    const AmbientStar& star = stars_[i];
    if (!fullRedraw) {
      tft.fillRect(star.x, star.y, 2, 2, bg);
    }
    const uint8_t wave = static_cast<uint8_t>(((nowMs >> 8) + star.phase) & 0xFF);
    const uint8_t bright = wave < 128 ? static_cast<uint8_t>(wave * 2)
                                      : static_cast<uint8_t>((255 - wave) * 2);
    const uint8_t size = bright > 200 ? 2 : 1;
    const uint8_t blue = bright > 220 ? 255 : static_cast<uint8_t>(bright + 48);
    const uint16_t color = tft.color565(bright, bright, blue);
    tft.fillRect(star.x, star.y, size, size, color);
  }
}

void WanderScreens::drawCurrent(uint32_t nowMs) {
  switch (screen_) {
    case WanderScreen::IDLE: {
      display_.setAnimBgColor(face_.expressionColor(AgentExpression::IDLE));
      face_.drawWanderIdle(blinkOpen_, lookOffset_, !idlePrimed_);
      idlePrimed_ = true;
      break;
    }
    case WanderScreen::AMBIENT:
      drawAmbientScreen(nowMs, !idlePrimed_);
      idlePrimed_ = true;
      break;
  }
  lastAnimMs_ = nowMs;
}

void WanderScreens::tick(uint32_t nowMs) {
  if (state_.mode != PetMode::WANDER || state_.securityOverlayActive) {
    return;
  }

  if (screenStartMs_ == 0) {
    reset();
  }

  if ((nowMs - screenStartMs_) >= kCycleMs) {
    advanceScreen(nowMs);
  }

  if (screen_ == WanderScreen::IDLE) {
    if ((nowMs - lastAnimMs_) > 4500) {
      blinkOpen_ = !blinkOpen_;
      needsRedraw_ = true;
      lastAnimMs_ = nowMs;
    }
    if ((nowMs - lastAnimMs_) > 8000) {
      lookOffset_ = static_cast<int8_t>(lookOffset_ + lookDir_);
      if (lookOffset_ > 6 || lookOffset_ < -6) {
        lookDir_ = static_cast<int8_t>(-lookDir_);
        lookOffset_ = static_cast<int8_t>(lookOffset_ + lookDir_);
      }
      needsRedraw_ = true;
    }
  } else if (screen_ == WanderScreen::AMBIENT) {
    if ((nowMs - lastAnimMs_) >= 280) {
      needsRedraw_ = true;
      lastAnimMs_ = nowMs;
    }
  }

  if (needsRedraw_) {
    drawCurrent(nowMs);
    needsRedraw_ = false;
  }
}
