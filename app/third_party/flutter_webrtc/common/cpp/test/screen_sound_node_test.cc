// Which outputs a shared screen's sound takes (linux/screen_sound_node.h): a program's
// own stream, as browsers, games through Wine or Proton, media players and Flatpak apps
// make them, and never one that passes on sound from elsewhere (a loopback, a combined
// output, an effect host), which may carry this application's own call. The properties
// are those PipeWire 1.x and pipewire-pulse give such nodes. Plain C++, no PipeWire.

#include <cstdio>
#include <map>
#include <string>

#include "screen_sound_node.h"

using flutter_webrtc_plugin::PassedOnBecause;

namespace {

int failures = 0;

using Props = std::map<std::string, std::string>;

const char* Why(const Props& props) {
  auto binary = props.find("application.process.binary");
  return PassedOnBecause(
      [&](const char* key) -> const char* {
        auto it = props.find(key);
        return it == props.end() ? nullptr : it->second.c_str();
      },
      binary == props.end() ? std::string() : binary->second);
}

void Taken(const std::string& what, const Props& props) {
  if (const char* why = Why(props)) {
    std::printf("FAIL: %s left out (%s)\n", what.c_str(), why);
    failures++;
  }
}

void LeftOut(const std::string& what, const Props& props) {
  if (Why(props) == nullptr) {
    std::printf("FAIL: %s taken\n", what.c_str());
    failures++;
  }
}

void Expect(bool got, bool wanted, const std::string& what) {
  if (got != wanted) {
    std::printf("FAIL: %s: %s\n", what.c_str(), got ? "yes" : "no");
    failures++;
  }
}

// Whose an output is (OwnOutput, OwnerKnown): this application's own is never taken,
// however it tells; nothing is taken before it tells anything.
void Owners() {
  using flutter_webrtc_plugin::OutputOwner;
  using flutter_webrtc_plugin::OwnerKnown;
  using flutter_webrtc_plugin::OwnOutput;
  using flutter_webrtc_plugin::ThisApp;
  const ThisApp app{4242, "fogata", "", false};
  Expect(OwnOutput({4242, 0, "fogata"}, app), true, "the call's own sound");
  Expect(OwnOutput({4242, 0, ""}, app), true, "own by its process alone");
  Expect(OwnOutput({0, 4242, ""}, app), true, "own by its client's process");
  Expect(OwnOutput({0, 0, "fogata"}, app), true, "another copy of the app");
  Expect(OwnOutput({5151, 900, "firefox"}, app), false, "Firefox");
  Expect(OwnerKnown({0, 900, ""}, app), false, "known by the client's process alone");
  Expect(OwnerKnown({5151, 0, ""}, app), true, "known by its process");
  Expect(OwnerKnown({0, 0, "firefox"}, app), true, "known by its executable");

  // In a Flatpak every sandbox numbers its processes from 1: another sandboxed
  // program's stream may give this one's number. Only the executable tells.
  const ThisApp flatpak{2, "fogata", "", true};
  Expect(OwnOutput({2, 0, "spotify"}, flatpak), false, "a Flatpak's 2 in a Flatpak");
  Expect(OwnOutput({0, 2, "spotify"}, flatpak), false, "a client's 2 in a Flatpak");
  Expect(OwnOutput({2, 0, "fogata"}, flatpak), true, "own in a Flatpak");
  Expect(OwnOutput({0, 3133, "fogata"}, flatpak), true, "own in a Flatpak, the host's number");
  // Without its executable's name the numbers are all there is.
  const ThisApp nameless{2, "", "", true};
  Expect(OwnOutput({2, 0, "spotify"}, nameless), true, "own by number, no name known");

  // For tests, the one program whose sound is taken: every other one once its
  // executable is known, and none before (one judged early stayed left out for good,
  // and a lab's screen went without the sound it was to carry).
  const ThisApp lab{4242, "fogata", "pw-cat", false};
  Expect(OwnOutput({777, 0, ""}, lab), false, "the test program before its name");
  Expect(OwnerKnown({777, 0, ""}, lab), false, "known by its process, for a test");
  Expect(OwnOutput({777, 0, "pw-cat"}, lab), false, "the test program");
  Expect(OwnerKnown({777, 0, "pw-cat"}, lab), true, "the test program known");
  Expect(OwnOutput({777, 0, "pacat"}, lab), true, "another program, for a test");
  Expect(OwnOutput({4242, 0, ""}, lab), true, "own, for a test");
}

}  // namespace

int main() {
  // Programs' own streams.
  Taken("Firefox through pipewire-pulse",
        {{"media.class", "Stream/Output/Audio"},
         {"application.name", "Firefox"},
         {"application.process.binary", "firefox"},
         {"application.process.id", "4242"},
         {"media.role", "video"},
         {"node.name", "Firefox"}});
  Taken("Chromium",
        {{"application.process.binary", "chromium"}, {"media.name", "Playback"}});
  Taken("a game through Proton",
        {{"application.process.binary", "wine64-preloader"},
         {"application.name", "Game.exe"},
         {"media.role", "game"}});
  Taken("mpv on PipeWire itself",
        {{"application.process.binary", "mpv"},
         {"media.category", "Playback"},
         {"media.role", "Movie"},
         {"node.autoconnect", "true"}});
  Taken("a Flatpak app",
        {{"application.process.binary", "spotify"},
         {"pipewire.access.portal.app_id", "com.spotify.Client"}});
  Taken("paplay", {{"application.process.binary", "paplay"}});
  Taken("a stream that says nothing", {});
  Taken("node.virtual false", {{"application.process.binary", "vlc"}, {"node.virtual", "false"}});
  Taken("an empty node.link-group",
        {{"application.process.binary", "vlc"}, {"node.link-group", ""}});

  // Sound passed on from elsewhere.
  LeftOut("pw-loopback's playback",
          {{"application.process.binary", "pw-loopback"},
           {"node.group", "loopback-12-34"},
           {"node.link-group", "loopback-12-34"},
           {"node.virtual", "true"},
           {"media.class", "Stream/Output/Audio"}});
  LeftOut("libpipewire-module-loopback in pipewire's own config",
          {{"application.process.binary", "pipewire"},
           {"node.link-group", "loopback-1-20"}});
  LeftOut("pactl load-module module-loopback (pipewire-pulse)",
          {{"application.process.binary", "pipewire-pulse"},
           {"node.group", "loopback-77-3"},
           {"node.link-group", "loopback-77-3"},
           {"stream.is-loopback", "true"}});
  LeftOut("module-combine-sink's stream to one device",
          {{"application.process.binary", "pipewire-pulse"},
           {"node.link-group", "combine-stream-77-5"}});
  LeftOut("a filter-chain",
          {{"application.process.binary", "pipewire"},
           {"node.link-group", "filter-chain-1-9"},
           {"media.category", "Filter"}});
  LeftOut("EasyEffects, by its binary alone", {{"application.process.binary", "easyeffects"}});
  LeftOut("an effect host's node",
          {{"application.process.binary", "some-host"}, {"media.role", "filter"}});
  LeftOut("a virtual node", {{"application.process.binary", "x"}, {"node.virtual", "true"}});
  LeftOut("WirePlumber", {{"application.process.binary", "wireplumber"}});
  LeftOut("a node.group alone", {{"application.process.binary", "x"}, {"node.group", "g"}});

  Owners();

  if (failures == 0) std::printf("screen_sound_node_test: ok\n");
  return failures == 0 ? 0 : 1;
}
