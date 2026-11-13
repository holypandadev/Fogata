#ifndef FLUTTER_WEBRTC_BASE_HXX
#define FLUTTER_WEBRTC_BASE_HXX

#include "flutter_common.h"
#include "fogata_denoise.h"
#include "fogata_track_owner.h"

#include <string.h>
#include <functional>
#include <list>
#include <map>
#include <memory>
#include <mutex>

#include "libwebrtc.h"

#include "rtc_audio_device.h"
#include "rtc_audio_processing.h"
#include "rtc_desktop_device.h"
#include "rtc_dtmf_sender.h"
#include "rtc_media_stream.h"
#include "rtc_media_track.h"
#include "rtc_mediaconstraints.h"
#include "rtc_peerconnection.h"
#include "rtc_peerconnection_factory.h"
#include "rtc_video_device.h"

namespace flutter_webrtc_plugin {

using namespace libwebrtc;

class FlutterVideoRenderer;
class FlutterRTCDataChannelObserver;
class FlutterPeerConnectionObserver;

class FlutterWebRTCBase {
 public:
  friend class FlutterMediaStream;
  friend class FlutterPeerConnection;
  friend class FlutterVideoRendererManager;
  friend class FlutterDataChannel;
  friend class FlutterPeerConnectionObserver;
  friend class FlutterScreenCapture;
  enum ParseConstraintType { kMandatory, kOptional };

 public:
  FlutterWebRTCBase(BinaryMessenger* messenger,
                    TextureRegistrar* textures,
                    TaskRunner* task_runner);
  ~FlutterWebRTCBase();

  // Initializes WebRTC and creates the peer connection factory the first time
  // it is needed, applying the process global field trials that the
  // `initialize()` options ask for. Field trials are read when the factory is
  // created, so only the first call decides them; later calls are no-ops.
  void EnsureWebRTCInitialized(bool enable_warp = false,
                               bool zero_playout_delay = false);

  // Fogata: whether the connections that send a shared screen share one encoder of
  // it: the field trial EnsureWebRTCInitialized sets, as the environment decides it
  // (FOGATA_SHARED_SCREEN_ENCODE=off, or FOGATA_WEBRTC_FIELD_TRIALS in debug builds).
  static bool SharedScreenEncode();

  virtual scoped_refptr<RTCAudioProcessing> audio_processing() {
    EnsureWebRTCInitialized();
    return audio_processing_;
  }

  virtual scoped_refptr<RTCMediaTrack> MediaTrackForId(const std::string& id);

  // Fogata: the track `id` as the application holds it, `owner` being the track's own
  // peerConnectionId ("local" for this device's own tracks). For a connection, a track
  // it receives, else one it sends (a sender's track is held with its connection's id),
  // and nothing else: not this device's own track of that id, nor another connection's,
  // also once that connection is gone (TrackOfOwner, fogata_track_owner.h). A connection
  // that receives a track sent by another connection of this same application
  // (mic_test.dart's LevelLoop, which reads the microphone's level while alone in a
  // call) gets it under the same id: a received track is named after the sender's msid.
  // The plain lookup, local tracks first, then gave the microphone for the received
  // track, and muting the loop's playback disabled the microphone in every call until it
  // was opened again. "local", or no owner, is the plain lookup.
  //
  // What it gives must be held (a scoped_refptr) for as long as it is used: a track
  // found among the connection's receivers or senders is a wrapper libwebrtc's C++
  // wrapper makes for this one lookup, which nothing else keeps. Nor is it ever the
  // object another lookup of the same track gives: compare tracks by where they were
  // found, or by id, never by pointer.
  scoped_refptr<RTCMediaTrack> MediaTrackForId(const std::string& id,
                                               const std::string& owner);

  // Fogata: the track `id` the connection `owner` receives (its observer's list, then
  // its receivers), or null; held as MediaTrackForId's.
  scoped_refptr<RTCMediaTrack> ReceivedTrackForId(const std::string& id,
                                                  const std::string& owner);

  // Fogata: the track `id` the connection `owner` sends (its senders'), or null; held as
  // MediaTrackForId's.
  scoped_refptr<RTCMediaTrack> SentTrackForId(const std::string& id,
                                              const std::string& owner);

  // Fogata: records from the input at `index` of the list, whose id is `id` (for the
  // list's first entry, the system's default, nothing more is kept). WebRTC keeps only
  // the index, and on Linux it reads the list again each time it starts recording, so
  // once another input comes or goes the same index may be another microphone, or the
  // default one.
  void SelectRecordingDevice(uint16_t index, const std::string& id);

  // Fogata: finds the chosen input in the list again and selects it at its new place,
  // after the list changed. While it is gone, the system's default is selected, as a
  // recording whose input goes away moves there by itself; it is taken again if it
  // comes back.
  void KeepRecordingDevice();

  // Fogata: keeps WebRTC's voice engine running from now until the plugin goes, with an
  // idle connection of its own; called once a connection has been made, so the engine
  // has started (made first, without a sound server it would end the application). The
  // engine stops when the last connection is freed, and the audio device with it: it
  // then lists no microphone and no speaker, forgets the ones chosen, and follows no
  // device plugged in or out, until a connection starts it again.
  void KeepVoiceEngine();

