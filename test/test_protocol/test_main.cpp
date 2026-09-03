#include <cstring>

#include <unity.h>

#include "JsonProtocol.h"
#include "PairingWindow.h"
#include "States.h"

void setUp() {}
void tearDown() {}

uint8_t gSessionStartHookCount = 0;
uint8_t gSessionEndHookCount = 0;

void onSessionStartHook() { gSessionStartHookCount++; }
void onSessionEndHook() { gSessionEndHookCount++; }

static void test_malformed_json() {
  const char* bad = "{not json";
  const JsonParseResult r = JsonProtocol::parseCommand(bad, strlen(bad));
  TEST_ASSERT_FALSE(r.ok);
  TEST_ASSERT_NOT_NULL(r.error);
}

static void test_missing_fields() {
  const char* json = "{\"id\":\"abc\"}";
  const JsonParseResult r = JsonProtocol::parseCommand(json, strlen(json));
  TEST_ASSERT_FALSE(r.ok);
}

static void test_session_start_ack_shape() {
  PetStateMachine sm;
  const char* json = "{\"id\":\"e1\",\"event\":\"sessionStart\",\"state\":\"coding\"}";
  const JsonParseResult parsed = JsonProtocol::parseCommand(json, strlen(json));
  TEST_ASSERT_TRUE(parsed.ok);
  TEST_ASSERT_EQUAL(TransitionResult::OK, JsonProtocol::applyEvent(sm, parsed));
  TEST_ASSERT_EQUAL(static_cast<int>(PetMode::AGENT), static_cast<int>(sm.mode));
  TEST_ASSERT_EQUAL(static_cast<int>(AgentExpression::CODING), static_cast<int>(sm.expression));

  char ack[192] = {};
  const size_t n = JsonProtocol::buildAckJson("e1", true, sm, nullptr, ack, sizeof(ack));
  TEST_ASSERT_GREATER_THAN(0, n);
  TEST_ASSERT_NOT_NULL(strstr(ack, "\"ok\":true"));
  TEST_ASSERT_NOT_NULL(strstr(ack, "\"mode\":\"agent\""));
  TEST_ASSERT_NOT_NULL(strstr(ack, "\"state\":\"coding\""));
}

static void test_invalid_session_start_preserves_existing_state() {
  PetStateMachine sm;
  TEST_ASSERT_EQUAL(TransitionResult::OK, sm.onSessionStart("thinking"));
  TEST_ASSERT_EQUAL(TransitionResult::OK, sm.onAttentionRequired());
  sm.attentionLevel = AttentionLevel::FAST_PULSE;

  const char* json = "{\"id\":\"bad\",\"event\":\"sessionStart\",\"state\":\"not-a-state\"}";
  const JsonParseResult parsed = JsonProtocol::parseCommand(json, strlen(json));
  TEST_ASSERT_TRUE(parsed.ok);
  TEST_ASSERT_EQUAL(TransitionResult::INVALID, JsonProtocol::applyEvent(sm, parsed));
  TEST_ASSERT_EQUAL(static_cast<int>(PetMode::AGENT), static_cast<int>(sm.mode));
  TEST_ASSERT_EQUAL(static_cast<int>(AgentExpression::THINKING), static_cast<int>(sm.expression));
  TEST_ASSERT_TRUE(sm.attentionActive);
  TEST_ASSERT_EQUAL(static_cast<int>(AttentionLevel::FAST_PULSE),
                    static_cast<int>(sm.attentionLevel));
}

static void test_quota_values_are_clamped_before_narrowing() {
  PetStateMachine sm;
  const char* json = "{\"id\":\"quota\",\"event\":\"quotaUpdate\",\"quota\":256}";
  const JsonParseResult parsed = JsonProtocol::parseCommand(json, strlen(json));
  TEST_ASSERT_TRUE(parsed.ok);
  TEST_ASSERT_EQUAL(TransitionResult::OK, JsonProtocol::applyEvent(sm, parsed));
  TEST_ASSERT_EQUAL_UINT8(100, sm.quotaPercent);
}

static void test_security_event_preserves_underlying_expression() {
  PetStateMachine sm;
  TEST_ASSERT_EQUAL(TransitionResult::OK, sm.onSessionStart("thinking"));
  TEST_ASSERT_EQUAL(TransitionResult::OK, sm.onSecurityEvent("ssh_login"));
  TEST_ASSERT_EQUAL(static_cast<int>(AgentExpression::THINKING), static_cast<int>(sm.expression));
}

