#include "Face.h"

#include <Arduino.h>
#include <string.h>

namespace {
// ── Eye constants (shared by both eye views) ──────────────────
#define EYE_W   30
#define EYE_H   60
#define EYE_GAP 120
#define EYE_OX  0     // horizontal offset
#define EYE_OY  40    // vertical offset upward (subtracted from centre)
#define LOGO_CX 120
#define LOGO_CY 105

#define LOGO_TRI_COUNT 162
static const int16_t LOGO_TRIS[][6] PROGMEM = {
  {120,105,65,134,100,114},{120,105,100,114,101,113},{120,105,101,113,100,112},
  {120,105,100,112,99,112},{120,105,99,112,93,111},{120,105,93,111,73,111},
  {120,105,73,111,55,110},{120,105,55,110,38,109},{120,105,38,109,34,108},
  {120,105,34,108,30,103},{120,105,30,103,30,100},{120,105,30,100,34,98},
  {120,105,34,98,39,98},{120,105,39,98,50,99},{120,105,50,99,67,100},
  {120,105,67,100,80,101},{120,105,80,101,98,103},{120,105,98,103,101,103},
  {120,105,101,103,101,102},{120,105,101,102,100,101},{120,105,100,101,100,100},
  {120,105,100,100,82,88},{120,105,82,88,63,76},{120,105,63,76,53,69},
  {120,105,53,69,48,65},{120,105,48,65,45,61},{120,105,45,61,44,54},
  {120,105,44,54,49,49},{120,105,49,49,55,49},{120,105,55,49,57,49},
  {120,105,57,49,64,55},{120,105,64,55,78,66},{120,105,78,66,96,79},
  {120,105,96,79,99,81},{120,105,99,81,100,81},{120,105,100,81,100,80},
  {120,105,100,80,99,78},{120,105,99,78,89,60},{120,105,89,60,78,41},
  {120,105,78,41,73,34},{120,105,73,34,72,29},{120,105,72,29,72,28},
  {120,105,72,28,72,27},{120,105,72,27,71,26},{120,105,71,26,71,25},
  {120,105,71,25,71,24},{120,105,71,24,77,16},{120,105,77,16,80,15},
  {120,105,80,15,87,16},{120,105,87,16,91,19},{120,105,91,19,95,29},
  {120,105,95,29,103,46},{120,105,103,46,114,68},{120,105,114,68,118,75},
  {120,105,118,75,119,81},{120,105,119,81,120,83},{120,105,120,83,121,83},
  {120,105,121,83,121,82},{120,105,121,82,122,69},{120,105,122,69,124,54},
  {120,105,124,54,126,34},{120,105,126,34,126,28},{120,105,126,28,129,21},
  {120,105,129,21,135,18},{120,105,135,18,139,20},{120,105,139,20,143,25},
  {120,105,143,25,142,28},{120,105,142,28,140,42},{120,105,140,42,136,64},
  {120,105,136,64,133,78},{120,105,133,78,135,78},{120,105,135,78,136,76},
  {120,105,136,76,144,67},{120,105,144,67,156,51},{120,105,156,51,162,45},
  {120,105,162,45,168,38},{120,105,168,38,172,35},{120,105,172,35,180,35},
  {120,105,180,35,185,43},{120,105,185,43,183,52},{120,105,183,52,175,62},
  {120,105,175,62,168,71},{120,105,168,71,159,83},{120,105,159,83,153,94},
  {120,105,153,94,154,94},{120,105,154,94,155,94},{120,105,155,94,176,90},
  {120,105,176,90,188,88},{120,105,188,88,201,85},{120,105,201,85,208,88},
  {120,105,208,88,208,91},{120,105,208,91,206,97},{120,105,206,97,191,101},
  {120,105,191,101,174,104},{120,105,174,104,148,110},{120,105,148,110,148,111},
  {120,105,148,111,148,111},{120,105,148,111,160,112},{120,105,160,112,165,112},
  {120,105,165,112,177,112},{120,105,177,112,200,114},{120,105,200,114,205,118},
  {120,105,205,118,209,123},{120,105,209,123,208,126},{120,105,208,126,199,131},
  {120,105,199,131,187,128},{120,105,187,128,159,121},{120,105,159,121,149,119},
  {120,105,149,119,147,119},{120,105,147,119,147,120},{120,105,147,120,156,128},
  {120,105,156,128,170,141},{120,105,170,141,189,158},{120,105,189,158,190,163},
  {120,105,190,163,188,166},{120,105,188,166,185,166},{120,105,185,166,169,153},
  {120,105,169,153,162,148},{120,105,162,148,148,136},{120,105,148,136,147,136},
  {120,105,147,136,147,137},{120,105,147,137,150,142},{120,105,150,142,168,168},
  {120,105,168,168,169,176},{120,105,169,176,168,179},{120,105,168,179,163,180},
  {120,105,163,180,158,179},{120,105,158,179,148,165},{120,105,148,165,137,149},
  {120,105,137,149,129,134},{120,105,129,134,128,135},{120,105,128,135,123,189},
  {120,105,123,189,120,192},{120,105,120,192,115,194},{120,105,115,194,110,191},
  {120,105,110,191,108,185},{120,105,108,185,110,174},{120,105,110,174,113,160},
  {120,105,113,160,116,148},{120,105,116,148,118,134},{120,105,118,134,119,129},
  {120,105,119,129,119,129},{120,105,119,129,118,129},{120,105,118,129,107,144},
  {120,105,107,144,91,166},{120,105,91,166,78,180},{120,105,78,180,75,181},
  {120,105,75,181,70,178},{120,105,70,178,70,173},{120,105,70,173,73,169},
  {120,105,73,169,91,146},{120,105,91,146,102,132},{120,105,102,132,109,124},
  {120,105,109,124,109,123},{120,105,109,123,108,123},{120,105,108,123,61,153},
  {120,105,61,153,52,155},{120,105,52,155,49,151},{120,105,49,151,49,146},
  {120,105,49,146,51,144},{120,105,51,144,65,134},{120,105,65,134,65,134},
};

#define LOGO_SEG_COUNT 162
static const int16_t LOGO_SEGS[][4] PROGMEM = {
  {65,134,100,114},{100,114,101,113},{101,113,100,112},{100,112,99,112},
  {99,112,93,111},{93,111,73,111},{73,111,55,110},{55,110,38,109},
  {38,109,34,108},{34,108,30,103},{30,103,30,100},{30,100,34,98},
  {34,98,39,98},{39,98,50,99},{50,99,67,100},{67,100,80,101},
  {80,101,98,103},{98,103,101,103},{101,103,101,102},{101,102,100,101},
  {100,101,100,100},{100,100,82,88},{82,88,63,76},{63,76,53,69},
  {53,69,48,65},{48,65,45,61},{45,61,44,54},{44,54,49,49},
  {49,49,55,49},{55,49,57,49},{57,49,64,55},{64,55,78,66},
  {78,66,96,79},{96,79,99,81},{99,81,100,81},{100,81,100,80},
  {100,80,99,78},{99,78,89,60},{89,60,78,41},{78,41,73,34},
  {73,34,72,29},{72,29,72,28},{72,28,72,27},{72,27,71,26},
  {71,26,71,25},{71,25,71,24},{71,24,77,16},{77,16,80,15},
  {80,15,87,16},{87,16,91,19},{91,19,95,29},{95,29,103,46},
  {103,46,114,68},{114,68,118,75},{118,75,119,81},{119,81,120,83},
  {120,83,121,83},{121,83,121,82},{121,82,122,69},{122,69,124,54},
  {124,54,126,34},{126,34,126,28},{126,28,129,21},{129,21,135,18},
  {135,18,139,20},{139,20,143,25},{143,25,142,28},{142,28,140,42},
  {140,42,136,64},{136,64,133,78},{133,78,135,78},{135,78,136,76},
  {136,76,144,67},{144,67,156,51},{156,51,162,45},{162,45,168,38},
  {168,38,172,35},{172,35,180,35},{180,35,185,43},{185,43,183,52},
  {183,52,175,62},{175,62,168,71},{168,71,159,83},{159,83,153,94},
  {153,94,154,94},{154,94,155,94},{155,94,176,90},{176,90,188,88},
  {188,88,201,85},{201,85,208,88},{208,88,208,91},{208,91,206,97},
  {206,97,191,101},{191,101,174,104},{174,104,148,110},{148,110,148,111},
  {148,111,148,111},{148,111,160,112},{160,112,165,112},{165,112,177,112},
  {177,112,200,114},{200,114,205,118},{205,118,209,123},{209,123,208,126},
  {208,126,199,131},{199,131,187,128},{187,128,159,121},{159,121,149,119},
  {149,119,147,119},{147,119,147,120},{147,120,156,128},{156,128,170,141},
  {170,141,189,158},{189,158,190,163},{190,163,188,166},{188,166,185,166},
  {185,166,169,153},{169,153,162,148},{162,148,148,136},{148,136,147,136},
  {147,136,147,137},{147,137,150,142},{150,142,168,168},{168,168,169,176},
  {169,176,168,179},{168,179,163,180},{163,180,158,179},{158,179,148,165},
  {148,165,137,149},{137,149,129,134},{129,134,128,135},{128,135,123,189},
  {123,189,120,192},{120,192,115,194},{115,194,110,191},{110,191,108,185},
  {108,185,110,174},{110,174,113,160},{113,160,116,148},{116,148,118,134},
  {118,134,119,129},{119,129,119,129},{119,129,118,129},{118,129,107,144},
  {107,144,91,166},{91,166,78,180},{78,180,75,181},{75,181,70,178},
  {70,178,70,173},{70,173,73,169},{73,169,91,146},{91,146,102,132},
  {102,132,109,124},{109,124,109,123},{109,123,108,123},{108,123,61,153},
  {61,153,52,155},{52,155,49,151},{49,151,49,146},{49,146,51,144},
  {51,144,65,134},{65,134,65,134},
};
}  // namespace

