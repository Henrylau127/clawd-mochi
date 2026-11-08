#pragma once

#include <stddef.h>
#include <stdint.h>

#include "../clawd_mochi_base/States.h"

struct JsonParseResult {
  bool ok = false;
  const char* error = nullptr;
  char id[16] = {};
  char event[32] = {};
  char state[32] = {};
  char detailType[32] = {};
  int quotaPercent = -1;
  bool hasCaption = false;
  bool hasMood = false;
  char caption[48] = {};
  char mood[16] = {};
};

struct JsonProtocol {
  static constexpr size_t kStatusJsonCapacity = 512;

  static JsonParseResult parseCommand(const char* json, size_t len);
  static size_t buildStatusJson(const PetStateMachine& sm, char* out, size_t outLen);
  static size_t buildAckJson(const char* id, bool ok, const PetStateMachine& sm,
                             const char* errorMsg, char* out, size_t outLen);
  static TransitionResult applyEvent(PetStateMachine& sm, const JsonParseResult& parsed);
  static void setEventHooks(void (*onAttentionRequired)(), void (*onAttentionCleared)(),
                            void (*onSecurityEvent)(const char*), void (*onQuotaUpdate)(uint8_t));
  static void setSessionHooks(void (*onSessionStart)(), void (*onSessionEnd)());
  static void setPairingHooks(void (*onPairingStart)(), void (*onPairingStop)(), void (*onUnbond)());
};
