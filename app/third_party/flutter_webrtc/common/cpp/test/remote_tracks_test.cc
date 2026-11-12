// A connection's received streams and tracks (fogata_remote_tracks.h), built with
// ThreadSanitizer by run_tests.sh: libwebrtc's signalling thread adds and removes them
// while the platform thread looks them up (setVolume while a description is applied), and
// any data race fails the run. Also: what is found, and that a search through a stream
// runs without the lock (it calls libwebrtc, which may wait for the signalling thread
// waiting here). Plain C++, no libwebrtc.

#include <chrono>
#include <cstdio>
#include <future>
#include <memory>
#include <string>
#include <thread>
#include <vector>

#include "fogata_remote_tracks.h"

using flutter_webrtc_plugin::RemoteTracks;

namespace {

int failures = 0;

void check(bool ok, const std::string& what) {
  if (!ok) {
    std::printf("FAIL: %s\n", what.c_str());
    failures++;
  }
}

using Track = std::shared_ptr<std::string>;
using Stream = std::shared_ptr<std::vector<Track>>;
using Remote = RemoteTracks<Stream, Track>;

Track InStream(const Stream& stream, const std::string& id) {
  for (const auto& track : *stream) {
    if (*track == id) return track;
  }
  return nullptr;
}

void Lookups() {
  Remote remote;
  check(remote.TrackFor("a", InStream) == nullptr, "nothing yet");
  auto a = std::make_shared<std::string>("a");
  remote.AddTrack("a", a);
  check(remote.TrackFor("a", InStream) == a, "a track by its id");
  auto b = std::make_shared<std::string>("b");
  auto stream = std::make_shared<std::vector<Track>>(std::vector<Track>{b});
  remote.AddStream("s", stream);
  check(remote.StreamFor("s") == stream, "a stream by its id");
  check(remote.StreamFor("t") == nullptr, "no other stream");
  check(remote.TrackFor("b", InStream) == b, "a track found in a stream");
  remote.RemoveTrack("a");
  check(remote.TrackFor("a", InStream) == nullptr, "a track removed");
  remote.RemoveStream("s");
  check(remote.TrackFor("b", InStream) == nullptr, "a stream removed");
  remote.RemoveTrack("absent");
  remote.RemoveStream("absent");
}

// A search through a stream that calls back into the same list (as a libwebrtc call
// may, by waiting for the signalling thread that adds a track) must not wait for itself.
void NoLockWhileSearching() {
  Remote remote;
  auto b = std::make_shared<std::string>("b");
  remote.AddStream("s", std::make_shared<std::vector<Track>>(std::vector<Track>{b}));
  auto done = std::async(std::launch::async, [&] {
    return remote.TrackFor("b", [&](const Stream& stream, const std::string& id) {
      std::thread other([&] { remote.AddTrack("c", std::make_shared<std::string>("c")); });
      other.join();
      return InStream(stream, id);
    });
  });
  if (done.wait_for(std::chrono::seconds(5)) != std::future_status::ready) {
    std::printf("FAIL: a search through a stream holds the lock\n");
    std::fflush(stdout);
    std::_Exit(1);
  }
  check(done.get() == b, "found while another thread added a track");
}

void Races() {
  Remote remote;
  constexpr int kRounds = 20000;
  std::thread signalling([&] {
    for (int i = 0; i < kRounds; i++) {
      const std::string id = std::to_string(i % 7);
      remote.AddTrack(id, std::make_shared<std::string>(id));
      if (i % 3 == 0) {
        remote.AddStream(id, std::make_shared<std::vector<Track>>(
                                 std::vector<Track>{std::make_shared<std::string>("s" + id)}));
      }
      if (i % 2 == 0) remote.RemoveTrack(std::to_string((i + 3) % 7));
      if (i % 5 == 0) remote.RemoveStream(std::to_string((i + 1) % 7));
    }
  });
  int found = 0;
  for (int i = 0; i < kRounds; i++) {
    const std::string id = std::to_string(i % 7);
    if (remote.TrackFor(id, InStream)) found++;
    if (remote.TrackFor("s" + id, InStream)) found++;
    if (remote.StreamFor(id)) found++;
  }
  signalling.join();
  (void)found;
}

}  // namespace

int main() {
  Lookups();
  NoLockWhileSearching();
  Races();
  if (failures == 0) std::printf("remote_tracks_test: ok\n");
  return failures == 0 ? 0 : 1;
}
