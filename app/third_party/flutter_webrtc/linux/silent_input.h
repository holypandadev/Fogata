#ifndef FLUTTER_WEBRTC_SILENT_INPUT_H_
#define FLUTTER_WEBRTC_SILENT_INPUT_H_

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
};

}  // namespace flutter_webrtc_plugin

#endif  // FLUTTER_WEBRTC_SILENT_INPUT_H_
