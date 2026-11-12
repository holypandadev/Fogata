// What an id stands for by the owner the application holds it with
// (fogata_track_owner.h): a connection's id finds a track it receives, or else one it
// sends, and never this device's own track of the same id nor another connection's,
// also once that connection is gone; trackDispose and a stream let go of decide whether
// this device's own track goes by where the id was found, never by comparing wrapper
// pointers, which libwebrtc's C++ wrapper makes anew for the same native track (modelled
// here). Plain C++, no libwebrtc.

#include <cstdio>
#include <map>
#include <memory>
#include <set>
#include <vector>
#include <string>

#include "fogata_track_owner.h"

using flutter_webrtc_plugin::DisposesLocalTrack;
using flutter_webrtc_plugin::OwnedByConnection;
using flutter_webrtc_plugin::ReceivedInStreams;
using flutter_webrtc_plugin::TrackOfOwner;

namespace {

int failures = 0;

void check(bool ok, const std::string& what) {
  if (!ok) {
    std::printf("FAIL: %s\n", what.c_str());
    failures++;
  }
}

using Track = std::shared_ptr<std::string>;
using Tracks = std::map<std::string, Track>;

Track Find(const Tracks& tracks, const std::string& id) {
  auto it = tracks.find(id);
  return it == tracks.end() ? nullptr : it->second;
}

// This device's own tracks and, by connection, those each receives and sends.
struct Plugin {
  Tracks local;
  std::map<std::string, Tracks> received;
  std::map<std::string, Tracks> sent;

  Track Lookup(const std::string& id, const std::string& owner) const {
    auto in = received.find(owner);
    auto out = sent.find(owner);
    return TrackOfOwner<Track>(
        owner,
        [&] { return in == received.end() ? nullptr : Find(in->second, id); },
        [&] { return out == sent.end() ? nullptr : Find(out->second, id); },
        [&]() -> Track {
          // The package's own lookup: local first, then every connection's.
          if (auto track = Find(local, id))
            return track;
          for (const auto& connection : received) {
            if (auto track = Find(connection.second, id))
              return track;
          }
          return nullptr;
        });
  }
};

// libwebrtc's C++ wrapper as the plugin meets it: a wrapper object stands for a native
// track, and a new one is made for the same native track again and again. Its
// RTCMediaStream (MediaStreamImpl) makes new wrappers for all its tracks whenever one is
// added or removed (OnChanged, called by the native stream inside AddTrack), and then
// keeps the one added too.
using Native = std::shared_ptr<std::string>;
using Wrapper = std::shared_ptr<Native>;

Wrapper Wrap(const Native& native) {
  return std::make_shared<Native>(native);
}

struct Stream {
  std::vector<Native> natives;
  std::vector<Wrapper> wrappers;

  void Add(const Wrapper& track) {
    natives.push_back(*track);
    wrappers.clear();
    for (const auto& native : natives)
      wrappers.push_back(Wrap(native));
    wrappers.push_back(track);
  }
};

// What MediaStreamDispose lets go of (local tracks forgotten, captures stopped), by id:
// every track of the stream but those noted as received in it.
std::set<std::string> LetGo(const Stream& stream,
                            const std::string& stream_id,
                            const ReceivedInStreams& received) {
  std::set<std::string> stopped;
  for (const auto& wrapper : stream.wrappers) {
    if (!received.Holds(stream_id, **wrapper))
      stopped.insert(**wrapper);
  }
  return stopped;
}

// The rule it replaced: a track of the stream let go of only when it is the very
// wrapper local_tracks_ holds for its id, or none is held.
std::set<std::string> LetGoByWrapper(
    const Stream& stream,
    const std::map<std::string, Wrapper>& local) {
  std::set<std::string> stopped;
  for (const auto& wrapper : stream.wrappers) {
    auto held = local.find(**wrapper);
    if (held == local.end() || held->second == wrapper)
      stopped.insert(**wrapper);
  }
  return stopped;
}

void StreamsLetGo() {
  std::map<std::string, Wrapper> local;  // local_tracks_
  ReceivedInStreams received;

  // A shared screen: its picture is added, then its sound (flutter_screen_capture.cc).
  // The stream no longer gives the wrapper local_tracks_ holds for the picture.
  local["screen"] = Wrap(std::make_shared<std::string>("screen"));
  local["screen-sound"] = Wrap(std::make_shared<std::string>("screen-sound"));
  Stream screen;
  screen.Add(local["screen"]);
  screen.Add(local["screen-sound"]);
  auto stopped = LetGo(screen, "s1", received);
  check(stopped.count("screen") == 1 && stopped.count("screen-sound") == 1,
        "a shared screen's stream let go of stops its picture and its sound");
  check(LetGoByWrapper(screen, local).count("screen") == 0,
        "the model: compared by wrapper, the screen's capture was left running");

  // A camera, alone in its stream.
  local["cam"] = Wrap(std::make_shared<std::string>("cam"));
  Stream own;
  own.Add(local["cam"]);
  check(LetGo(own, "s2", received).count("cam") == 1,
        "a camera's stream let go of stops the camera");

  // The stream made to show a participant's picture, whose id is the camera's.
  Stream shown;
  received.Add("s3", "cam");
  shown.Add(Wrap(std::make_shared<std::string>("cam")));
  check(LetGo(shown, "s3", received).empty(),
        "a stream showing a received track leaves the camera of its id");
  check(received.Holds("s3", "cam") && !received.Holds("s2", "cam"),
        "only the stream it was added to holds it");
  received.Remove("s3", "cam");
  check(!received.Holds("s3", "cam") && received.size() == 0,
        "a received track removed from its stream is forgotten");
  received.Add("s3", "cam");
  received.Forget("s3");
  check(received.size() == 0, "a stream let go of is forgotten");
}

}  // namespace

