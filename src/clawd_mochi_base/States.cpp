#include "States.h"

#include <string.h>

namespace {

bool streq(const char* a, const char* b) {
  return a != nullptr && b != nullptr && strcmp(a, b) == 0;
}

}  // namespace

const char* PetStateMachine::modeName(PetMode mode) {
  switch (mode) {
    case PetMode::WANDER:
      return "wander";
    case PetMode::AGENT:
      return "agent";
    case PetMode::SLEEP:
      return "sleep";
  }
  return "unknown";
}

const char* PetStateMachine::expressionName(AgentExpression expr) {
  switch (expr) {
    case AgentExpression::IDLE:
      return "idle";
    case AgentExpression::CODING:
      return "coding";
    case AgentExpression::THINKING:
      return "thinking";
    case AgentExpression::WAITING:
      return "waiting_for_user";
    case AgentExpression::ERROR:
      return "error";
    case AgentExpression::SECURITY:
      return "security";
  }
  return "unknown";
}

bool PetStateMachine::parseExpression(const char* name, AgentExpression& out) {
  if (name == nullptr) {
    return false;
  }
  if (streq(name, "idle")) {
    out = AgentExpression::IDLE;
    return true;
  }
  if (streq(name, "coding")) {
    out = AgentExpression::CODING;
    return true;
  }
  if (streq(name, "thinking")) {
    out = AgentExpression::THINKING;
    return true;
  }
  if (streq(name, "waiting") || streq(name, "waiting_for_user")) {
    out = AgentExpression::WAITING;
    return true;
  }
  if (streq(name, "error")) {
    out = AgentExpression::ERROR;
    return true;
  }
  if (streq(name, "security")) {
    out = AgentExpression::SECURITY;
    return true;
  }
  return false;
}

const char* PetStateMachine::attentionLevelName(AttentionLevel level) {
  switch (level) {
    case AttentionLevel::NONE:
      return "none";
    case AttentionLevel::URGENT:
      return "urgent";
    case AttentionLevel::SLOW_PULSE:
      return "slow_pulse";
    case AttentionLevel::FAST_PULSE:
      return "fast_pulse";
  }
  return "unknown";
}

TransitionResult PetStateMachine::onSessionStart(const char* state) {
  if (mode == PetMode::SLEEP) {
    return TransitionResult::IGNORED;
  }
  AgentExpression parsed = AgentExpression::CODING;
  if (state != nullptr && !parseExpression(state, parsed)) {
    return TransitionResult::INVALID;
  }
  attentionActive = false;
  attentionLevel = AttentionLevel::NONE;
  mode = PetMode::AGENT;
  expression = parsed;
  return TransitionResult::OK;
}

TransitionResult PetStateMachine::onSessionEnd() {
  if (mode == PetMode::SLEEP) {
    return TransitionResult::IGNORED;
  }
  attentionActive = false;
  attentionLevel = AttentionLevel::NONE;
  mode = PetMode::WANDER;
  expression = AgentExpression::IDLE;
  return TransitionResult::OK;
}

TransitionResult PetStateMachine::onBleDisconnect() {
  if (mode == PetMode::SLEEP) {
    return TransitionResult::IGNORED;
  }
  const bool hadAgentSession = mode == PetMode::AGENT;
  attentionActive = false;
  attentionLevel = AttentionLevel::NONE;
  securityOverlayActive = false;
  quotaVisible = false;
  securityTypeLabel[0] = '\0';
  mode = PetMode::WANDER;
  expression = AgentExpression::IDLE;
  return hadAgentSession ? TransitionResult::OK : TransitionResult::IGNORED;
}

TransitionResult PetStateMachine::onStateUpdate(const char* state) {
  if (mode != PetMode::AGENT) {
    return TransitionResult::IGNORED;
  }
  AgentExpression parsed;
  if (!parseExpression(state, parsed)) {
    return TransitionResult::INVALID;
  }
  expression = parsed;
  return TransitionResult::OK;
}

TransitionResult PetStateMachine::onSleep() {
  mode = PetMode::SLEEP;
  backlightPercent = 0;
  return TransitionResult::OK;
}

TransitionResult PetStateMachine::onWake() {
  if (mode != PetMode::SLEEP) {
    return TransitionResult::IGNORED;
  }
  mode = PetMode::WANDER;
  expression = AgentExpression::IDLE;
  backlightPercent = 100;
  return TransitionResult::OK;
}

TransitionResult PetStateMachine::onTestExpression(const char* expr) {
  AgentExpression parsed;
  if (!parseExpression(expr, parsed)) {
    return TransitionResult::INVALID;
  }
  if (mode == PetMode::SLEEP) {
    return TransitionResult::IGNORED;
  }
  mode = PetMode::AGENT;
  expression = parsed;
  return TransitionResult::OK;
}

TransitionResult PetStateMachine::onPairingStart() {
  pairingActive = true;
  return TransitionResult::OK;
}

TransitionResult PetStateMachine::onPairingStop() {
  pairingActive = false;
  return TransitionResult::OK;
}

TransitionResult PetStateMachine::onUnbond() {
  pairingActive = false;
  bleConnected = false;
  return TransitionResult::OK;
}

TransitionResult PetStateMachine::onAttentionRequired() {
  if (mode != PetMode::AGENT) {
    return TransitionResult::IGNORED;
  }
  attentionActive = true;
  attentionLevel = AttentionLevel::NONE;
  return TransitionResult::OK;
}

TransitionResult PetStateMachine::onAttentionCleared() {
  attentionActive = false;
  attentionLevel = AttentionLevel::NONE;
  return TransitionResult::OK;
}

TransitionResult PetStateMachine::onSecurityEvent(const char* typeLabel) {
  (void)typeLabel;
  if (mode == PetMode::SLEEP) {
    return TransitionResult::IGNORED;
  }
  return TransitionResult::OK;
}

TransitionResult PetStateMachine::onQuotaUpdate(uint8_t percent) {
  quotaPercent = percent > 100 ? 100 : percent;
  quotaVisible = true;
  return TransitionResult::OK;
}
