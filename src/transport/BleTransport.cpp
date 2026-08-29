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
NimBLECharacteristic* gStatusTx = nullptr;
void (*gBleRedrawHook)() = nullptr;

class DeskPetServerCallbacks : public NimBLEServerCallbacks {
  void onConnect(NimBLEServer* server, NimBLEConnInfo& connInfo) override {
    if (gTransport != nullptr) {
      gTransport->updateConnectionState(true);
    }
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
    const uint32_t passkey = esp_random() % 1000000UL;
    if (gSecurity != nullptr) {
      gSecurity->onPasskeyDisplay(passkey);
    }
    return passkey;
  }

  void onAuthenticationComplete(NimBLEConnInfo& connInfo) override {
    (void)connInfo;
    Serial.println(F("BLE auth complete"));
  }
};

class DeskPetCharacteristicCallbacks : public NimBLECharacteristicCallbacks {
 public:
  explicit DeskPetCharacteristicCallbacks(bool isConfig) : isConfig_(isConfig) {}

  void onWrite(NimBLECharacteristic* characteristic, NimBLEConnInfo& connInfo) override {
    (void)connInfo;
    if (gTransport == nullptr || characteristic == nullptr) {
      return;
    }
    const std::string& value = characteristic->getValue();
    if (isConfig_) {
      gTransport->handleConfigWrite(reinterpret_cast<const uint8_t*>(value.data()), value.size());
    } else {
      gTransport->handleCommandWrite(reinterpret_cast<const uint8_t*>(value.data()), value.size());
    }
  }

  void onSubscribe(NimBLECharacteristic* characteristic, NimBLEConnInfo& connInfo, uint16_t subValue) override {
    (void)connInfo;
    if (gTransport == nullptr || characteristic == nullptr) {
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
  server->setCallbacks(new DeskPetServerCallbacks());

  NimBLEService* service = server->createService(kServiceUuid);

  NimBLECharacteristic* cmdRx = service->createCharacteristic(
      kCmdRxUuid,
      NIMBLE_PROPERTY::WRITE | NIMBLE_PROPERTY::WRITE_ENC | NIMBLE_PROPERTY::WRITE_AUTHEN);
  cmdRx->setCallbacks(new DeskPetCharacteristicCallbacks(false));

  gStatusTx = service->createCharacteristic(
      kStatusTxUuid,
      NIMBLE_PROPERTY::READ | NIMBLE_PROPERTY::READ_ENC | NIMBLE_PROPERTY::READ_AUTHEN |
          NIMBLE_PROPERTY::NOTIFY);
  gStatusTx->setCallbacks(new DeskPetCharacteristicCallbacks(false));

  NimBLECharacteristic* configRx = service->createCharacteristic(
      kConfigRxUuid,
      NIMBLE_PROPERTY::WRITE | NIMBLE_PROPERTY::WRITE_ENC | NIMBLE_PROPERTY::WRITE_AUTHEN);
  configRx->setCallbacks(new DeskPetCharacteristicCallbacks(true));

  NimBLEAdvertising* advertising = NimBLEDevice::getAdvertising();
  advertising->addServiceUUID(kServiceUuid);
  advertising->setName(name.c_str());
  advertising->setMinInterval(0x20);
  advertising->setMaxInterval(0x40);
  advertising->start();
}

void BleTransport::tick() { security_.tick(); }

void BleTransport::updateConnectionState(bool connected) {
  connected_ = connected;
  state_.bleConnected = connected;
  if (!connected) {
    notifyEnabled_ = false;
  }
}

void BleTransport::sendStatusJson(const char* json, size_t len) {
  if (gStatusTx == nullptr || !connected_ || !notifyEnabled_ || json == nullptr || len == 0) {
    return;
  }
  gStatusTx->setValue(reinterpret_cast<const uint8_t*>(json), len);
  gStatusTx->notify();
}

void BleTransport::sendAck(const char* id, bool ok, const char* errorMsg) {
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
