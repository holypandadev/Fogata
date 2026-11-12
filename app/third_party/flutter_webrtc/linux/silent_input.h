#ifndef FLUTTER_WEBRTC_SILENT_INPUT_H_
#define FLUTTER_WEBRTC_SILENT_INPUT_H_

#include <functional>

namespace flutter_webrtc_plugin {

// A sound input that only gives silence, named "Fogata silence (<pid>)" after
// this process, so each Fogata running has its own. WebRTC records
// from its input whenever any channel of sound sends, even one carrying other
// sound; pointed here, it records nothing and the microphone stays closed.
class SilentInput {
 public:
  // Creates the input if it is not there yet; false where PipeWire is missing.
  static bool Start();
  // Removes it.
  static void Stop();
  // Creates it (on) or removes it on a thread of its own, once every one asked before
  // is done and answered, so a removal never overtakes the creation asked before it
  // and a server slow to answer (up to 2 s) holds up nothing else; then tells `answer`,
  // on that thread, whether it was made (true for a removal). Asked from one thread
  // only (the platform thread).
  static void Ask(bool on, std::function<void(bool made)> answer);
};

}  // namespace flutter_webrtc_plugin

#endif  // FLUTTER_WEBRTC_SILENT_INPUT_H_