int main() {
  auto camera = std::make_shared<std::string>("this device's camera");
  auto picture = std::make_shared<std::string>("the camera received on pc-a");
  auto other = std::make_shared<std::string>("the camera received on pc-b");
  auto voice = std::make_shared<std::string>("a voice received on pc-a");

  Plugin plugin;
  // A received track named as one of this device's own (a loop of its own, or a
  // participant whose msid names it so), and the same id on another connection.
  plugin.local["cam"] = camera;
  plugin.received["pc-a"]["cam"] = picture;
  plugin.received["pc-a"]["voice"] = voice;
  plugin.received["pc-b"]["cam"] = other;
  plugin.sent["pc-a"]["cam"] = camera;

  check(OwnedByConnection("pc-a"), "a connection's id is a connection's");
  check(!OwnedByConnection("local"), "'local' is no connection");
  check(!OwnedByConnection(""), "no owner is no connection");

  check(plugin.Lookup("cam", "pc-a") == picture,
        "a connection's received track comes before its own camera");
  check(plugin.Lookup("cam", "pc-b") == other,
        "each connection finds what it received, not another's");
  check(plugin.Lookup("cam", "local") == camera, "'local' finds the camera");
  check(plugin.Lookup("cam", "") == camera,
        "no owner looks as the package did, local first");
  check(plugin.Lookup("voice", "") == voice,
        "no owner still finds a received track");

  // What a connection sends, when it receives nothing of that id: a sender's track is
  // held with its connection's id.
  plugin.received["pc-a"].erase("cam");
  check(plugin.Lookup("cam", "pc-a") == camera,
        "a connection finds the track it sends");

  // A connection that neither receives nor sends the id finds nothing: before, the
  // camera was taken for it, and stopping or muting the received track stopped or
  // muted the camera.
  check(plugin.Lookup("cam", "pc-c") == nullptr,
        "a connection that has no such track finds nothing, not the camera");
  check(plugin.Lookup("voice", "pc-b") == nullptr,
        "another connection's received track is not found");
  plugin.received.erase("pc-a");
  plugin.sent.erase("pc-a");
  check(plugin.Lookup("voice", "pc-a") == nullptr,
        "a connection gone finds nothing");
  check(plugin.Lookup("cam", "pc-a") == nullptr,
        "a connection gone finds nothing, not the camera of the same id");

  // trackDispose with a connection as owner: only a track it sends (this device's own),
  // never one it receives, nor an id it has neither of.
  check(DisposesLocalTrack("local", false, false), "'local' lets go of its track");
  check(DisposesLocalTrack("", true, false), "no owner lets go as the package did");
  check(DisposesLocalTrack("pc-a", false, true),
        "a sender's track, held with its connection's id, is let go of");
  check(!DisposesLocalTrack("pc-a", true, true),
        "a received track is not, even if the connection sends one of its id");
  check(!DisposesLocalTrack("pc-a", true, false), "a received track is not");
  check(!DisposesLocalTrack("pc-a", false, false),
        "an id the connection has nothing of is not");

  StreamsLetGo();

  if (failures == 0)
    std::printf("track_owner_test: all passed\n");
  return failures == 0 ? 0 : 1;
}
