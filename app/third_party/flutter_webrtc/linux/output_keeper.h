#ifndef FLUTTER_WEBRTC_OUTPUT_KEEPER_H_
#define FLUTTER_WEBRTC_OUTPUT_KEEPER_H_

#include <string>

namespace flutter_webrtc_plugin {

// Keeps every sound this process plays (its short sounds through ALSA, its videos through
// GStreamer, besides WebRTC's own) on the output chosen in the application, as the
// sound server lists outputs (PulseAudio, or PipeWire's PulseAudio service): each of
// this process's streams is moved there as it appears. WebRTC names an output by its
// description there, and so does the application.
class OutputKeeper {
 public:
  // From now on the chosen output is the one whose description (or name) is `output`;
  // empty leaves new sounds on the system's default. Nothing where libpulse is missing.
  static void SendTo(const std::string& output);
};

}  // namespace flutter_webrtc_plugin

#endif  // FLUTTER_WEBRTC_OUTPUT_KEEPER_H_