Face::Face(Display& display) : display_(display) {}

int16_t Face::eyeLX(int16_t ox) const {
  return (DISP_W - (EYE_W * 2 + EYE_GAP)) / 2 + EYE_OX + ox;
}

int16_t Face::eyeRX(int16_t ox) const { return eyeLX(ox) + EYE_W + EYE_GAP; }
int16_t Face::eyeY() const { return (DISP_H - EYE_H) / 2 - EYE_OY; }
int16_t Face::eyeCY() const { return eyeY() + EYE_H / 2; }

uint16_t Face::expressionColor(AgentExpression expr) const {
  Adafruit_ST7789& tft = display_.tft();
  switch (expr) {
    case AgentExpression::CODING:
      return tft.color565(40, 90, 200);
    case AgentExpression::THINKING:
      return tft.color565(220, 180, 40);
    case AgentExpression::IDLE:
      return tft.color565(60, 170, 90);
    case AgentExpression::WAITING:
      return tft.color565(220, 120, 40);
    case AgentExpression::ERROR:
      return tft.color565(200, 40, 40);
    case AgentExpression::SECURITY:
      return tft.color565(140, 40, 160);
    case AgentExpression::CELEBRATE:
      return tft.color565(20, 128, 104);
  }
  return display_.colorOrange();
}

