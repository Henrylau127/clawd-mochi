#include "SerialDiag.h"

#include <Arduino.h>
#include <esp_mac.h>

#include "../transport/BleSecurity.h"
#include "../transport/BleTransport.h"
#include "Config.h"
#include "JsonProtocol.h"

namespace {

constexpr uint32_t kUsbCdcWaitMs = 3000;

String deviceSuffixFromMac() {
  uint8_t mac[6] = {};
  esp_read_mac(mac, ESP_MAC_WIFI_STA);
  char suffix[8] = {};
  snprintf(suffix, sizeof(suffix), "%02X%02X", mac[4], mac[5]);
  return String(suffix);
}

}  // namespace

SerialDiag::SerialDiag(PetStateMachine& state, Face& face, Display& display, BleSecurity* bleSecurity,
                       BleTransport* bleTransport)
    : state_(state),
      face_(face),
      display_(display),
      bleSecurity_(bleSecurity),
      bleTransport_(bleTransport) {}

void SerialDiag::begin() {
  Serial.begin(115200);
  const uint32_t start = millis();
  while (!Serial && (millis() - start) < kUsbCdcWaitMs) {
    delay(10);
  }

  const String suffix = deviceSuffixFromMac();
  char deviceName[16] = {};
  snprintf(deviceName, sizeof(deviceName), "DP-%s", suffix.c_str());
  printBootBanner(deviceName);
}

void SerialDiag::printBootBanner(const char* deviceName) {
  Serial.println();
  Serial.print(F("Desk Pet fw="));
  Serial.print(F(DESKPET_FW_VERSION));
  Serial.print(F(" device="));
  Serial.print(deviceName);
  Serial.print(F(" heap="));
  Serial.println(ESP.getFreeHeap());
  Serial.println(F("Type HELP for serial commands."));
}

String SerialDiag::trimUpper(const String& s) {
  String out = s;
  out.trim();
  out.toUpperCase();
  return out;
}

bool SerialDiag::isSensitiveToken(const char* token) {
  if (token == nullptr) {
    return false;
  }
  static const char* kSensitive[] = {"PASSKEY", "PASSWORD", "TOKEN", "SECRET", "BOND"};
  for (const char* item : kSensitive) {
    if (strcasecmp(token, item) == 0) {
      return true;
    }
  }
  return false;
}

void SerialDiag::replyOk(const char* msg) {
  if (msg != nullptr) {
    Serial.print(F("OK "));
    Serial.println(msg);
  } else {
    Serial.println(F("OK"));
  }
}

void SerialDiag::replyErr(const char* msg) {
  Serial.print(F("ERR "));
  Serial.println(msg);
}

void SerialDiag::printHelp() const {
  Serial.println(F("Commands:"));
  Serial.println(F("  STATUS"));
  Serial.println(F("  TEST coding|thinking|idle|waiting|error|security|celebrate"));
  Serial.println(F("  PAIR"));
  Serial.println(F("  UNBOND"));
  Serial.println(F("  SLEEP"));
  Serial.println(F("  WAKE"));
  Serial.println(F("  WANDER"));
  Serial.println(F("  HELP"));
}

void SerialDiag::handleLine(const String& line) {
  if (line.length() == 0) {
    return;
  }

  int spaceIdx = line.indexOf(' ');
  const String cmd = trimUpper(spaceIdx >= 0 ? line.substring(0, spaceIdx) : line);
  const String arg = spaceIdx >= 0 ? line.substring(spaceIdx + 1) : String();

  if (isSensitiveToken(cmd.c_str())) {
    replyErr("redacted");
    return;
  }

  if (cmd == F("STATUS")) {
    char json[JsonProtocol::kStatusJsonCapacity] = {};
    const size_t n = JsonProtocol::buildStatusJson(state_, json, sizeof(json));
    if (n == 0) {
      replyErr("status encode failed");
      return;
    }
    Serial.println(json);
    return;
  }

  if (cmd == F("HELP")) {
    printHelp();
    return;
  }

  if (cmd == F("TEST")) {
    if (arg.length() == 0) {
      replyErr("missing expression");
      return;
    }
    const TransitionResult tr = state_.onTestExpression(arg.c_str());
    if (tr == TransitionResult::INVALID) {
      replyErr("unknown expression");
      return;
    }
    if (tr == TransitionResult::IGNORED) {
      replyErr("ignored in sleep");
      return;
    }
    needsRedraw_ = true;
    replyOk(PetStateMachine::expressionName(state_.expression));
    return;
  }

  if (cmd == F("PAIR")) {
    state_.onPairingStart();
    if (bleSecurity_ != nullptr) {
      bleSecurity_->startPairingWindow();
    }
    needsRedraw_ = true;
    replyOk("pairing window opened");
    return;
  }

  if (cmd == F("UNBOND")) {
    if (bleSecurity_ != nullptr) {
      bleSecurity_->eraseBonds();
    } else {
      state_.onUnbond();
    }
    needsRedraw_ = true;
    replyOk("bonds cleared");
    return;
  }

  if (cmd == F("SLEEP")) {
    state_.onSleep();
    display_.setBacklightPercent(0);
    needsRedraw_ = true;
    replyOk("sleep");
    return;
  }

  if (cmd == F("WAKE")) {
    const TransitionResult tr = state_.onWake();
    if (tr == TransitionResult::IGNORED) {
      replyErr("not sleeping");
      return;
    }
    display_.setBacklightPercent(state_.backlightPercent);
    needsRedraw_ = true;
    replyOk("wake");
    return;
  }

  if (cmd == F("WANDER")) {
    if (state_.mode == PetMode::SLEEP) {
      replyErr("ignored in sleep");
      return;
    }
    state_.onSessionEnd();
    needsRedraw_ = true;
    replyOk("wander");
    return;
  }

  if (line.startsWith("{")) {
    const JsonParseResult parsed = JsonProtocol::parseCommand(line.c_str(), line.length());
    if (!parsed.ok) {
      replyErr(parsed.error != nullptr ? parsed.error : "parse failed");
      return;
    }
    const TransitionResult tr = JsonProtocol::applyEvent(state_, parsed);
    if (tr == TransitionResult::INVALID) {
      char ack[192] = {};
      JsonProtocol::buildAckJson(parsed.id, false, state_, "invalid transition", ack, sizeof(ack));
      Serial.println(ack);
      return;
    }
    needsRedraw_ = true;
    char ack[192] = {};
    JsonProtocol::buildAckJson(parsed.id, true, state_, nullptr, ack, sizeof(ack));
    Serial.println(ack);
    return;
  }

  replyErr("unknown command");
}

void SerialDiag::poll() {
  while (Serial.available() > 0) {
    const char c = static_cast<char>(Serial.read());
    if (c == '\r') {
      continue;
    }
    if (c == '\n') {
      handleLine(lineBuffer_);
      lineBuffer_ = "";
      continue;
    }
    if (lineBuffer_.length() < 256) {
      lineBuffer_ += c;
    }
  }
}
