#ifndef FLUTTER_WEBRTC_SCREEN_SOUND_NODE_H_
#define FLUTTER_WEBRTC_SCREEN_SOUND_NODE_H_

// Fogata: which sound outputs a shared screen's sound may take, by a node's properties
// alone. Header only, with neither PipeWire nor libwebrtc, so that its test
// (common/cpp/test/screen_sound_node_test.cc) builds anywhere.

#include <cstring>
#include <string>

namespace flutter_webrtc_plugin {

// The programs of the sound server itself and of effect hosts, whose outputs carry
// what other programs played: a loopback, a combined output, an equalizer playing on
// to the speakers. One of them may carry this application's own call.
inline bool SoundServerBinary(const std::string& binary) {
  static const char* const kServers[] = {
      "pipewire", "pipewire-pulse", "wireplumber", "pw-loopback",
      "easyeffects", "pulseeffects", "pulseaudio",
  };
  for (const char* server : kServers) {
    if (binary == server) return true;
  }
  return false;
}

// Why an output (a node of class Stream/Output/Audio) is not a program's own sound but
// sound passed on from elsewhere, or null when nothing says so. `lookup` gives a
// property's value, or null without it.
//
// - node.link-group: the playing side of a pair that passes sound on (PipeWire's
//   loopback and filter-chain, echo-cancel, combine-stream, and PulseAudio's
//   module-loopback and module-combine-sink as pipewire-pulse loads them);
// - node.group: the same pairs, scheduled together (pipewire-jack's own group, the
//   only other user, makes no stream of this class);
// - stream.is-loopback, node.virtual = true: what a loopback or a virtual device says
//   of itself;
// - media.category or media.role "Filter"/"filter": an effect host's node;
// - application.process.binary: the sound server's, or an effect host's.
//
// A program's own stream (a browser, a game through Wine or Proton, a media player, a
// Flatpak's) says none of these: through pipewire-pulse its binary and process are
// the program's (libpulse fills them in), and it has no group.
template <typename Lookup>
const char* PassedOnBecause(Lookup lookup, const std::string& binary) {
  auto is = [](const char* value, const char* wanted) {
    return value != nullptr && std::strcmp(value, wanted) == 0;
  };
  auto present = [](const char* value) { return value != nullptr && *value != 0; };
  if (present(lookup("node.link-group"))) return "node.link-group";
  if (present(lookup("node.group"))) return "node.group";
  if (is(lookup("stream.is-loopback"), "true")) return "stream.is-loopback";
  if (is(lookup("node.virtual"), "true")) return "node.virtual";
  const char* category = lookup("media.category");
  if (is(category, "Filter") || is(category, "filter")) return "media.category";
  const char* role = lookup("media.role");
  if (is(role, "Filter") || is(role, "filter")) return "media.role";
  if (SoundServerBinary(binary)) return "sound server's binary";
  return nullptr;
}

}  // namespace flutter_webrtc_plugin

#endif  // FLUTTER_WEBRTC_SCREEN_SOUND_NODE_H_
