#ifndef FLUTTER_WEBRTC_INPUT_WATCHER_H_
#define FLUTTER_WEBRTC_INPUT_WATCHER_H_

#include <functional>

namespace flutter_webrtc_plugin {

// Tells when a sound input is added to the system or removed from it, as the
// sound server lists them (PulseAudio, or PipeWire's PulseAudio service).
// WebRTC selects its input by place in that list, so after such a change the
// chosen input must be selected again at its new place.
class InputWatcher {
 public:
  // Calls `changed` on the watcher's own thread after each change; nothing
  // where libpulse is missing.
  static void Start(std::function<void()> changed);
  // Stops the watcher's thread and waits for it, which may be running
  // `changed`: never call it from a task `changed` queued, nor while holding
  // anything such a task waits for.
  static void Stop();

  // What a check of the sound server found: it answered (kReady), there is
  // definitely none (kNone: the connection was refused or failed), it took the
  // connection but did not answer in time (kTimeout), or it could not tell
  // (kUnknown: libpulse missing, or no memory). WebRTC's voice engine ends the
  // process when it reaches no server, so a call must not start on kNone; a
  // slow server (a fresh session, a busy computer, a remote one) must not be
  // taken for none at once.
  enum class Server { kReady, kNone, kTimeout, kUnknown };

  // Checks without starting a server, waiting at most `seconds` for an answer.
  // Blocks the calling thread: never call it on the platform thread.
  static Server CheckServer(int seconds);
};

}  // namespace flutter_webrtc_plugin

#endif  // FLUTTER_WEBRTC_INPUT_WATCHER_H_
