#ifndef FOGATA_TRACK_OWNER_H_
#define FOGATA_TRACK_OWNER_H_

// Fogata: what a track's or a stream's id stands for, by the owner the application holds
// it with. Ids are not unique across connections: a received track is named after the
// sender's msid, which another connection of this same application (mic_test.dart's
// LevelLoop) or any other participant may share with one of this device's own tracks.
// Looking an id up locally first once gave the microphone for a received track, and
// muting that received track's playback disabled the microphone in every call until it
// was opened again. Header only, with no libwebrtc, so that its test
// (test/track_owner_test.cc) builds anywhere.

#include <map>
#include <set>
#include <string>

namespace flutter_webrtc_plugin {

// Whether `owner` is a connection's id (a peerConnectionId), not this device's own
// ("local") nor left out (the package's own callers that pass none).
inline bool OwnedByConnection(const std::string& owner) {
  return !owner.empty() && owner != "local";
}

// The track `owner` means by an id. A connection means a track it receives, or else one
// it sends (a sender's track is held with its connection's id), and never any other: a
// connection that is gone, or that has no track of that id, finds nothing, rather than
// this device's own track or another connection's of the same id. Any other owner looks
// `anywhere`, as the package always did (local tracks first, then every connection's).
// `received`, `sent` and `anywhere` each give the track found, or a null one. What this
// gives must be held (a scoped_refptr) for as long as it is used: a track found among a
// connection's receivers or senders is a wrapper made for this one lookup.
template <typename Track, typename Received, typename Sent, typename Anywhere>
Track TrackOfOwner(const std::string& owner,
                   Received received,
                   Sent sent,
                   Anywhere anywhere) {
  if (!OwnedByConnection(owner))
    return anywhere();
  Track track = received();
  if (track)
    return track;
  return sent();
}

// Whether `trackDispose` of an id held with `owner` lets go of this device's own track of
// that id (its capturer, its screen capture): always for "local" or no owner; for a
// connection, only when the id is one it sends (a sender's track is this device's own)
// and not one it receives (a received track goes with its connection).
//
// Decided by where the id is found, never by comparing wrapper pointers: libwebrtc's C++
// wrapper makes a new RTCMediaTrack for the same native track on every
// RTCRtpReceiver::track() and RTCRtpSender::track(), so a sender's track is never the
// object local_tracks_ holds.
inline bool DisposesLocalTrack(const std::string& owner,
                               bool received,
                               bool sent) {
  return !OwnedByConnection(owner) || (!received && sent);
}

// The ids of the received tracks added to each local stream (mediaStreamAddTrack with a
// connection as the track's owner): the stream call.dart makes to show a participant's
// picture. Letting go of such a stream must not let go of this device's own track of the
// same id (stop the camera or the shared screen that has it); any other track of a local
// stream is let go of with it, by id, as the package always did.
//
// Again by where the track came from, not by its wrapper: the wrapper's RTCMediaStream
// makes new RTCMediaTrack objects for all its tracks whenever one is added or removed
// (MediaStreamImpl::OnChanged, called by the native stream inside AddTrack), so the
// objects video_tracks() and audio_tracks() give are in general not those local_tracks_
// holds: a shared screen's stream, to which its sound is added after its picture, gives
// a new object for the picture.
class ReceivedInStreams {
 public:
  void Add(const std::string& stream, const std::string& track) {
    streams_[stream].insert(track);
  }
  void Remove(const std::string& stream, const std::string& track) {
    auto it = streams_.find(stream);
    if (it == streams_.end())
      return;
    it->second.erase(track);
    if (it->second.empty())
      streams_.erase(it);
  }
  bool Holds(const std::string& stream, const std::string& track) const {
    auto it = streams_.find(stream);
    return it != streams_.end() && it->second.count(track) > 0;
  }
  void Forget(const std::string& stream) { streams_.erase(stream); }
  size_t size() const { return streams_.size(); }

 private:
  std::map<std::string, std::set<std::string>> streams_;
};

}  // namespace flutter_webrtc_plugin

#endif  // FOGATA_TRACK_OWNER_H_