void Face::drawChevron(int16_t cx, int16_t cy, int16_t arm, int16_t reach, uint8_t thk,
                       bool rightFacing, uint16_t col) {
  Adafruit_ST7789& tft = display_.tft();
  for (int8_t t = -(int8_t)thk; t <= (int8_t)thk; t++) {
    if (rightFacing) {
      tft.drawLine(cx - reach / 2, cy - arm + t, cx + reach / 2, cy + t, col);
      tft.drawLine(cx + reach / 2, cy + t, cx - reach / 2, cy + arm + t, col);
    } else {
      tft.drawLine(cx + reach / 2, cy - arm + t, cx - reach / 2, cy + t, col);
      tft.drawLine(cx - reach / 2, cy + t, cx + reach / 2, cy + arm + t, col);
    }
  }
}

void Face::drawNormalEyes(int16_t ox, bool blink, bool clearBackground) {
  Adafruit_ST7789& tft = display_.tft();
  if (clearBackground) {
    tft.fillScreen(display_.animBgColor());
  }
  const int16_t lx = eyeLX(ox);
  const int16_t rx = eyeRX(ox);
  const int16_t ey = eyeY();
  if (!blink) {
    tft.fillRect(lx, ey, EYE_W, EYE_H, Display::kBlack);
    tft.fillRect(rx, ey, EYE_W, EYE_H, Display::kBlack);
  } else {
    tft.fillRect(lx, ey, EYE_W, EYE_H, display_.animBgColor());
    tft.fillRect(rx, ey, EYE_W, EYE_H, display_.animBgColor());
    tft.fillRect(lx, ey + EYE_H / 2 - 3, EYE_W, 6, Display::kBlack);
    tft.fillRect(rx, ey + EYE_H / 2 - 3, EYE_W, 6, Display::kBlack);
  }
}

