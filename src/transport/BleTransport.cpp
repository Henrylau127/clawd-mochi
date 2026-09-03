#include "BleTransport.h"

#include <Arduino.h>
#include <NimBLEDevice.h>
#include <esp_mac.h>

#include "../clawd_mochi_base/States.h"
#include "../utils/JsonProtocol.h"
#include "BleSecurity.h"

namespace {

constexpr char kServiceUuid[] = "7d3f1b2e-5a8c-4f61-9e09-9a7d4955d001";
constexpr char kCmdRxUuid[] = "7d3f1b2e-5a8c-4f61-9e09-9a7d4955d002";
constexpr char kStatusTxUuid[] = "7d3f1b2e-5a8c-4f61-9e09-9a7d4955d003";
constexpr char kConfigRxUuid[] = "7d3f1b2e-5a8c-4f61-9e09-9a7d4955d004";

BleTransport* gTransport = nullptr;
BleSecurity* gSecurity = nullptr;
NimBLEServer* gServer = nullptr;
NimBLECharacteristic* gStatusTx = nullptr;
void (*gBleRedrawHook)() = nullptr;
void (*gSecureWakeHook)() = nullptr;

bool isAuthorizedSecureConnection(const NimBLEConnInfo& connInfo) {
  if (!connInfo.isEncrypted() || !connInfo.isAuthenticated()) {
    return false;
  }
  return gSecurity == nullptr || gSecurity->allowsConnection(connInfo.isBonded());
}

class DeskPetServerCallbacks : public NimBLEServerCallbacks {
  void onConnect(NimBLEServer* server, NimBLEConnInfo& connInfo) override {
    if (gSecurity != nullptr && !gSecurity->allowsConnection(connInfo.isBonded())) {
      Serial.println(F("BLE rejected: pairing window closed"));
      if (server != nullptr) {
        server->disconnect(connInfo);
      }
      return;
    }
    if (gTransport != nullptr) {
      gTransport->updateConnectionState(true, connInfo.getConnHandle(), connInfo.isBonded());
      gTransport->updateSecurityState(isAuthorizedSecureConnection(connInfo));
    }
    Serial.print(F("BLE connected encrypted="));
    Serial.print(connInfo.isEncrypted() ? F("yes") : F("no"));
    Serial.print(F(" authenticated="));
    Serial.print(connInfo.isAuthenticated() ? F("yes") : F("no"));
    Serial.print(F(" bonded="));
    Serial.println(connInfo.isBonded() ? F("yes") : F("no"));
    if (server != nullptr) {
      server->updateConnParams(connInfo.getConnHandle(), 24, 48, 0, 400);
    }
  }

  void onDisconnect(NimBLEServer* server, NimBLEConnInfo& connInfo, int reason) override {
    (void)connInfo;
    (void)reason;
    if (gTransport != nullptr) {
      gTransport->updateConnectionState(false);
    }
    if (server != nullptr) {
      NimBLEDevice::startAdvertising();
    }
  }

  uint32_t onPassKeyDisplay() override {
    if (gSecurity != nullptr) {
      gSecurity->onPasskeyDisplay();
      return gSecurity->passkey();
    }
    return 0;
  }

  void onAuthenticationComplete(NimBLEConnInfo& connInfo) override {
    const bool secure = isAuthorizedSecureConnection(connInfo);
    if (!secure && gServer != nullptr) {
      gServer->disconnect(connInfo);
    }
    if (gTransport != nullptr) {
      gTransport->updateBondState(connInfo.isBonded());
      gTransport->updateSecurityState(secure);
    }
    if (gSecurity != nullptr) {
      gSecurity->onAuthenticationComplete(secure);
    }
    Serial.print(F("BLE auth complete encrypted="));
    Serial.print(connInfo.isEncrypted() ? F("yes") : F("no"));
    Serial.print(F(" authenticated="));
    Serial.print(connInfo.isAuthenticated() ? F("yes") : F("no"));
    Serial.print(F(" bonded="));
    Serial.println(connInfo.isBonded() ? F("yes") : F("no"));
  }
};

class DeskPetCharacteristicCallbacks : public NimBLECharacteristicCallbacks {
 public:
  explicit DeskPetCharacteristicCallbacks(bool isConfig) : isConfig_(isConfig) {}

