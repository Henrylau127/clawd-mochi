#include "PairingWindow.h"

void PairingWindow::open(uint32_t now, uint32_t passkey) {
  open_ = true;
  openedAtMs_ = now;
  passkey_ = passkey % 1000000UL;
}

void PairingWindow::close() {
  open_ = false;
  passkey_ = 0;
}

bool PairingWindow::isOpen(uint32_t now) const {
  return open_ && static_cast<uint32_t>(now - openedAtMs_) < kDurationMs;
}

uint32_t PairingWindow::remainingMs(uint32_t now) const {
  if (!isOpen(now)) {
    return 0;
  }
  return kDurationMs - static_cast<uint32_t>(now - openedAtMs_);
}
