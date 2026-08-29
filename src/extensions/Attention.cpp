#include "Attention.h"

#include <Arduino.h>

#include "../clawd_mochi_base/Display.h"
#include "../clawd_mochi_base/Face.h"

Attention::Attention(PetStateMachine& state, Face& face, Display& display)
    : state_(state), face_(face), display_(display) {}

void Attention::reset() {
  active_ = false;
  attentionStartMs_ = 0;
  state_.attentionLevel = AttentionLevel::NONE;
  pulseBrightness_ = 100;
  needsRedraw_ = true;
}

void Attention::onAttentionRequired() {
  active_ = true;
  attentionStartMs_ = millis();
  state_.attentionLevel = AttentionLevel::NONE;
  needsRedraw_ = true;
}

void Attention::onAttentionCleared() {
  reset();
}

void Attention::onSessionChange() { reset(); }

void Attention::updateLevel(uint32_t nowMs) {
  if (!active_ || attentionStartMs_ == 0) {
    return;
  }
  const uint32_t elapsed = nowMs - attentionStartMs_;
  const AttentionLevel prev = state_.attentionLevel;
  if (elapsed >= kFastPulseMs) {
    state_.attentionLevel = AttentionLevel::FAST_PULSE;
  } else if (elapsed >= kSlowPulseMs) {
    state_.attentionLevel = AttentionLevel::SLOW_PULSE;
  } else if (elapsed >= kUrgentMs) {
    state_.attentionLevel = AttentionLevel::URGENT;
  }
  if (prev != state_.attentionLevel) {
    needsRedraw_ = true;
  }
}

void Attention::tick(uint32_t nowMs) {
  if (state_.mode != PetMode::AGENT) {
    return;
  }
  updateLevel(nowMs);
  if (!active_ || state_.attentionLevel < AttentionLevel::SLOW_PULSE) {
    return;
  }

  const uint32_t period =
      state_.attentionLevel == AttentionLevel::FAST_PULSE ? kFastPulsePeriodMs : kSlowPulsePeriodMs;
  if ((nowMs - lastPulseMs_) < period) {
    return;
  }
  lastPulseMs_ = nowMs;
  pulseHigh_ = !pulseHigh_;
  pulseBrightness_ = pulseHigh_ ? 100 : 20;
  needsRedraw_ = true;
}
