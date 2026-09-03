#pragma once

#include <stdint.h>

enum class PetMode : uint8_t {
  WANDER = 0,
  AGENT,
  SLEEP,
};

enum class AgentExpression : uint8_t {
  IDLE = 0,
  CODING,
  THINKING,
  WAITING,
  ERROR,
  SECURITY,
};

enum class TransitionResult : uint8_t {
  OK = 0,
  INVALID,
  IGNORED,
};

enum class AttentionLevel : uint8_t {
  NONE = 0,
  URGENT,
  SLOW_PULSE,
  FAST_PULSE,
};

struct PetStateMachine {
  PetMode mode = PetMode::WANDER;
  AgentExpression expression = AgentExpression::IDLE;
  AttentionLevel attentionLevel = AttentionLevel::NONE;
  bool pairingActive = false;
  bool bleConnected = false;
  bool securityOverlayActive = false;
  bool quotaVisible = false;
  bool attentionActive = false;
  uint8_t backlightPercent = 100;
  uint8_t quotaPercent = 0;
  char securityTypeLabel[24] = {};

  TransitionResult onSessionStart(const char* state);
  TransitionResult onSessionEnd();
  TransitionResult onStateUpdate(const char* state);
  TransitionResult onSleep();
  TransitionResult onWake();
  TransitionResult onTestExpression(const char* expr);
  TransitionResult onPairingStart();
  TransitionResult onPairingStop();
  TransitionResult onUnbond();
  TransitionResult onAttentionRequired();
  TransitionResult onAttentionCleared();
  TransitionResult onSecurityEvent(const char* typeLabel);
  TransitionResult onQuotaUpdate(uint8_t percent);

  static const char* attentionLevelName(AttentionLevel level);

  static const char* modeName(PetMode mode);
  static const char* expressionName(AgentExpression expr);
  static bool parseExpression(const char* name, AgentExpression& out);
};
