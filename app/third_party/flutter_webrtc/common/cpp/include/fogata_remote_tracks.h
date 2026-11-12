#ifndef FOGATA_REMOTE_TRACKS_H_
#define FOGATA_REMOTE_TRACKS_H_

// Fogata: the streams and tracks a connection received, by id. libwebrtc adds and
// removes them on its signalling thread (the observer's OnAddStream, OnTrack,
// OnRemoveTrack) while the platform thread looks them up (setVolume, a track enabled or
// disabled, a renderer given a stream): a map changed on one thread while read on
// another is undefined behaviour. Every access takes the lock, and the lock is never
// held while calling out: a search through a stream's tracks asks libwebrtc, which may
// wait for the very signalling thread that waits here. Header only, with no libwebrtc,
// so that its test (test/remote_tracks_test.cc) builds anywhere.

#include <map>
#include <mutex>
#include <string>
#include <utility>
#include <vector>

namespace flutter_webrtc_plugin {

template <typename Stream, typename Track>
class RemoteTracks {
 public:
  void AddStream(const std::string& id, Stream stream) {
    std::lock_guard<std::mutex> guard(mutex_);
    streams_[id] = std::move(stream);
  }

  void RemoveStream(const std::string& id) {
    Stream gone;
    {
      std::lock_guard<std::mutex> guard(mutex_);
      auto it = streams_.find(id);
      if (it == streams_.end()) return;
      gone = std::move(it->second);
      streams_.erase(it);
    }
    // Let go of outside the lock: the last reference may free it.
  }

  void AddTrack(const std::string& id, Track track) {
    std::lock_guard<std::mutex> guard(mutex_);
    tracks_[id] = std::move(track);
  }

  void RemoveTrack(const std::string& id) {
    Track gone;
    {
      std::lock_guard<std::mutex> guard(mutex_);
      auto it = tracks_.find(id);
      if (it == tracks_.end()) return;
      gone = std::move(it->second);
      tracks_.erase(it);
    }
  }

  // The stream of that id, or a null one.
  Stream StreamFor(const std::string& id) const {
    std::lock_guard<std::mutex> guard(mutex_);
    auto it = streams_.find(id);
    return it == streams_.end() ? Stream() : it->second;
  }

  // The track of that id, or else the first that `in_stream(stream, id)` finds in one of
  // the streams; called without the lock, on a copy of the list.
  template <typename InStream>
  Track TrackFor(const std::string& id, InStream in_stream) const {
    std::vector<Stream> streams;
    {
      std::lock_guard<std::mutex> guard(mutex_);
      auto it = tracks_.find(id);
      if (it != tracks_.end()) return it->second;
      streams.reserve(streams_.size());
      for (const auto& entry : streams_) streams.push_back(entry.second);
    }
    for (const auto& stream : streams) {
      Track track = in_stream(stream, id);
      if (track) return track;
    }
    return Track();
  }

 private:
  mutable std::mutex mutex_;
  std::map<std::string, Stream> streams_;
  std::map<std::string, Track> tracks_;
};

}  // namespace flutter_webrtc_plugin

#endif  // FOGATA_REMOTE_TRACKS_H_