void Face::drawWanderIdle(bool blinkOpen, int16_t lookOffset, bool fullRedraw) {
  if (!fullRedraw) {
    Adafruit_ST7789& tft = display_.tft();
    const int16_t y = eyeY();
    tft.fillRect(eyeLX(-6), y, EYE_W + 12, EYE_H, display_.animBgColor());
    tft.fillRect(eyeRX(-6), y, EYE_W + 12, EYE_H, display_.animBgColor());
  }
  drawNormalEyes(lookOffset, !blinkOpen, fullRedraw);
}

void Face::drawSquishEyes(bool closed) {
  Adafruit_ST7789& tft = display_.tft();
  tft.fillScreen(display_.animBgColor());
  const int16_t lx = eyeLX(0);
  const int16_t rx = eyeRX(0);
  const int16_t cy = eyeCY();
  const int16_t arm = EYE_H / 2;
  const int16_t reach = EYE_W / 2;
  const int16_t lcx = lx + EYE_W / 2;
  const int16_t rcx = rx + EYE_W / 2;
  if (!closed) {
    drawChevron(lcx, cy, arm, reach, 10, true, Display::kBlack);
    drawChevron(rcx, cy, arm, reach, 10, false, Display::kBlack);
  } else {
    tft.fillRect(lx, cy - 5, EYE_W, 10, Display::kBlack);
    tft.fillRect(rx, cy - 5, EYE_W, 10, Display::kBlack);
  }
}

void Face::drawErrorEyes() {
  Adafruit_ST7789& tft = display_.tft();
  tft.fillScreen(display_.animBgColor());
  const int16_t lx = eyeLX(0);
  const int16_t rx = eyeRX(0);
  const int16_t ey = eyeY();
  tft.drawLine(lx, ey, lx + EYE_W, ey + EYE_H, Display::kBlack);
  tft.drawLine(lx + EYE_W, ey, lx, ey + EYE_H, Display::kBlack);
  tft.drawLine(rx, ey, rx + EYE_W, ey + EYE_H, Display::kBlack);
  tft.drawLine(rx + EYE_W, ey, rx, ey + EYE_H, Display::kBlack);
}

void Face::drawWaitingEyes() {
  Adafruit_ST7789& tft = display_.tft();
  tft.fillScreen(display_.animBgColor());
  const int16_t lx = eyeLX(0);
  const int16_t rx = eyeRX(0);
  const int16_t ey = eyeY() - 8;
  const int16_t tall = EYE_H + 16;
  tft.fillRoundRect(lx, ey, EYE_W, tall, 8, Display::kBlack);
  tft.fillRoundRect(rx, ey, EYE_W, tall, 8, Display::kBlack);
}

void Face::drawLinkLost(bool blinkOpen, bool fullRedraw) {
  Adafruit_ST7789& tft = display_.tft();
  const uint16_t bg = tft.color565(32, 36, 48);
  display_.setAnimBgColor(bg);
  if (fullRedraw) {
    tft.fillScreen(bg);
  }
  drawNormalEyes(0, !blinkOpen, fullRedraw);
  tft.setTextColor(tft.color565(140, 148, 168));
  tft.setTextSize(2);
  tft.setCursor(DISP_W / 2 - 48, DISP_H - 36);
  tft.print("link lost");
}

