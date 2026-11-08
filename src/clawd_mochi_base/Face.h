#pragma once

#include <stdint.h>

#include "../clawd_mochi_base/States.h"
#include "Display.h"

class Face {
 public:
  explicit Face(Display& display);

  void showBootSplash();
  void beginLogoReveal();
  bool tickLogoReveal();
  void drawExpression(AgentExpression expr);
  void drawCaption(const char* text);
  void drawSceneOverlay(AgentExpression expr, const char* caption, const char* mood);
  void drawWanderIdle(bool blinkOpen, int16_t lookOffset, bool fullRedraw);
  void drawLinkLost(bool blinkOpen, bool fullRedraw);
  void drawCurrent();
  uint16_t expressionColor(AgentExpression expr) const;

#if defined(DESKPET_DEBUG)
  void printAsciiPreview(AgentExpression expr) const;
#endif

 private:
  Display& display_;

  int16_t eyeLX(int16_t ox) const;
  int16_t eyeRX(int16_t ox) const;
  int16_t eyeY() const;
  int16_t eyeCY() const;

  void drawChevron(int16_t cx, int16_t cy, int16_t arm, int16_t reach, uint8_t thk,
                   bool rightFacing, uint16_t col);
  void drawNormalEyes(int16_t ox, bool blink, bool clearBackground = true);
  void drawSquishEyes(bool closed);
  void drawLogoFilled(uint16_t bg, uint16_t fg);
  void drawErrorEyes();
  void drawWaitingEyes();
  void drawSecurityEyes();
  void drawCelebrateFace();
  void drawWakeSmile();
  void drawWorriedMouth();
  void drawStartledMarks();
  void drawSparkle(int16_t cx, int16_t cy, uint16_t col);

  uint16_t logoSegmentIndex_ = 0;
  bool logoRevealDone_ = true;
  bool logoFilled_ = false;
};
