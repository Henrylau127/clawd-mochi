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
  void sendAck(const char* id, bool ok, const char* errorMsg);

  static void setRedrawHook(void (*hook)());

  void updateConnectionState(bool connected);
  void setNotifyEnabled(bool enabled) { notifyEnabled_ = enabled; }
  void handleCommandWrite(const uint8_t* data, size_t len);
  void handleConfigWrite(const uint8_t* data, size_t len);

 private:
  PetStateMachine& state_;
  BleSecurity& security_;
  bool connected_ = false;
  bool notifyEnabled_ = false;
};