void Face::drawSparkle(int16_t cx, int16_t cy, uint16_t col) {
  Adafruit_ST7789& tft = display_.tft();
  tft.drawLine(cx - 5, cy, cx + 5, cy, col);
  tft.drawLine(cx, cy - 5, cx, cy + 5, col);
  tft.drawPixel(cx - 3, cy - 3, col);
  tft.drawPixel(cx + 3, cy - 3, col);
  tft.drawPixel(cx - 3, cy + 3, col);
  tft.drawPixel(cx + 3, cy + 3, col);
}

void Face::drawWakeSmile() {
  Adafruit_ST7789& tft = display_.tft();
  for (int16_t x = -32; x <= 32; x++) {
    const int16_t y = static_cast<int16_t>((x * x) / 70);
    tft.drawPixel(LOGO_CX + x, 156 + y, Display::kBlack);
    tft.drawPixel(LOGO_CX + x, 157 + y, Display::kBlack);
  }
}

void Face::drawWorriedMouth() {
  Adafruit_ST7789& tft = display_.tft();
  for (int16_t x = -28; x <= 28; x++) {
    const int16_t y = static_cast<int16_t>((x * x) / 90);
    tft.drawPixel(LOGO_CX + x, 164 - y, Display::kBlack);
    tft.drawPixel(LOGO_CX + x, 165 - y, Display::kBlack);
  }
}

void Face::drawStartledMarks() {
  Adafruit_ST7789& tft = display_.tft();
  const int16_t y = eyeY() - 12;
  const int16_t lx = eyeLX(0);
  const int16_t rx = eyeRX(0);
  tft.drawLine(lx, y + 8, lx + EYE_W, y, Display::kBlack);
  tft.drawLine(lx, y + 9, lx + EYE_W, y + 1, Display::kBlack);
  tft.drawLine(rx, y, rx + EYE_W, y + 8, Display::kBlack);
  tft.drawLine(rx, y + 1, rx + EYE_W, y + 9, Display::kBlack);
}

void Face::drawCelebrateFace() {
  Adafruit_ST7789& tft = display_.tft();
  tft.fillScreen(display_.animBgColor());
  const int16_t lx = eyeLX(0);
  const int16_t rx = eyeRX(0);
  const int16_t cy = eyeCY();
  tft.fillRoundRect(lx, cy - 10, EYE_W, 18, 8, Display::kBlack);
  tft.fillRoundRect(rx, cy - 10, EYE_W, 18, 8, Display::kBlack);
  drawWakeSmile();
  const uint16_t spark = tft.color565(255, 220, 80);
  drawSparkle(32, 36, spark);
  drawSparkle(46, 168, spark);
  drawSparkle(188, 156, spark);
}

void Face::drawCaption(const char* text) {
  if (text == nullptr || text[0] == '\0') {
    return;
  }
  char line[17] = {};
  size_t n = 0;
  while (text[n] != '\0' && n < 16) {
    line[n] = text[n];
    n++;
  }
  line[n] = '\0';

  Adafruit_ST7789& tft = display_.tft();
  tft.fillRoundRect(12, 198, 216, 30, 8, tft.color565(16, 20, 28));
  tft.setTextColor(tft.color565(245, 245, 245));
  tft.setTextSize(2);
  const int16_t textW = static_cast<int16_t>(n * 12);
  int16_t x = static_cast<int16_t>((DISP_W - textW) / 2);
  if (x < 16) {
    x = 16;
  }
  tft.setCursor(x, 206);
  tft.print(line);
}

void Face::drawSceneOverlay(AgentExpression expr, const char* caption, const char* mood) {
  const bool skipMood = expr == AgentExpression::CELEBRATE || expr == AgentExpression::ERROR ||
                        expr == AgentExpression::SECURITY;
  if (!skipMood) {
    const bool happy = (caption != nullptr && strcmp(caption, "waking up") == 0) ||
                       (mood != nullptr && strcmp(mood, "happy") == 0);
    if (happy) {
      drawWakeSmile();
    } else if (mood != nullptr && strcmp(mood, "worried") == 0) {
      drawWorriedMouth();
    } else if (mood != nullptr && strcmp(mood, "startled") == 0) {
      drawStartledMarks();
    }
  }
  drawCaption(caption);
}

void Face::drawSecurityEyes() {
  Adafruit_ST7789& tft = display_.tft();
  tft.fillScreen(display_.animBgColor());
  drawSquishEyes(false);
  tft.setTextColor(Display::kBlack);
  tft.setTextSize(2);
  tft.setCursor(DISP_W / 2 - 18, DISP_H - 40);
  tft.print("LOCK");
}

