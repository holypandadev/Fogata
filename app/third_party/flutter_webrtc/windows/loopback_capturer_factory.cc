#ifdef _WIN32

#include "application_loopback_capturer.h"

namespace flutter_webrtc_plugin {

// ---------------------------------------------------------------------------
// CreateLoopbackCapturer — Windows factory implementation
// Creates an ApplicationLoopbackCapturer. For a window (its id is its HWND as a
// decimal string) it captures only that window's process and its children.
// Fogata: a window whose process cannot be found (the window closed meanwhile,
// an id that is no window) gets no capturer at all, never all-system audio; a
// screen gets every application's sound but this one's.
// ---------------------------------------------------------------------------
std::unique_ptr<LoopbackCapturer> CreateLoopbackCapturer(
    const std::string& source_id,
    bool window) {
  if (!window) {
    return std::unique_ptr<LoopbackCapturer>(new ApplicationLoopbackCapturer());
  }
  DWORD pid = 0;
  try {
    HWND hwnd = reinterpret_cast<HWND>(
        static_cast<uintptr_t>(std::stoull(source_id)));
    if (hwnd && IsWindow(hwnd)) {
      GetWindowThreadProcessId(hwnd, &pid);
    }
  } catch (...) {
    pid = 0;
  }
  if (pid == 0) {
    return nullptr;
  }
  auto* cap = new ApplicationLoopbackCapturer();
  cap->SetTargetProcessId(pid);
  return std::unique_ptr<LoopbackCapturer>(cap);
}

}  // namespace flutter_webrtc_plugin

#endif  // _WIN32
