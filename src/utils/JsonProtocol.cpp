#include "JsonProtocol.h"

#include "Caption.h"
#include "Config.h"
#include <ArduinoJson.h>
#include <string.h>

namespace {

void (*gOnAttentionRequired)() = nullptr;
void (*gOnAttentionCleared)() = nullptr;
void (*gOnSessionStart)() = nullptr;
void (*gOnSessionEnd)() = nullptr;
void (*gOnSecurityEvent)(const char*) = nullptr;
void (*gOnQuotaUpdate)(uint8_t) = nullptr;
void (*gOnPairingStart)() = nullptr;
void (*gOnPairingStop)() = nullptr;
void (*gOnUnbond)() = nullptr;

void applyCaption(PetStateMachine& sm, const JsonParseResult& parsed, bool defaultIfAbsent) {
  if (!parsed.hasCaption) {
    if (defaultIfAbsent) {
      sm.setDefaultCaption();
    }
    return;
  }
  if (parsed.caption[0] == '\0') {
    sm.clearCaption();
    return;
  }
  char cleaned[PetStateMachine::kCaptionCapacity] = {};
  if (sanitizeCaption(parsed.caption, cleaned, sizeof(cleaned)) && cleaned[0] != '\0') {
    sm.setCaption(cleaned);
    return;
  }
  if (defaultIfAbsent) {
    sm.setDefaultCaption();
  }
}

void applyMood(PetStateMachine& sm, const JsonParseResult& parsed) {
  if (!parsed.hasMood || parsed.mood[0] == '\0') {
    sm.clearMood();
    return;
  }
  char cleaned[PetStateMachine::kMoodCapacity] = {};
  if (sanitizeMood(parsed.mood, cleaned, sizeof(cleaned)) && cleaned[0] != '\0') {
    sm.setMood(cleaned);
    return;
  }
  sm.clearMood();
}

}  // namespace

void JsonProtocol::setEventHooks(void (*onAttentionRequired)(), void (*onAttentionCleared)(),
                                 void (*onSecurityEvent)(const char*), void (*onQuotaUpdate)(uint8_t)) {
  gOnAttentionRequired = onAttentionRequired;
  gOnAttentionCleared = onAttentionCleared;
  gOnSecurityEvent = onSecurityEvent;
  gOnQuotaUpdate = onQuotaUpdate;
}

void JsonProtocol::setSessionHooks(void (*onSessionStart)(), void (*onSessionEnd)()) {
  gOnSessionStart = onSessionStart;
  gOnSessionEnd = onSessionEnd;
}

void JsonProtocol::setPairingHooks(void (*onPairingStart)(), void (*onPairingStop)(), void (*onUnbond)()) {
  gOnPairingStart = onPairingStart;
  gOnPairingStop = onPairingStop;
  gOnUnbond = onUnbond;
}

JsonParseResult JsonProtocol::parseCommand(const char* json, size_t len) {
  JsonParseResult result;
  if (json == nullptr || len == 0) {
    result.error = "empty payload";
    return result;
  }
  if (len > 512) {
    result.error = "payload too large";
    return result;
  }

  JsonDocument doc;
  const DeserializationError err = deserializeJson(doc, json, len);
  if (err) {
    result.error = "malformed json";
    return result;
  }
  if (!doc.is<JsonObject>()) {
    result.error = "expected object";
    return result;
  }

  if (doc["id"].is<const char*>()) {
    strlcpy(result.id, doc["id"], sizeof(result.id));
  }
  if (doc["event"].is<const char*>()) {
    strlcpy(result.event, doc["event"], sizeof(result.event));
  }
  if (doc["state"].is<const char*>()) {
    strlcpy(result.state, doc["state"], sizeof(result.state));
  }
  if (doc["quota"].is<int>()) {
    result.quotaPercent = doc["quota"].as<int>();
  }
  if (doc["details"].is<JsonObject>()) {
    const JsonObject details = doc["details"].as<JsonObject>();
    if (details["type"].is<const char*>()) {
      strlcpy(result.detailType, details["type"], sizeof(result.detailType));
    }
  }
  if (doc["caption"].is<const char*>()) {
    result.hasCaption = true;
    strlcpy(result.caption, doc["caption"], sizeof(result.caption));
  }
  if (doc["mood"].is<const char*>()) {
    result.hasMood = true;
    strlcpy(result.mood, doc["mood"], sizeof(result.mood));
  }

  if (result.event[0] == '\0' && result.state[0] == '\0' && result.quotaPercent < 0) {
    result.error = "missing event/state";
    return result;
  }

  result.ok = true;
  return result;
}

