#pragma once

#include <stdint.h>

class PairingWindow {
 public:
  static constexpr uint32_t kDurationMs = 120000;

  void open(uint32_t now, uint32_t passkey);
  void close();

  bool isOpen(uint32_t now) const;
  bool allowsNewBond(uint32_t now) const { return isOpen(now); }
  bool hasPasskey() const { return open_; }
  uint32_t passkey() const { return passkey_; }
  uint32_t remainingMs(uint32_t now) const;

 private:
  bool open_ = false;
  uint32_t openedAtMs_ = 0;
  uint32_t passkey_ = 0;
};