static void test_session_hooks_run_only_after_successful_transitions() {
  PetStateMachine sm;
  gSessionStartHookCount = 0;
  gSessionEndHookCount = 0;
  JsonProtocol::setSessionHooks(onSessionStartHook, onSessionEndHook);

  const char* invalidJson = "{\"event\":\"sessionStart\",\"state\":\"invalid\"}";
  const JsonParseResult invalid = JsonProtocol::parseCommand(invalidJson, strlen(invalidJson));
  TEST_ASSERT_EQUAL(TransitionResult::INVALID, JsonProtocol::applyEvent(sm, invalid));
  TEST_ASSERT_EQUAL_UINT8(0, gSessionStartHookCount);

  const char* startJson = "{\"event\":\"sessionStart\",\"state\":\"coding\"}";
  const JsonParseResult start = JsonProtocol::parseCommand(startJson, strlen(startJson));
  TEST_ASSERT_EQUAL(TransitionResult::OK, JsonProtocol::applyEvent(sm, start));
  TEST_ASSERT_EQUAL_UINT8(1, gSessionStartHookCount);

  const char* endJson = "{\"event\":\"sessionEnd\"}";
  const JsonParseResult end = JsonProtocol::parseCommand(endJson, strlen(endJson));
  TEST_ASSERT_EQUAL(TransitionResult::OK, JsonProtocol::applyEvent(sm, end));
  TEST_ASSERT_EQUAL_UINT8(1, gSessionEndHookCount);
  JsonProtocol::setSessionHooks(nullptr, nullptr);
}

static void test_wander_agent_wander_transitions() {
  PetStateMachine sm;
  TEST_ASSERT_EQUAL(static_cast<int>(PetMode::WANDER), static_cast<int>(sm.mode));

  TEST_ASSERT_EQUAL(TransitionResult::OK, sm.onSessionStart("thinking"));
  TEST_ASSERT_EQUAL(static_cast<int>(PetMode::AGENT), static_cast<int>(sm.mode));
  TEST_ASSERT_EQUAL(static_cast<int>(AgentExpression::THINKING), static_cast<int>(sm.expression));

  TEST_ASSERT_EQUAL(TransitionResult::OK, sm.onSessionEnd());
  TEST_ASSERT_EQUAL(static_cast<int>(PetMode::WANDER), static_cast<int>(sm.mode));
  TEST_ASSERT_EQUAL(static_cast<int>(AgentExpression::IDLE), static_cast<int>(sm.expression));
}

static void test_status_json_fields() {
  PetStateMachine sm;
  sm.onSessionStart("idle");
  char status[256] = {};
  const size_t n = JsonProtocol::buildStatusJson(sm, status, sizeof(status));
  TEST_ASSERT_GREATER_THAN(0, n);
  TEST_ASSERT_NOT_NULL(strstr(status, "\"mode\":\"agent\""));
  TEST_ASSERT_NOT_NULL(strstr(status, "\"ble_connected\":false"));
  TEST_ASSERT_NOT_NULL(strstr(status, "\"secure_session\":false"));
  TEST_ASSERT_NOT_NULL(strstr(status, "\"pairing_active\":false"));

  sm.bleConnected = true;
  memset(status, 0, sizeof(status));
  const size_t secureN = JsonProtocol::buildStatusJson(sm, status, sizeof(status));
  TEST_ASSERT_GREATER_THAN(0, secureN);
  TEST_ASSERT_NOT_NULL(strstr(status, "\"ble_connected\":true"));
  TEST_ASSERT_NOT_NULL(strstr(status, "\"secure_session\":true"));
}

static void test_ack_error_shape() {
  PetStateMachine sm;
  char ack[192] = {};
  const size_t n = JsonProtocol::buildAckJson("x1", false, sm, "invalid transition", ack, sizeof(ack));
  TEST_ASSERT_GREATER_THAN(0, n);
  TEST_ASSERT_NOT_NULL(strstr(ack, "\"ok\":false"));
  TEST_ASSERT_NOT_NULL(strstr(ack, "\"error\":\"invalid transition\""));
}

static void test_oversize_payload() {
  char big[600] = {};
  memset(big, 'a', sizeof(big) - 1);
  big[0] = '{';
  big[1] = '"';
  big[sizeof(big) - 2] = '}';
  const JsonParseResult r = JsonProtocol::parseCommand(big, strlen(big));
  TEST_ASSERT_FALSE(r.ok);
  TEST_ASSERT_NOT_NULL(r.error);
}

static void test_attention_events() {
  PetStateMachine sm;
  sm.onSessionStart("coding");
  TEST_ASSERT_EQUAL(TransitionResult::OK, sm.onAttentionRequired());
  TEST_ASSERT_TRUE(sm.attentionActive);
  TEST_ASSERT_EQUAL(static_cast<int>(AttentionLevel::NONE), static_cast<int>(sm.attentionLevel));
  TEST_ASSERT_EQUAL(TransitionResult::OK, sm.onAttentionCleared());
  TEST_ASSERT_FALSE(sm.attentionActive);
}

