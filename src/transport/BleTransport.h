#pragma once

#include <stddef.h>
#include <stdint.h>

class BleSecurity;
class PetStateMachine;

class BleTransport {
 public:
  BleTransport(PetStateMachine& state, BleSecurity& security);

  void begin(const char* deviceName);
  void tick();

  bool isConnected() const { return connected_; }
  void sendStatusJson(const char* json, size_t len);
  void refreshStatusValue();
  void sendAck(const char* id, bool ok, const char* errorMsg);

  static void setRedrawHook(void (*hook)());
  static void setSecureWakeHook(void (*hook)());

  void updateConnectionState(bool connected, uint16_t connectionHandle = 0, bool bonded = false);
  void updateSecurityState(bool secure);
  void updateBondState(bool bonded) { bonded_ = bonded; }
  void setNotifyEnabled(bool enabled) { notifyEnabled_ = enabled; }
  void handleCommandWrite(const uint8_t* data, size_t len);
  void handleConfigWrite(const uint8_t* data, size_t len);

 private:
  PetStateMachine& state_;
  BleSecurity& security_;
  bool connected_ = false;
  bool secure_ = false;
  bool bonded_ = false;
  bool notifyEnabled_ = false;
  uint16_t connectionHandle_ = 0;
};
