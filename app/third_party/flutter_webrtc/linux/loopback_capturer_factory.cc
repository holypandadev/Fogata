#include "loopback_capturer.h"

// Linux factory for LoopbackCapturer.
//
// Fogata: built with PipeWire (HAVE_PIPEWIRE, set in CMakeLists.txt), the sound of
// every other application, without this one's own (its calls), instead of
// everything the default output plays. Without PipeWire, or where it cannot
// connect (PulseAudio itself is the sound server, a Flatpak without PipeWire's
// socket), a shared screen goes without sound: PulseAudio's monitor of the output
// would carry this application's own call sound, and the others in the call would
// hear themselves back. getDisplayMedia then gives no audio track.
//
// The declaration in loopback_capturer.h is non-inline for __linux__, so this
// translation unit must always define the symbol.

#if defined(__linux__) && defined(HAVE_PIPEWIRE)
#include "pipewire_apps_capturer.h"
#endif

namespace flutter_webrtc_plugin {

#if defined(__linux__) && defined(HAVE_PIPEWIRE)

// None for a shared window either: the sound cannot be narrowed to the window's
// own application here (Windows and macOS do), and every other one's would go to
// the whole room with it.
std::unique_ptr<LoopbackCapturer> CreateLoopbackCapturer(
    const std::string& /*source_id*/,
    bool window) {
  if (window) return nullptr;
  return std::unique_ptr<LoopbackCapturer>(new PipeWireAppsCapturer());
}

#else  // No PipeWire: no sound with a shared screen.

std::unique_ptr<LoopbackCapturer> CreateLoopbackCapturer(
    const std::string& /*source_id*/,
    bool /*window*/) {
  return nullptr;
}

#endif

}  // namespace flutter_webrtc_plugin