  void onRead(NimBLECharacteristic* characteristic, NimBLEConnInfo& connInfo) override {
    if (gTransport == nullptr || characteristic == nullptr ||
        strcmp(characteristic->getUUID().toString().c_str(), kStatusTxUuid) != 0) {
      return;
    }
    if (!isAuthorizedSecureConnection(connInfo)) {
      Serial.println(F("BLE read rejected: insecure connection"));
      if (gServer != nullptr) {
        gServer->disconnect(connInfo);
      }
      return;
    }
    gTransport->refreshStatusValue();
  }

  void onWrite(NimBLECharacteristic* characteristic, NimBLEConnInfo& connInfo) override {
    if (gTransport == nullptr || characteristic == nullptr) {
      return;
    }
    if (!isAuthorizedSecureConnection(connInfo)) {
      Serial.println(F("BLE write rejected: insecure connection"));
      if (gServer != nullptr) {
        gServer->disconnect(connInfo);
      }
      return;
    }
    const std::string& value = characteristic->getValue();
    Serial.print(F("BLE write channel="));
    Serial.print(isConfig_ ? F("config") : F("command"));
    Serial.print(F(" len="));
    Serial.println(value.size());
    if (isConfig_) {
      gTransport->handleConfigWrite(reinterpret_cast<const uint8_t*>(value.data()), value.size());
    } else {
      gTransport->handleCommandWrite(reinterpret_cast<const uint8_t*>(value.data()), value.size());
    }
  }

  void onSubscribe(NimBLECharacteristic* characteristic, NimBLEConnInfo& connInfo, uint16_t subValue) override {
    if (gTransport == nullptr || characteristic == nullptr) {
      return;
    }
    if (!isAuthorizedSecureConnection(connInfo)) {
      Serial.println(F("BLE subscribe rejected: insecure connection"));
      if (gServer != nullptr) {
        gServer->disconnect(connInfo);
      }
      return;
    }
    if (strcmp(characteristic->getUUID().toString().c_str(), kStatusTxUuid) == 0) {
      gTransport->setNotifyEnabled(subValue != 0);
    }
  }

 private:
  bool isConfig_;
};

String deviceNameFromMac() {
  uint8_t mac[6] = {};
  esp_read_mac(mac, ESP_MAC_WIFI_STA);
  char name[16] = {};
  snprintf(name, sizeof(name), "DP-%02X%02X", mac[4], mac[5]);
  return String(name);
}

}  // namespace

void BleTransport::setRedrawHook(void (*hook)()) { gBleRedrawHook = hook; }

void BleTransport::setSecureWakeHook(void (*hook)()) { gSecureWakeHook = hook; }

BleTransport::BleTransport(PetStateMachine& state, BleSecurity& security)
    : state_(state), security_(security) {}