  // Fogata: turns the microphone's noise suppression on or off, from the next 10 ms.
  // The first call makes the suppressor with the table of functions at `api`, which the
  // application hands over; false if there is none to make it with.
  bool SuppressNoise(bool on, int64_t api);

  std::string GenerateUUID();

  RTCPeerConnection* PeerConnectionForId(const std::string& id);

  void RemovePeerConnectionForId(const std::string& id);

  void RemoveMediaTrackForId(const std::string& id);

  FlutterPeerConnectionObserver* PeerConnectionObserversForId(
      const std::string& id);

  void RemovePeerConnectionObserversForId(const std::string& id);

  scoped_refptr<RTCMediaStream> MediaStreamForId(
      const std::string& id,
      std::string ownerTag = std::string());

  void RemoveStreamForId(const std::string& id);

  bool ParseConstraints(const EncodableMap& constraints,
                        RTCConfiguration* configuration);

  scoped_refptr<RTCMediaConstraints> ParseMediaConstraints(
      const EncodableMap& constraints);

  bool ParseRTCConfiguration(const EncodableMap& map,
                             RTCConfiguration& configuration);

  scoped_refptr<RTCMediaTrack> MediaTracksForId(const std::string& id);

  void RemoveTracksForId(const std::string& id);

  EventChannelProxy* event_channel();

  libwebrtc::scoped_refptr<libwebrtc::RTCRtpSender> GetRtpSenderById(
      RTCPeerConnection* pc,
      std::string id);

  libwebrtc::scoped_refptr<libwebrtc::RTCRtpReceiver> GetRtpReceiverById(
      RTCPeerConnection* pc,
      std::string id);

 private:
  void ParseConstraints(const EncodableMap& src,
                        scoped_refptr<RTCMediaConstraints> mediaConstraints,
                        ParseConstraintType type = kMandatory);

  bool CreateIceServers(const EncodableList& iceServersArray,
                        IceServer* ice_servers);

 protected:
  bool webrtc_initialized_ = false;
  // Whether `enableWARP` was requested; peer connections created afterwards
  // default to DSCP marking. See EnsureWebRTCInitialized().
  bool warp_enabled_ = false;
  scoped_refptr<RTCPeerConnectionFactory> factory_;
  scoped_refptr<RTCAudioDevice> audio_device_;
  scoped_refptr<RTCVideoDevice> video_device_;
  scoped_refptr<RTCDesktopDevice> desktop_device_;
  scoped_refptr<RTCAudioProcessing> audio_processing_;
  RTCConfiguration configuration_;
  // Fogata: an idle connection of the plugin's own, made with the first connection and
  // kept until the plugin goes: see KeepVoiceEngine().
  scoped_refptr<RTCPeerConnection> voice_engine_keeper_;

  std::map<std::string, scoped_refptr<RTCPeerConnection>> peerconnections_;
  std::map<std::string, scoped_refptr<RTCMediaStream>> local_streams_;
  std::map<std::string, scoped_refptr<RTCMediaTrack>> local_tracks_;
  // Fogata: the received tracks added to each local stream, by the stream's id: not let
  // go of as this device's own when the stream is (ReceivedInStreams).
  ReceivedInStreams received_in_streams_;
  std::map<std::string, scoped_refptr<RTCVideoCapturer>> video_capturers_;
  // Fogata: stops what feeds a local track (the portal's screen capture) when the
  // track or its stream is disposed.
  std::map<std::string, std::function<void()>> track_stoppers_;
  // Fogata: sets how many frames a second a screen captured through the portal sends,
  // by its track.
  std::map<std::string, std::function<void(int)>> track_rates_;
  // Fogata: gives the encoders of a screen captured through the portal its last frame
  // again, by its track; false when there is none.
  std::map<std::string, std::function<bool()>> track_refreshers_;
  // Fogata: the input chosen, by id ("" for the system's default), and its place in
  // the list when last selected.
  std::mutex recording_device_mutex_;
  std::string recording_device_id_;
  uint16_t recording_device_index_ = 0;
  // False while the audio device refused the last selection: the place above is then
  // the one it still has.
  bool recording_device_selected_ = true;
  // Fogata: the microphone's noise suppressor, once made. Never freed: libwebrtc keeps a
  // plain pointer to it for as long as the factory lives and cannot be given none.
  FogataDenoiser* denoiser_ = nullptr;
  std::map<int64_t, std::shared_ptr<FlutterVideoRenderer>> renders_;
  std::map<std::string, std::shared_ptr<FlutterRTCDataChannelObserver>>
      data_channel_observers_;
  std::map<std::string, std::shared_ptr<FlutterPeerConnectionObserver>>
      peerconnection_observers_;
  mutable std::mutex mutex_;

  void lock() { mutex_.lock(); }
  void unlock() { mutex_.unlock(); }

 protected:
  BinaryMessenger* messenger_;
  TaskRunner* task_runner_;
  TextureRegistrar* textures_;
  std::unique_ptr<EventChannelProxy> event_channel_;
};

}  // namespace flutter_webrtc_plugin

#endif  // !FLUTTER_WEBRTC_BASE_HXX