static void test_pairing_json_events() {
  PetStateMachine sm;
  const char* startJson = "{\"id\":\"p1\",\"event\":\"pairingStart\"}";
  const JsonParseResult startParsed = JsonProtocol::parseCommand(startJson, strlen(startJson));
  TEST_ASSERT_TRUE(startParsed.ok);
  TEST_ASSERT_EQUAL(TransitionResult::OK, JsonProtocol::applyEvent(sm, startParsed));
  TEST_ASSERT_TRUE(sm.pairingActive);

  const char* stopJson = "{\"id\":\"p2\",\"event\":\"pairingStop\"}";
  const JsonParseResult stopParsed = JsonProtocol::parseCommand(stopJson, strlen(stopJson));
  TEST_ASSERT_TRUE(stopParsed.ok);
  TEST_ASSERT_EQUAL(TransitionResult::OK, JsonProtocol::applyEvent(sm, stopParsed));
  TEST_ASSERT_FALSE(sm.pairingActive);

  sm.pairingActive = true;
  const char* unbondJson = "{\"id\":\"p3\",\"event\":\"unbond\"}";
  const JsonParseResult unbondParsed = JsonProtocol::parseCommand(unbondJson, strlen(unbondJson));
  TEST_ASSERT_TRUE(unbondParsed.ok);
  TEST_ASSERT_EQUAL(TransitionResult::OK, JsonProtocol::applyEvent(sm, unbondParsed));
  TEST_ASSERT_FALSE(sm.pairingActive);
}

static void test_sleep_wake_json_events() {
  PetStateMachine sm;
  const char* sleepJson = "{\"id\":\"s1\",\"event\":\"sleep\"}";
  const JsonParseResult sleepParsed = JsonProtocol::parseCommand(sleepJson, strlen(sleepJson));
  TEST_ASSERT_TRUE(sleepParsed.ok);
  TEST_ASSERT_EQUAL(TransitionResult::OK, JsonProtocol::applyEvent(sm, sleepParsed));
  TEST_ASSERT_EQUAL(static_cast<int>(PetMode::SLEEP), static_cast<int>(sm.mode));

  const char* wakeJson = "{\"id\":\"s2\",\"event\":\"wake\"}";
  const JsonParseResult wakeParsed = JsonProtocol::parseCommand(wakeJson, strlen(wakeJson));
  TEST_ASSERT_TRUE(wakeParsed.ok);
  TEST_ASSERT_EQUAL(TransitionResult::OK, JsonProtocol::applyEvent(sm, wakeParsed));
  TEST_ASSERT_EQUAL(static_cast<int>(PetMode::WANDER), static_cast<int>(sm.mode));
}

static void test_pairing_window_rejects_new_central_when_closed() {
  PairingWindow window;

  TEST_ASSERT_FALSE(window.allowsNewBond(100));
}

static void test_pairing_window_allows_new_central_until_expiry() {
  PairingWindow window;
  window.open(100, 123456);

  TEST_ASSERT_TRUE(window.allowsNewBond(100));
  TEST_ASSERT_TRUE(window.allowsNewBond(100 + PairingWindow::kDurationMs - 1));
  TEST_ASSERT_FALSE(window.allowsNewBond(100 + PairingWindow::kDurationMs));
}

static void test_pairing_window_expiry_is_wrap_safe() {
  PairingWindow window;
  const uint32_t openedAt = UINT32_MAX - 50;
  window.open(openedAt, 123456);

  TEST_ASSERT_TRUE(window.allowsNewBond(20));
  TEST_ASSERT_FALSE(window.allowsNewBond(openedAt + PairingWindow::kDurationMs));
}

static void test_pairing_window_reuses_passkey_until_closed() {
  PairingWindow window;
  window.open(42, 123456);

  TEST_ASSERT_EQUAL_UINT32(123456, window.passkey());
  TEST_ASSERT_EQUAL_UINT32(123456, window.passkey());
  window.close();
  TEST_ASSERT_FALSE(window.hasPasskey());
}

int main(int argc, char** argv) {
  UNITY_BEGIN();
  RUN_TEST(test_malformed_json);
  RUN_TEST(test_missing_fields);
  RUN_TEST(test_oversize_payload);
  RUN_TEST(test_attention_events);
  RUN_TEST(test_session_start_ack_shape);
  RUN_TEST(test_invalid_session_start_preserves_existing_state);
  RUN_TEST(test_quota_values_are_clamped_before_narrowing);
  RUN_TEST(test_security_event_preserves_underlying_expression);
  RUN_TEST(test_session_hooks_run_only_after_successful_transitions);
  RUN_TEST(test_wander_agent_wander_transitions);
  RUN_TEST(test_status_json_fields);
  RUN_TEST(test_ack_error_shape);
  RUN_TEST(test_pairing_json_events);
  RUN_TEST(test_sleep_wake_json_events);
  RUN_TEST(test_pairing_window_rejects_new_central_when_closed);
  RUN_TEST(test_pairing_window_allows_new_central_until_expiry);
  RUN_TEST(test_pairing_window_expiry_is_wrap_safe);
  RUN_TEST(test_pairing_window_reuses_passkey_until_closed);
  return UNITY_END();
}
