#pragma once

#include <stddef.h>

#include "../clawd_mochi_base/Face.h"
#include "../clawd_mochi_base/States.h"

class BleSecurity;
class BleTransport;

class SerialDiag {
 public:
  SerialDiag(PetStateMachine& state, Face& face, Display& display, BleSecurity* bleSecurity = nullptr,
             BleTransport* bleTransport = nullptr);

  void begin();
  void printBootBanner(const char* deviceName);
  void poll();
  bool needsRedraw() const { return needsRedraw_; }
  void clearRedraw() { needsRedraw_ = false; }

 private:
  PetStateMachine& state_;
  Face& face_;
  Display& display_;
  BleSecurity* bleSecurity_;
  BleTransport* bleTransport_;
  String lineBuffer_;
  bool needsRedraw_ = false;

  void handleLine(const String& line);
  void replyOk(const char* msg = nullptr);
  void replyErr(const char* msg);
  void printHelp() const;
  static String trimUpper(const String& s);
  static bool isSensitiveToken(const char* token);
};
