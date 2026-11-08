#pragma once

#include <ctype.h>
#include <stddef.h>
#include <string.h>

// Short on-screen labels only. Reject paths, shell words, and secret-shaped tokens.
inline bool sanitizeCaption(const char* in, char* out, size_t outLen) {
  if (out == nullptr || outLen < 2) {
    return false;
  }
  out[0] = '\0';
  if (in == nullptr) {
    return true;
  }

  char cleaned[17] = {};
  size_t n = 0;
  bool prevSpace = true;
  for (size_t i = 0; in[i] != '\0'; i++) {
    if (i > 48) {
      return false;
    }
    const unsigned char ch = static_cast<unsigned char>(in[i]);
    char c = 0;
    if ((ch >= 'A' && ch <= 'Z') || (ch >= 'a' && ch <= 'z') || (ch >= '0' && ch <= '9') ||
        ch == '.' || ch == ',' || ch == '!' || ch == '?' || ch == '\'' || ch == '-') {
      c = static_cast<char>(ch);
    } else if (ch == ' ' || ch == '\t') {
      if (prevSpace) {
        continue;
      }
      c = ' ';
    } else {
      return false;
    }
    if (n + 1 >= sizeof(cleaned)) {
      return false;
    }
    cleaned[n++] = c;
    prevSpace = (c == ' ');
  }
  while (n > 0 && cleaned[n - 1] == ' ') {
    cleaned[--n] = '\0';
  }
  cleaned[n] = '\0';
  if (n == 0) {
    return true;
  }

  char lower[17] = {};
  bool hasLetter = false;
  for (size_t i = 0; i < n; i++) {
    lower[i] = static_cast<char>(tolower(static_cast<unsigned char>(cleaned[i])));
    if (lower[i] >= 'a' && lower[i] <= 'z') {
      hasLetter = true;
    }
  }
  lower[n] = '\0';
  if (!hasLetter) {
    return false;
  }

  static const char* kFragments[] = {"password", "token", "secret", "passkey", "bearer",
                                     "http://",  "https://"};
  for (const char* fragment : kFragments) {
    if (strstr(lower, fragment) != nullptr) {
      return false;
    }
  }

  size_t words = 0;
  size_t wordStart = 0;
  for (size_t i = 0; i <= n; i++) {
    if (i < n && lower[i] != ' ') {
      continue;
    }
    const size_t wordLen = i - wordStart;
    if (wordLen == 0) {
      wordStart = i + 1;
      continue;
    }
    words++;
    if (words > 4 || wordLen > 12) {
      return false;
    }
    char word[13] = {};
    memcpy(word, lower + wordStart, wordLen);
    word[wordLen] = '\0';
    static const char* kCommands[] = {"sudo",  "git",     "npm",  "pnpm", "yarn", "cargo",
                                      "ssh",   "curl",    "wget", "bash", "zsh",  "python",
                                      "python3", "pytest", "pio", "cat",  "rm",   "cd",
                                      "ls",    "echo",    "printf", "go"};
    for (const char* command : kCommands) {
      if (strcmp(word, command) == 0) {
        return false;
      }
    }
    if (wordLen >= 8) {
      bool hex = true;
      for (size_t c = 0; c < wordLen; c++) {
        if (!isxdigit(static_cast<unsigned char>(word[c]))) {
          hex = false;
          break;
        }
      }
      if (hex) {
        return false;
      }
    }
    wordStart = i + 1;
  }

  if (outLen <= n) {
    return false;
  }
  memcpy(out, cleaned, n + 1);
  return true;
}

inline bool sanitizeMood(const char* in, char* out, size_t outLen) {
  if (out == nullptr || outLen < 2) {
    return false;
  }
  out[0] = '\0';
  if (in == nullptr || in[0] == '\0') {
    return true;
  }
  static const char* kMoods[] = {"happy", "worried", "startled"};
  for (const char* mood : kMoods) {
    if (strcmp(in, mood) == 0) {
      if (outLen <= strlen(mood)) {
        return false;
      }
      memcpy(out, mood, strlen(mood) + 1);
      return true;
    }
  }
  return false;
}