void Face::drawLogoFilled(uint16_t bg, uint16_t fg) {
  Adafruit_ST7789& tft = display_.tft();
  tft.fillScreen(bg);
  for (uint16_t i = 0; i < LOGO_TRI_COUNT; i++) {
    tft.fillTriangle(pgm_read_word(&LOGO_TRIS[i][0]), pgm_read_word(&LOGO_TRIS[i][1]),
                     pgm_read_word(&LOGO_TRIS[i][2]), pgm_read_word(&LOGO_TRIS[i][3]),
                     pgm_read_word(&LOGO_TRIS[i][4]), pgm_read_word(&LOGO_TRIS[i][5]), fg);
  }
  tft.setTextColor(fg);
  tft.setTextSize(2);
  tft.setCursor(LOGO_CX - 54, 210);
  tft.print("Anthropic");
  tft.setCursor(LOGO_CX - 53, 210);
  tft.print("Anthropic");
}

void Face::showBootSplash() {
  Adafruit_ST7789& tft = display_.tft();
  tft.fillScreen(display_.animBgColor());
  tft.setTextColor(Display::kWhite);
  tft.setTextSize(3);
  tft.setCursor(DISP_W / 2 - 54, DISP_H / 2 - 22);
  tft.print("Clawd");
  tft.setCursor(DISP_W / 2 - 54, DISP_H / 2 + 14);
  tft.print("Mochi");
}

void Face::beginLogoReveal() {
  logoSegmentIndex_ = 0;
  logoRevealDone_ = false;
  logoFilled_ = false;
  Adafruit_ST7789& tft = display_.tft();
  tft.fillScreen(display_.animBgColor());
}

bool Face::tickLogoReveal() {
  if (logoRevealDone_) {
    return true;
  }

  Adafruit_ST7789& tft = display_.tft();
  uint16_t drawn = 0;
  while (logoSegmentIndex_ < LOGO_SEG_COUNT && drawn < 4) {
    const int16_t x1 = pgm_read_word(&LOGO_SEGS[logoSegmentIndex_][0]);
    const int16_t y1 = pgm_read_word(&LOGO_SEGS[logoSegmentIndex_][1]);
    const int16_t x2 = pgm_read_word(&LOGO_SEGS[logoSegmentIndex_][2]);
    const int16_t y2 = pgm_read_word(&LOGO_SEGS[logoSegmentIndex_][3]);
    tft.drawLine(x1, y1, x2, y2, Display::kWhite);
    tft.drawLine(x1 + 1, y1, x2 + 1, y2, Display::kWhite);
    logoSegmentIndex_++;
    drawn++;
  }

  if (logoSegmentIndex_ >= LOGO_SEG_COUNT && !logoFilled_) {
    drawLogoFilled(display_.animBgColor(), Display::kWhite);
    logoFilled_ = true;
    logoRevealDone_ = true;
    return true;
  }
  return false;
}

void Face::drawExpression(AgentExpression expr) {
  display_.setAnimBgColor(expressionColor(expr));
  switch (expr) {
    case AgentExpression::CODING:
      drawNormalEyes(0, false, true);
      break;
    case AgentExpression::THINKING:
      drawSquishEyes(false);
      break;
    case AgentExpression::IDLE:
      drawNormalEyes(0, false, true);
      break;
    case AgentExpression::WAITING:
      drawWaitingEyes();
      break;
    case AgentExpression::ERROR:
      drawErrorEyes();
      break;
    case AgentExpression::SECURITY:
      drawSecurityEyes();
      break;
    case AgentExpression::CELEBRATE:
      drawCelebrateFace();
      break;
  }
#if defined(DESKPET_DEBUG)
  printAsciiPreview(expr);
#endif
}

void Face::drawCurrent() { drawExpression(AgentExpression::IDLE); }

#if defined(DESKPET_DEBUG)
void Face::printAsciiPreview(AgentExpression expr) const {
  Serial.print(F("[face] "));
  Serial.println(PetStateMachine::expressionName(expr));
}
#endif
