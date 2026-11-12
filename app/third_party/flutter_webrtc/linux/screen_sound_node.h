#ifndef FLUTTER_WEBRTC_SCREEN_SOUND_NODE_H_
#define FLUTTER_WEBRTC_SCREEN_SOUND_NODE_H_

// Fogata: which sound outputs a shared screen's sound may take, by a node's properties
// and whose it is. Header only, with neither PipeWire nor libwebrtc, so that its test
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

// What is known of whose an output is: the process and the executable its properties
// give (application.process.id, application.process.binary without the " (deleted)"
// the kernel adds), 0 or empty while not known, and the process its client connected
// from as the server saw it (pipewire.sec.pid; every stream of a PulseAudio program has
// the one pipewire-pulse client's).
struct OutputOwner {
  int pid = 0;
  int client_pid = 0;
  std::string binary;
};

// This application as its own outputs name it. `only_from`, for tests: the one program
// whose sound is taken. `sandboxed`: it runs in a Flatpak, whose processes are numbered
// in a namespace of each sandbox's own (the first program of every sandbox is 2): a
// process number another sandboxed program's output gives may then be this one's, and
// the server's number for a client is not in this one's namespace at all. Only the
// executable tells there.
struct ThisApp {
  int pid = 0;
  std::string binary;
  std::string only_from;
  bool sandboxed = false;
};

// Whether an output is this application's own (its call, its sounds, another copy of
// it), or, for tests, not the one program whose sound is taken: never linked.
inline bool OwnOutput(const OutputOwner& output, const ThisApp& app) {
  if (!app.binary.empty() && output.binary == app.binary) return true;
  // In a sandbox the numbers say nothing, unless nothing else can.
  if (!app.sandboxed || app.binary.empty()) {
    if (output.pid != 0 && output.pid == app.pid) return true;
    if (output.client_pid != 0 && output.client_pid == app.pid) return true;
  }
  // A program only once its executable is known: until then it may be the one.
  return !app.only_from.empty() && !output.binary.empty() &&
         output.binary != app.only_from;
}

// Whether enough is known of an output to link it: its process or its executable (the
// client's process alone says nothing of a PulseAudio program's stream), and with
// `only_from` its executable. Until then it may be this application's own.
inline bool OwnerKnown(const OutputOwner& output, const ThisApp& app) {
  if (!app.only_from.empty()) return !output.binary.empty();
  return output.pid != 0 || !output.binary.empty();
}

}  // namespace flutter_webrtc_plugin

#endif  // FLUTTER_WEBRTC_SCREEN_SOUND_NODE_H_
