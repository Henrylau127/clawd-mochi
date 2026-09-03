#include <cstring>

#include <unity.h>

#include "JsonProtocol.h"

#include "States.h"

void setUp() {}
void tearDown() {}

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
  TEST_ASSERT_NOT_NULL(strstr(status, "\"pairing_active\":false"));
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

int main(int argc, char** argv) {
  UNITY_BEGIN();
  RUN_TEST(test_malformed_json);
  RUN_TEST(test_missing_fields);
  RUN_TEST(test_oversize_payload);
  RUN_TEST(test_attention_events);
  RUN_TEST(test_session_start_ack_shape);
  RUN_TEST(test_wander_agent_wander_transitions);
  RUN_TEST(test_status_json_fields);
  RUN_TEST(test_ack_error_shape);
  RUN_TEST(test_pairing_json_events);
  return UNITY_END();
}
