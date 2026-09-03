#pragma once

#include <stddef.h>
#include <stdint.h>

class Display;
class Face;
class PetStateMachine;

class SecurityEvents {
 public:
  static constexpr uint32_t kMinOverlayMs = 10000;
  static constexpr uint32_t kMaxOverlayMs = 30000;

  SecurityEvents(PetStateMachine& state, Face& face, Display& display);

  void trigger(const char* typeLabel);
  bool tick(uint32_t nowMs);
  bool isActive() const;
  bool needsRedraw() const { return needsRedraw_; }
  void clearRedraw() { needsRedraw_ = false; }
  void redraw();

 private:
  PetStateMachine& state_;
  Face& face_;
  Display& display_;
  uint32_t overlayEndMs_ = 0;
  bool needsRedraw_ = false;

  static const char* sanitizeTypeLabel(const char* type);
  void drawOverlay();
};