void BleTransport::begin(const char* deviceName) {
  gTransport = this;
  gSecurity = &security_;

  const String name = (deviceName != nullptr && deviceName[0] != '\0') ? String(deviceName)
                                                                       : deviceNameFromMac();

  NimBLEDevice::init(name.c_str());
  NimBLEDevice::setSecurityAuth(true, true, true);
  NimBLEDevice::setSecurityIOCap(BLE_HS_IO_DISPLAY_ONLY);

  NimBLEServer* server = NimBLEDevice::createServer();
  gServer = server;
  server->setCallbacks(new DeskPetServerCallbacks());

  NimBLEService* service = server->createService(kServiceUuid);

  NimBLECharacteristic* cmdRx = service->createCharacteristic(
      kCmdRxUuid,
      NIMBLE_PROPERTY::WRITE_ENC | NIMBLE_PROPERTY::WRITE_AUTHEN);
  cmdRx->setCallbacks(new DeskPetCharacteristicCallbacks(false));

  gStatusTx = service->createCharacteristic(
      kStatusTxUuid,
      NIMBLE_PROPERTY::READ_ENC | NIMBLE_PROPERTY::READ_AUTHEN | NIMBLE_PROPERTY::NOTIFY);
  gStatusTx->setCallbacks(new DeskPetCharacteristicCallbacks(false));

  NimBLECharacteristic* configRx = service->createCharacteristic(
      kConfigRxUuid,
      NIMBLE_PROPERTY::WRITE_ENC | NIMBLE_PROPERTY::WRITE_AUTHEN);
  configRx->setCallbacks(new DeskPetCharacteristicCallbacks(true));
  refreshStatusValue();

  NimBLEAdvertising* advertising = NimBLEDevice::getAdvertising();
  advertising->addServiceUUID(kServiceUuid);
  advertising->setName(name.c_str());
  advertising->setMinInterval(0x20);
  advertising->setMaxInterval(0x40);
  advertising->start();
}

void BleTransport::tick() {
  security_.tick();
  if (connected_ && !bonded_ && !security_.allowsNewBond() && gServer != nullptr) {
    Serial.println(F("BLE disconnected: pairing window expired"));
    gServer->disconnect(connectionHandle_);
  }
  refreshStatusValue();
}

void BleTransport::updateConnectionState(bool connected, uint16_t connectionHandle, bool bonded) {
  connected_ = connected;
  if (!connected) {
    secure_ = false;
    bonded_ = false;
    notifyEnabled_ = false;
    connectionHandle_ = 0;
  } else {
    connectionHandle_ = connectionHandle;
    bonded_ = bonded;
  }
  state_.bleConnected = connected_ && secure_;
  refreshStatusValue();
}

void BleTransport::updateSecurityState(bool secure) {
  const bool becameSecure = !secure_ && secure;
  secure_ = secure;
  state_.bleConnected = connected_ && secure_;
  if (becameSecure && connected_ && gSecureWakeHook != nullptr) {
    gSecureWakeHook();
  }
  refreshStatusValue();
}

void BleTransport::sendStatusJson(const char* json, size_t len) {
  if (gStatusTx == nullptr || !connected_ || !secure_ || !notifyEnabled_ || json == nullptr ||
      len == 0) {
    return;
  }
  gStatusTx->notify(reinterpret_cast<const uint8_t*>(json), len);
}

void BleTransport::refreshStatusValue() {
  if (gStatusTx == nullptr) {
    return;
  }
  char status[256] = {};
  const size_t len = JsonProtocol::buildStatusJson(state_, status, sizeof(status));
  if (len > 0) {
    gStatusTx->setValue(reinterpret_cast<const uint8_t*>(status), len);
  }
}

void BleTransport::sendAck(const char* id, bool ok, const char* errorMsg) {
  refreshStatusValue();
  char ack[192] = {};
  const size_t n = JsonProtocol::buildAckJson(id, ok, state_, errorMsg, ack, sizeof(ack));
  if (n > 0) {
    sendStatusJson(ack, n);
  }
}

void BleTransport::handleCommandWrite(const uint8_t* data, size_t len) {
  if (data == nullptr || len == 0) {
    return;
  }
  refreshStatusValue();
  const JsonParseResult parsed = JsonProtocol::parseCommand(reinterpret_cast<const char*>(data), len);
  if (!parsed.ok) {
    sendAck(parsed.id, false, parsed.error != nullptr ? parsed.error : "parse failed");
    return;
  }
  const TransitionResult tr = JsonProtocol::applyEvent(state_, parsed);
  if (tr == TransitionResult::INVALID) {
    sendAck(parsed.id, false, "invalid transition");
    return;
  }
  sendAck(parsed.id, true, nullptr);
  if (gBleRedrawHook != nullptr) {
    gBleRedrawHook();
  }
}

void BleTransport::handleConfigWrite(const uint8_t* data, size_t len) {
  handleCommandWrite(data, len);
}