TransitionResult JsonProtocol::applyEvent(PetStateMachine& sm, const JsonParseResult& parsed) {
  if (!parsed.ok) {
    return TransitionResult::INVALID;
  }

  if (strcmp(parsed.event, "sessionStart") == 0) {
    const char* state = parsed.state[0] != '\0' ? parsed.state : "coding";
    const TransitionResult tr = sm.onSessionStart(state);
    if (tr == TransitionResult::OK) {
      applyCaption(sm, parsed, false);
      applyMood(sm, parsed);
      if (gOnSessionStart != nullptr) {
        gOnSessionStart();
      }
    }
    return tr;
  }
  if (strcmp(parsed.event, "sessionEnd") == 0) {
    const TransitionResult tr = sm.onSessionEnd();
    if (tr == TransitionResult::OK && gOnSessionEnd != nullptr) {
      gOnSessionEnd();
    }
    return tr;
  }
  if (strcmp(parsed.event, "stateUpdate") == 0) {
    if (parsed.state[0] == '\0') {
      return TransitionResult::INVALID;
    }
    const TransitionResult tr = sm.onStateUpdate(parsed.state);
    if (tr == TransitionResult::OK) {
      applyCaption(sm, parsed, true);
      applyMood(sm, parsed);
    }
    return tr;
  }
  if (strcmp(parsed.event, "sleep") == 0) {
    return sm.onSleep();
  }
  if (strcmp(parsed.event, "wake") == 0) {
    return sm.onWake();
  }
  if (strcmp(parsed.event, "attentionRequired") == 0) {
    const TransitionResult tr = sm.onAttentionRequired();
    if (tr == TransitionResult::OK && gOnAttentionRequired != nullptr) {
      gOnAttentionRequired();
    }
    return tr;
  }
  if (strcmp(parsed.event, "attentionCleared") == 0) {
    const TransitionResult tr = sm.onAttentionCleared();
    if (tr == TransitionResult::OK && gOnAttentionCleared != nullptr) {
      gOnAttentionCleared();
    }
    return tr;
  }
  if (strcmp(parsed.event, "securityEvent") == 0) {
    const TransitionResult tr = sm.onSecurityEvent(parsed.detailType);
    if (tr == TransitionResult::OK && gOnSecurityEvent != nullptr) {
      gOnSecurityEvent(parsed.detailType);
    }
    return tr;
  }
  if (strcmp(parsed.event, "quotaUpdate") == 0 || parsed.quotaPercent >= 0) {
    const int quota = parsed.quotaPercent < 0 ? 0 : parsed.quotaPercent;
    const uint8_t pct = static_cast<uint8_t>(quota > 100 ? 100 : quota);
    const TransitionResult tr = sm.onQuotaUpdate(pct);
    if (tr == TransitionResult::OK && gOnQuotaUpdate != nullptr) {
      gOnQuotaUpdate(pct);
    }
    return tr;
  }
  if (strcmp(parsed.event, "pairingStart") == 0) {
    const TransitionResult tr = sm.onPairingStart();
    if (tr == TransitionResult::OK && gOnPairingStart != nullptr) {
      gOnPairingStart();
    }
    return tr;
  }
  if (strcmp(parsed.event, "pairingStop") == 0) {
    const TransitionResult tr = sm.onPairingStop();
    if (tr == TransitionResult::OK && gOnPairingStop != nullptr) {
      gOnPairingStop();
    }
    return tr;
  }
  if (strcmp(parsed.event, "unbond") == 0) {
    const TransitionResult tr = sm.onUnbond();
    if (tr == TransitionResult::OK && gOnUnbond != nullptr) {
      gOnUnbond();
    }
    return tr;
  }

  if (parsed.state[0] != '\0') {
    return sm.onStateUpdate(parsed.state);
  }

  return TransitionResult::INVALID;
}

size_t JsonProtocol::buildStatusJson(const PetStateMachine& sm, char* out, size_t outLen) {
  JsonDocument doc;
  doc["mode"] = PetStateMachine::modeName(sm.mode);
  doc["state"] = PetStateMachine::expressionName(sm.expression);
  doc["expression"] = PetStateMachine::expressionName(sm.expression);
  doc["backlight"] = sm.backlightPercent;
  doc["ble_connected"] = sm.bleConnected;
  doc["secure_session"] = sm.bleConnected;
  doc["pairing_active"] = sm.pairingActive;
  doc["attention_level"] = PetStateMachine::attentionLevelName(sm.attentionLevel);
  doc["attention_active"] = sm.attentionActive;
  doc["quota_percent"] = sm.quotaPercent;
  doc["quota_visible"] = sm.quotaVisible;
  doc["security_overlay"] = sm.securityOverlayActive;
  doc["fw"] = DESKPET_FW_VERSION;
  if (sm.caption[0] != '\0') {
    doc["caption"] = sm.caption;
  }
  if (sm.mood[0] != '\0') {
    doc["mood"] = sm.mood;
  }

  if (out == nullptr || outLen < 2) {
    return 0;
  }
  const size_t n = serializeJson(doc, out, outLen);
  if (n == 0 || n >= outLen - 1) {
    out[0] = '\0';
    return 0;
  }
  return n;
}

size_t JsonProtocol::buildAckJson(const char* id, bool ok, const PetStateMachine& sm,
                                  const char* errorMsg, char* out, size_t outLen) {
  JsonDocument doc;
  if (id != nullptr && id[0] != '\0') {
    doc["id"] = id;
  }
  doc["ok"] = ok;
  doc["mode"] = PetStateMachine::modeName(sm.mode);
  doc["state"] = PetStateMachine::expressionName(sm.expression);
  if (!ok && errorMsg != nullptr) {
    doc["error"] = errorMsg;
  }
  return serializeJson(doc, out, outLen);
}
