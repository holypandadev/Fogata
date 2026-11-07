#ifndef FLUTTER_WEBRTC_INPUT_NAME_H_
#define FLUTTER_WEBRTC_INPUT_NAME_H_

#include <string>

namespace flutter_webrtc_plugin {

// The sound server's own name (PulseAudio, or PipeWire's PulseAudio service) of the input
// WebRTC lists as `input`, which is the input's description there: what the application
// opens itself to record a voice message, through ALSA's plugin for the sound server.
// Empty when no input has that description or name, when the server does not answer
// within a second, or where libpulse is missing.
std::string InputName(const std::string& input);

}  // namespace flutter_webrtc_plugin

#endif  // FLUTTER_WEBRTC_INPUT_NAME_H_
