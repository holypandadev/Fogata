#include "flutter_webrtc_base.h"

#include "flutter_data_channel.h"
#include "flutter_peerconnection.h"

#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <ctime>
#include <string>
#include <vector>

#include "flutter_utf8_sanitize.h"
#include "fogata_denoise_table.h"
#include "fogata_track_owner.h"
#include "helper.h"
#include "rtc_field_trials.h"
#include "rtc_logging.h"
#include "task_runner.h"
#ifdef __linux__
#include "camera_access.h"
#include "input_watcher.h"
#endif

namespace flutter_webrtc_plugin {

// Fogata: libwebrtc's own log on standard error, each line with the time, for tests on
// real networks (see EnsureWebRTCInitialized).
static void LogToStderr(const libwebrtc::string& message) {
  const auto now = std::chrono::system_clock::now();
  const std::time_t seconds = std::chrono::system_clock::to_time_t(now);
  const auto millis = std::chrono::duration_cast<std::chrono::milliseconds>(
                          now.time_since_epoch())
                          .count() %
                      1000;
  char clock[16] = {};
  std::tm parts = {};
#ifdef _WIN32
  localtime_s(&parts, &seconds);
#else
  localtime_r(&seconds, &parts);
#endif
  std::strftime(clock, sizeof(clock), "%H:%M:%S", &parts);
  std::fprintf(stderr, "webrtc %s.%03d %s", clock, static_cast<int>(millis),
               message.c_string());
}

// An environment variable's value, or empty.
static std::string Env(const char* name) {
#ifdef _WIN32
  char* value = nullptr;
  size_t length = 0;
  if (_dupenv_s(&value, &length, name) != 0 || value == nullptr) {
    return "";
  }
  std::string result(value);
  free(value);
  return result;
#else
  const char* value = std::getenv(name);
  return value ? value : "";
#endif
}

const char* kEventChannelName = "FlutterWebRTC.Event";

// `WebRTC-ForcePlayoutDelay` renders every frame as soon as it is decoded
// instead of holding it back for the jitter buffer target delay. Opted into
// through the `zeroPlayoutDelay` initialize() option, and read at the same
// moment as `WebRTC-IceHandshakeDtls` below.
const char kFieldTrialForcePlayoutDelayKey[] = "WebRTC-ForcePlayoutDelay";

// `WebRTC-ForcePlayoutDelay` is parametrized rather than a plain "Enabled"
// flag: pin both ends of the playout delay to 0 ms.
const char kFieldTrialZeroPlayoutDelayValue[] = "min_ms:0,max_ms:0";

// Fogata: WebRTC's newer loss-based bandwidth estimate holds the estimate where it is
// and stops probing while packets are lost at random (Wi-Fi, satellite links), so a
// shared screen or a camera stayed at a few hundred kbit/s on links with far more room.
// The older one, used instead, lowers it only past 10 % loss; the delay-based estimate
// still finds the real limit of a congested link.
// Each video encoder also runs on one thread only (`encoder_thread_limit:1`):
// libvpx gives a VP8 encoder past 640x480 three threads, which wait for each other
// by spinning. Every person watching has an encoder of their own, so a call of four
// with cameras at 540p ran nine encoder threads: on a 16-thread desktop they took
// 2.1 cores and sent 21 frames a second, against 0.63 cores and 30 frames on one
// thread each (measured with tools/call-lab).
const char kFogataFieldTrials[] =
    "WebRTC-Bwe-LossBasedBweV2/Enabled:false/"
    "WebRTC-VideoEncoderSettings/encoder_thread_limit:1/";

// Builds a "Key/Value/" field trial entry.
static std::string FieldTrial(const std::string& key,
                              const std::string& value) {
  return key + "/" + value + "/";
}

// Builds a "Key/Enabled/" field trial entry.
static std::string EnabledFieldTrial(const std::string& key) {
  return FieldTrial(key, kRTCFieldTrialEnabledValue);
}

FlutterWebRTCBase::FlutterWebRTCBase(BinaryMessenger* messenger,
                                     TextureRegistrar* textures,
                                     TaskRunner *task_runner)
    : messenger_(messenger), task_runner_(task_runner), textures_(textures) {
  event_channel_ = EventChannelProxy::Create(messenger_, task_runner_, kEventChannelName);
}

FlutterWebRTCBase::~FlutterWebRTCBase() {
#ifdef __linux__
  InputWatcher::Stop();
#endif
  if (voice_engine_keeper_) {
    voice_engine_keeper_->Close();
    factory_->Delete(voice_engine_keeper_);
    voice_engine_keeper_ = nullptr;
  }
  if (webrtc_initialized_) {
    LibWebRTC::Terminate();
  }
}

void FlutterWebRTCBase::EnsureWebRTCInitialized(bool enable_warp,
                                                bool zero_playout_delay) {
  if (webrtc_initialized_) {
    return;
  }
  webrtc_initialized_ = true;
  warp_enabled_ = enable_warp;

  // WARP (WebRTC Abridged Roundtrip Protocol, draft-uberti-tsvwg-warp) is
  // opted into through the `enableWARP` initialize() option. The part of it
  // that libwebrtc implements is `WebRTC-IceHandshakeDtls`, the DTLS handshake
  // piggybacked on the ICE STUN binding exchange. Field trials are process
  // global and are read when the peer connection factory builds its
  // environment, which is why the factory is created here instead of in the
  // constructor: the options only arrive with the initialize() call.
  // Fogata: FOGATA_WEBRTC_FIELD_TRIALS ("Key/Value/Key/Value/") replaces Fogata's own
  // field trials, for trying libwebrtc's settings in tests (debug builds only).
#ifdef NDEBUG
  const std::string trials;
#else
  const std::string trials = Env("FOGATA_WEBRTC_FIELD_TRIALS");
#endif
  std::vector<libwebrtc::string> field_trials;
  field_trials.push_back(trials.empty() ? std::string(kFogataFieldTrials)
                                        : trials);
  if (enable_warp) {
    field_trials.push_back(
        EnabledFieldTrial(kRTCFieldTrialIceHandshakeDtlsKey));
  }
  if (zero_playout_delay) {
    field_trials.push_back(FieldTrial(kFieldTrialForcePlayoutDelayKey,
                                      kFieldTrialZeroPlayoutDelayValue));
  }

  // Fogata: FOGATA_WEBRTC_LOG=verbose, info, warning or error sends libwebrtc's own log
  // (bandwidth estimates, probes, route changes) to standard error.
  const std::string name = Env("FOGATA_WEBRTC_LOG");
  if (!name.empty()) {
    const libwebrtc::RTCLoggingSeverity severity =
        name == "verbose"   ? libwebrtc::Verbose
        : name == "info"    ? libwebrtc::Info
        : name == "warning" ? libwebrtc::Warning
                            : libwebrtc::Error;
    libwebrtc::LibWebRTCLogging::setLogSink(severity, &LogToStderr);
  }

  LibWebRTC::InitializeWithFieldTrials(
      libwebrtc::vector<libwebrtc::string>(field_trials));

  factory_ = LibWebRTC::CreateRTCPeerConnectionFactory();
  factory_->Initialize();
  audio_device_ = factory_->GetAudioDevice();
  video_device_ = factory_->GetVideoDevice();
#ifdef __linux__
  // Fogata: in a Flatpak the cameras come through the Camera portal and PipeWire
  // (camera_access.h).
  const CameraWay camera_way = CameraWayHere();
  video_device_->SetCameraPortal(camera_way.portal, camera_way.direct_fallback);
#endif
  desktop_device_ = factory_->GetDesktopDevice();
  audio_processing_ = factory_->GetAudioProcessing();

  audio_device_->OnDeviceChange([&] {
    // Fogata: the chosen input may have moved in the list. Not on the thread
    // that tells, which may be WebRTC's own.
    task_runner_->EnqueueTask([this] { KeepRecordingDevice(); });
    EncodableMap info;
    info[EncodableValue("event")] = "onDeviceChange";
    event_channel()->Success(EncodableValue(info), false);
  });
#ifdef __linux__
  // Fogata: libwebrtc tells nothing on Linux when inputs come and go. The watcher
  // tells on its own thread; the audio device and the event channel are used from
  // the platform thread only, in one task: the input is chosen again, then Dart
  // hears of it as elsewhere ("onDeviceChange"), so a call can say when its
  // microphone goes away and comes back. The task runner runs each task later,
  // from the main loop and with its queue let go, so the event's own delivery can be
  // queued from the task.
  InputWatcher::Start([this] {
    task_runner_->EnqueueTask([this] {
      KeepRecordingDevice();
      EncodableMap info;
      info[EncodableValue("event")] = "onDeviceChange";
      event_channel()->Success(EncodableValue(info), false);
    });
  });
#endif
}

void FlutterWebRTCBase::SelectRecordingDevice(uint16_t index,
                                              const std::string& id) {
  std::lock_guard<std::mutex> guard(recording_device_mutex_);
  recording_device_id_ = index == 0 ? std::string() : id;
  // Refused (while recording, on some systems), the place stays as it was and the
  // next change in the list, or the next selection, tries again.
  recording_device_selected_ = audio_device_->SetRecordingDevice(index) == 0;
  if (recording_device_selected_) recording_device_index_ = index;
}

void FlutterWebRTCBase::KeepRecordingDevice() {
  std::lock_guard<std::mutex> guard(recording_device_mutex_);
  // The system's default stays the list's first entry.
  if (!audio_device_ ||
      (recording_device_id_.empty() && recording_device_selected_)) {
    return;
  }
  char name[RTCAudioDevice::kAdmMaxDeviceNameSize + 1] = {0};
  char guid[RTCAudioDevice::kAdmMaxGuidSize + 1] = {0};
  // The same id as flutter_media_stream.cc gives a device: its guid, else its
  // name. False when the list cannot be read (it may be changing).
  auto id_at = [&](uint16_t i, std::string& id) {
    name[0] = guid[0] = 0;
    if (audio_device_->RecordingDeviceName(i, name, guid) != 0) return false;
    id = SanitizeUtf8ForFlutter(guid[0] != 0 ? guid : name);
    return true;
  };
  const int count = audio_device_->RecordingDevices();
  if (count <= 0) return;
  std::string id;
  if (recording_device_selected_ && recording_device_index_ > 0 &&
      recording_device_index_ < count && id_at(recording_device_index_, id) &&
      id == recording_device_id_) {
    return;
  }
  uint16_t found = 0;
  for (int i = 1; i < count && found == 0; i++) {
    if (!id_at(static_cast<uint16_t>(i), id)) return;
    if (id == recording_device_id_) found = static_cast<uint16_t>(i);
  }
  if (recording_device_selected_ && found == recording_device_index_) return;
  // As in SelectRecordingDevice: refused, it is tried again on the next change.
  recording_device_selected_ = audio_device_->SetRecordingDevice(found) == 0;
  if (recording_device_selected_) recording_device_index_ = found;
}

void FlutterWebRTCBase::KeepVoiceEngine() {
  if (voice_engine_keeper_ || !factory_) return;
  // No ICE servers and nothing added: it never gathers candidates, sends or receives.
  RTCConfiguration idle;
  voice_engine_keeper_ = factory_->Create(idle, RTCMediaConstraints::Create());
}

bool FlutterWebRTCBase::SuppressNoise(bool on, int64_t api) {
  if (denoiser_ == nullptr) {
    const FogataDenoiseApi* table = FogataDenoiseTable(api);
    if (table == nullptr || !audio_processing()) {
      return false;
    }
    denoiser_ = new FogataDenoiser(table);
    audio_processing_->SetCapturePostProcessing(denoiser_);
  }
  denoiser_->SetOn(on);
  return true;
}

EventChannelProxy* FlutterWebRTCBase::event_channel() {
  return event_channel_ ? event_channel_.get() : nullptr;
}

std::string FlutterWebRTCBase::GenerateUUID() {
  return libwebrtc::Helper::CreateRandomUuid().std_string();
}

RTCPeerConnection* FlutterWebRTCBase::PeerConnectionForId(
    const std::string& id) {
  auto it = peerconnections_.find(id);

  if (it != peerconnections_.end())
    return (*it).second.get();

  return nullptr;
}

void FlutterWebRTCBase::RemovePeerConnectionForId(const std::string& id) {
  auto it = peerconnections_.find(id);
  if (it != peerconnections_.end())
    peerconnections_.erase(it);
}

scoped_refptr<RTCMediaTrack> FlutterWebRTCBase ::MediaTrackForId(const std::string& id) {
  auto it = local_tracks_.find(id);

  if (it != local_tracks_.end())
    return (*it).second;

  for (auto kv : peerconnection_observers_) {
    auto pco = kv.second.get();
    auto track = pco->MediaTrackForId(id);
    if (track != nullptr)
      return track;
  }

  return nullptr;
}

scoped_refptr<RTCMediaTrack> FlutterWebRTCBase::MediaTrackForId(
    const std::string& id,
    const std::string& owner) {
  return TrackOfOwner<scoped_refptr<RTCMediaTrack>>(
      owner, [&]() { return ReceivedTrackForId(id, owner); },
      [&]() { return SentTrackForId(id, owner); },
      [&]() { return MediaTrackForId(id); });
}

scoped_refptr<RTCMediaTrack> FlutterWebRTCBase::ReceivedTrackForId(
    const std::string& id,
    const std::string& owner) {
  auto observer = peerconnection_observers_.find(owner);
  if (observer != peerconnection_observers_.end()) {
    if (auto received = observer->second->MediaTrackForId(id))
      return received;
  }
  // Not listed by the observer: the connection's own receivers, as macOS looks. Each
  // receiver->track() is a new wrapper, held only by what this returns.
  auto connection = peerconnections_.find(owner);
  if (connection == peerconnections_.end())
    return nullptr;
  auto receivers = connection->second->receivers();
  for (auto receiver : receivers.std_vector()) {
    auto track = receiver->track();
    if (track.get() && track->id().std_string() == id)
      return track;
  }
  return nullptr;
}

scoped_refptr<RTCMediaTrack> FlutterWebRTCBase::SentTrackForId(
    const std::string& id,
    const std::string& owner) {
  // Each sender->track() is a new wrapper, held only by what this returns.
  auto connection = peerconnections_.find(owner);
  if (connection == peerconnections_.end())
    return nullptr;
  auto senders = connection->second->senders();
  for (auto sender : senders.std_vector()) {
    auto track = sender->track();
    if (track.get() && track->id().std_string() == id)
      return track;
  }
  return nullptr;
}

void FlutterWebRTCBase::RemoveMediaTrackForId(const std::string& id) {
  auto it = local_tracks_.find(id);
  if (it != local_tracks_.end())
    local_tracks_.erase(it);
}

FlutterPeerConnectionObserver* FlutterWebRTCBase::PeerConnectionObserversForId(
    const std::string& id) {
  auto it = peerconnection_observers_.find(id);

  if (it != peerconnection_observers_.end())
    return (*it).second.get();

  return nullptr;
}

void FlutterWebRTCBase::RemovePeerConnectionObserversForId(
    const std::string& id) {
  auto it = peerconnection_observers_.find(id);
  if (it != peerconnection_observers_.end())
    peerconnection_observers_.erase(it);
}

scoped_refptr<RTCMediaStream> FlutterWebRTCBase::MediaStreamForId(
    const std::string& id, std::string ownerTag) {
  if (!ownerTag.empty()) {
    if (ownerTag == "local") {
      auto it = local_streams_.find(id);
      if (it != local_streams_.end()) {
        return (*it).second;
      }
    } else {
      auto pco = peerconnection_observers_.find(ownerTag);
      if (peerconnection_observers_.end() != pco) {
        auto stream = pco->second->MediaStreamForId(id);
        if (stream != nullptr) {
          return stream;
        }
      }
      // Fogata: a stream that connection received, never this device's own of the same
      // id (see fogata_track_owner.h).
      return nullptr;
    }
  }

  auto it = local_streams_.find(id);
  if (it != local_streams_.end()) {
    return (*it).second;
  }

  return nullptr;
}

void FlutterWebRTCBase::RemoveStreamForId(const std::string& id) {
  auto it = local_streams_.find(id);
  if (it != local_streams_.end())
    local_streams_.erase(it);
}

bool FlutterWebRTCBase::ParseConstraints(const EncodableMap& constraints,
                                         RTCConfiguration* configuration) {
  memset(&configuration->ice_servers, 0, sizeof(configuration->ice_servers));
  return false;
}

void FlutterWebRTCBase::ParseConstraints(
    const EncodableMap& src,
    scoped_refptr<RTCMediaConstraints> mediaConstraints,
    ParseConstraintType type /*= kMandatory*/) {
  for (auto kv : src) {
    EncodableValue k = kv.first;
    EncodableValue v = kv.second;
    std::string key = GetValue<std::string>(k);
    std::string value;
    if (TypeIs<EncodableList>(v) || TypeIs<EncodableMap>(v)) {
    } else if (TypeIs<std::string>(v)) {
      value = GetValue<std::string>(v);
    } else if (TypeIs<double>(v)) {
      value = std::to_string(GetValue<double>(v));
    } else if (TypeIs<int>(v)) {
      value = std::to_string(GetValue<int>(v));
    } else if (TypeIs<bool>(v)) {
      value = GetValue<bool>(v) ? RTCMediaConstraints::kValueTrue
                                : RTCMediaConstraints::kValueFalse;
    } else {
      continue;
    }
    if (type == kMandatory) {
      mediaConstraints->AddMandatoryConstraint(key.c_str(), value.c_str());
    } else {
      mediaConstraints->AddOptionalConstraint(key.c_str(), value.c_str());
      if (key == "DtlsSrtpKeyAgreement") {
        configuration_.srtp_type = GetValue<bool>(v)
                                       ? MediaSecurityType::kDTLS_SRTP
                                       : MediaSecurityType::kSDES_SRTP;
      }
    }
  }
}

scoped_refptr<RTCMediaConstraints> FlutterWebRTCBase::ParseMediaConstraints(
    const EncodableMap& constraints) {
  scoped_refptr<RTCMediaConstraints> media_constraints =
      RTCMediaConstraints::Create();

  if (constraints.find(EncodableValue("mandatory")) != constraints.end()) {
    auto it = constraints.find(EncodableValue("mandatory"));
    const EncodableMap mandatory = GetValue<EncodableMap>(it->second);
    ParseConstraints(mandatory, media_constraints, kMandatory);
  } else {
    // Log.d(TAG, "mandatory constraints are not a map");
  }

  auto it = constraints.find(EncodableValue("optional"));
  if (it != constraints.end()) {
    const EncodableValue optional = it->second;
    if (TypeIs<EncodableMap>(optional)) {
      ParseConstraints(GetValue<EncodableMap>(optional), media_constraints,
                       kOptional);
    } else if (TypeIs<EncodableList>(optional)) {
      const EncodableList list = GetValue<EncodableList>(optional);
      for (size_t i = 0; i < list.size(); i++) {
        ParseConstraints(GetValue<EncodableMap>(list[i]), media_constraints,
                         kOptional);
      }
    }
  } else {
    // Log.d(TAG, "optional constraints are not an array");
  }

  return media_constraints;
}

bool FlutterWebRTCBase::CreateIceServers(const EncodableList& iceServersArray,
                                         IceServer* ice_servers) {
  size_t size = iceServersArray.size();
  for (size_t i = 0; i < size; i++) {
    IceServer& ice_server = ice_servers[i];
    EncodableMap iceServerMap = GetValue<EncodableMap>(iceServersArray[i]);

    if (iceServerMap.find(EncodableValue("username")) != iceServerMap.end()) {
      ice_server.username = GetValue<std::string>(
          iceServerMap.find(EncodableValue("username"))->second);
    }
    if (iceServerMap.find(EncodableValue("credential")) != iceServerMap.end()) {
      ice_server.password = GetValue<std::string>(
          iceServerMap.find(EncodableValue("credential"))->second);
    }

    auto it = iceServerMap.find(EncodableValue("url"));
    if (it != iceServerMap.end() && TypeIs<std::string>(it->second)) {
      ice_server.uri = GetValue<std::string>(it->second);
    }
    it = iceServerMap.find(EncodableValue("urls"));
    if (it != iceServerMap.end()) {
      if (TypeIs<std::string>(it->second)) {
        ice_server.uri = GetValue<std::string>(it->second);
      }
      if (TypeIs<EncodableList>(it->second)) {
        const EncodableList urls = GetValue<EncodableList>(it->second);
        for (auto url : urls) {
          if (TypeIs<EncodableMap>(url)) {
            const EncodableMap map = GetValue<EncodableMap>(url);
            std::string value;
            auto it2 = map.find(EncodableValue("url"));
            if (it2 != map.end()) {
              ice_server.uri = GetValue<std::string>(it2->second);
            }
          } else if (TypeIs<std::string>(url)) {
            ice_server.uri = GetValue<std::string>(url);
          }
        }
      }
    }
  }
  return size > 0;
}

bool FlutterWebRTCBase::ParseRTCConfiguration(const EncodableMap& map,
                                              RTCConfiguration& conf) {
  // Enable SNAP (SCTP INIT in SDP), part of WARP.
  // see https://www.ietf.org/archive/id/draft-hancke-tsvwg-snap-00.html
  if (warp_enabled_) {
    conf.enable_sctp_snap = true;
  }

  auto it = map.find(EncodableValue("iceServers"));
  if (it != map.end()) {
    const EncodableList iceServersArray = GetValue<EncodableList>(it->second);
    CreateIceServers(iceServersArray, conf.ice_servers);
  }
  // iceTransportPolicy (public API)
  it = map.find(EncodableValue("iceTransportPolicy"));
  if (it != map.end() && TypeIs<std::string>(it->second)) {
    std::string v = GetValue<std::string>(it->second);
    if (v == "all")  // public
      conf.type = IceTransportsType::kAll;
    else if (v == "relay")
      conf.type = IceTransportsType::kRelay;
    else if (v == "nohost")
      conf.type = IceTransportsType::kNoHost;
    else if (v == "none")
      conf.type = IceTransportsType::kNone;
  }

  // bundlePolicy (public api)
  it = map.find(EncodableValue("bundlePolicy"));
  if (it != map.end() && TypeIs<std::string>(it->second)) {
    std::string v = GetValue<std::string>(it->second);
    if (v == "balanced")  // public
      conf.bundle_policy = kBundlePolicyBalanced;
    else if (v == "max-compat")  // public
      conf.bundle_policy = kBundlePolicyMaxCompat;
    else if (v == "max-bundle")  // public
      conf.bundle_policy = kBundlePolicyMaxBundle;
  }

  // rtcpMuxPolicy (public api)
  it = map.find(EncodableValue("rtcpMuxPolicy"));
  if (it != map.end() && TypeIs<std::string>(it->second)) {
    std::string v = GetValue<std::string>(it->second);
    if (v == "negotiate")  // public
      conf.rtcp_mux_policy = RtcpMuxPolicy::kRtcpMuxPolicyNegotiate;
    else if (v == "require")  // public
      conf.rtcp_mux_policy = RtcpMuxPolicy::kRtcpMuxPolicyRequire;
  }

  // FIXME: peerIdentity of type DOMString (public API)
  // FIXME: certificates of type sequence<RTCCertificate> (public API)
  // iceCandidatePoolSize of type unsigned short, defaulting to 0
  it = map.find(EncodableValue("iceCandidatePoolSize"));
  if (it != map.end()) {
    conf.ice_candidate_pool_size = GetValue<int>(it->second);
  }

  // sdpSemantics (public api)
  it = map.find(EncodableValue("sdpSemantics"));
  if (it != map.end() && TypeIs<std::string>(it->second)) {
    std::string v = GetValue<std::string>(it->second);
    if (v == "plan-b")  // public
      conf.sdp_semantics = SdpSemantics::kPlanB;
    else if (v == "unified-plan")  // public
      conf.sdp_semantics = SdpSemantics::kUnifiedPlan;
  } else {
    conf.sdp_semantics = SdpSemantics::kUnifiedPlan;
  }

  it = map.find(EncodableValue("enableDscp"));
  if (it != map.end() && TypeIs<bool>(it->second)) {
    conf.enable_dscp = GetValue<bool>(it->second);
  }

  // maxIPv6Networks
  it = map.find(EncodableValue("maxIPv6Networks"));
  if (it != map.end()) {
    conf.max_ipv6_networks = GetValue<int>(it->second);
  }
  return true;
}

scoped_refptr<RTCMediaTrack> FlutterWebRTCBase::MediaTracksForId(
    const std::string& id) {
  auto it = local_tracks_.find(id);
  if (it != local_tracks_.end()) {
    return (*it).second;
  }

  for (auto it2 : peerconnection_observers_) {
    auto pco = it2.second;
    auto t = pco->MediaTrackForId(id);
    if (t != nullptr) {
      return t;
    }
  }

  return nullptr;
}

void FlutterWebRTCBase::RemoveTracksForId(const std::string& id) {
  auto it = local_tracks_.find(id);
  if (it != local_tracks_.end())
    local_tracks_.erase(it);
}

libwebrtc::scoped_refptr<libwebrtc::RTCRtpSender>
FlutterWebRTCBase::GetRtpSenderById(RTCPeerConnection* pc, std::string id) {
  libwebrtc::scoped_refptr<libwebrtc::RTCRtpSender> result;
  auto senders = pc->senders();
  for (scoped_refptr<RTCRtpSender> item : senders.std_vector()) {
    std::string itemId = item->id().std_string();
    if (nullptr == result.get() && 0 == id.compare(itemId)) {
      result = item;
    }
  }
  return result;
}

libwebrtc::scoped_refptr<libwebrtc::RTCRtpReceiver>
FlutterWebRTCBase::GetRtpReceiverById(RTCPeerConnection* pc,
                                          std::string id) {
  libwebrtc::scoped_refptr<libwebrtc::RTCRtpReceiver> result;
  auto receivers = pc->receivers();
  for (scoped_refptr<RTCRtpReceiver> item : receivers.std_vector()) {
    std::string itemId = item->id().std_string();
    if (nullptr == result.get() && 0 == id.compare(itemId)) {
      result = item;
    }
  }
  return result;
}

}  // namespace flutter_webrtc_plugin
