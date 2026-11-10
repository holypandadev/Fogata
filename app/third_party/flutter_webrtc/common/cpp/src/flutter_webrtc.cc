#include "flutter_webrtc.h"
#include "flutter_data_channel.h"
#include "flutter_utf8_sanitize.h"
#include "task_runner.h"

#include "flutter_webrtc/flutter_web_r_t_c_plugin.h"
#ifdef __linux__
#include <future>
#include <thread>

#include "camera_access.h"
#include "desktop_session.h"
#include "input_name.h"
#include "input_watcher.h"
#include "output_keeper.h"
#include "silent_input.h"
#include "test_camera.h"
#ifdef HAVE_PIPEWIRE
#include "pipewire_apps_capturer.h"
#endif
#endif

namespace flutter_webrtc_plugin {

static EventChannelProxy* eventChannelProxy = nullptr;

FlutterWebRTC::FlutterWebRTC(FlutterWebRTCPlugin* plugin)
    : FlutterWebRTCBase::FlutterWebRTCBase(plugin->messenger(),
                                           plugin->textures(),
                                           plugin->task_runner()),
      FlutterVideoRendererManager::FlutterVideoRendererManager(this),
      FlutterMediaStream::FlutterMediaStream(this),
      FlutterPeerConnection::FlutterPeerConnection(this),
      FlutterScreenCapture::FlutterScreenCapture(this),
      FlutterDataChannel::FlutterDataChannel(this) {}

FlutterWebRTC::~FlutterWebRTC() {
  // A sound server check still running answers no more: its result's task
  // runner and channel go with the plugin.
  std::lock_guard<std::mutex> guard(answer_gate_->lock);
  answer_gate_->open = false;
}

void FlutterWebRTC::AskCameraAccess(std::function<void()> then) {
  std::shared_ptr<AnswerGate> gate = answer_gate_;
  TaskRunner* runner = task_runner_;
  auto next = std::make_shared<std::function<void()>>(std::move(then));
  // Told on the thread that learns the outcome (GLib's main loop or PipeWire's),
  // maybe at once: the rest runs later, from the platform thread's loop.
  video_device_->RequestCameraAccess(
      [gate, runner, next](RTCVideoDevice::CameraAccess) {
        std::lock_guard<std::mutex> guard(gate->lock);
        if (!gate->open) return;
        runner->EnqueueTask([gate, next] {
          {
            std::lock_guard<std::mutex> guard(gate->lock);
            if (!gate->open) return;
          }
          (*next)();
        });
      });
}

bool FlutterWebRTC::CameraToAskFor(const EncodableMap& constraints) {
  auto it = constraints.find(EncodableValue("video"));
  if (it == constraints.end() ||
      !((TypeIs<bool>(it->second) && GetValue<bool>(it->second)) ||
        TypeIs<EncodableMap>(it->second))) {
    return false;
  }
#ifdef __linux__
  // Tests with the made-up camera never ask.
  if (TestCamera::Wanted()) return false;
#endif
  switch (video_device_->GetCameraAccess()) {
    case RTCVideoDevice::kCameraAccessNotAsked:
    case RTCVideoDevice::kCameraAccessAsking:
    // Asked again: the person may have allowed it since, or the portal come.
    case RTCVideoDevice::kCameraAccessDenied:
    case RTCVideoDevice::kCameraAccessUnavailable:
      return true;
    default:
      return false;
  }
}

void FlutterWebRTC::HandleMethodCall(
    const MethodCallProxy& method_call,
    std::unique_ptr<MethodResultProxy> result) {
  if (method_call.method_name().compare("initialize") == 0) {
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    const EncodableMap options = findMap(params, "options");
    std::string severityStr = findString(options, "logSeverity");
    if (severityStr.empty() == false) {
      RTCLoggingSeverity severity = str2LogSeverity(severityStr);
      initLoggerCallback(severity);
    }
    // The field trials `enableWARP` and `zeroPlayoutDelay` turn on are read
    // when the peer connection factory is created, so the factory is built
    // here rather than in the constructor.
    EnsureWebRTCInitialized(findBoolean(options, "enableWARP"),
                            findBoolean(options, "zeroPlayoutDelay"));
    result->Success();
    return;
  }

  // Everything below needs the factory. If the Dart side never called
  // initialize() with options, fall back to the default field trials.
  EnsureWebRTCInitialized();

  if (method_call.method_name().compare("createPeerConnection") == 0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    const EncodableMap configuration = findMap(params, "configuration");
    const EncodableMap constraints = findMap(params, "constraints");
    CreateRTCPeerConnection(configuration, constraints, std::move(result));
  } else if (method_call.method_name().compare("getUserMedia") == 0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null constraints arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    const EncodableMap constraints = findMap(params, "constraints");
    if (CameraToAskFor(constraints)) {
      // Fogata: the system asks the person (once) before the cameras are listed.
      auto answer = std::make_shared<std::unique_ptr<MethodResultProxy>>(
          std::move(result));
      AskCameraAccess([this, constraints, answer]() {
        GetUserMedia(constraints, std::move(*answer));
      });
      return;
    }
    GetUserMedia(constraints, std::move(result));
#ifdef __linux__
  } else if (method_call.method_name().compare("fogataCameraAccess") == 0) {
    // Fogata: how the cameras are reached ("direct") or, through the Camera
    // portal, whether they were granted ("notAsked", "asking", "granted",
    // "denied", "unavailable"); with `request`, asked for first, which may show
    // the system's question.
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    const EncodableValue request = findEncodableValue(params, "request");
    std::shared_ptr<MethodResultProxy> answer(std::move(result));
    auto tell = [this, answer]() {
      answer->Success(EncodableValue(
          std::string(CameraAccessName(video_device_->GetCameraAccess()))));
    };
    if (TypeIs<bool>(request) && GetValue<bool>(request) &&
        video_device_->GetCameraAccess() !=
            RTCVideoDevice::kCameraAccessGranted) {
      AskCameraAccess(tell);
    } else {
      tell();
    }
#endif
  } else if (method_call.method_name().compare("getDisplayMedia") == 0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null constraints arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    const EncodableMap constraints = findMap(params, "constraints");

    GetDisplayMedia(constraints, std::move(result));
  } else if (method_call.method_name().compare("getDesktopSources") == 0) {
    // types: ["screen", "window"]
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Bad arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());

    const EncodableList types = findList(params, "types");
    if (types.empty()) {
      result->Error("Bad Arguments", "Types is required");
      return;
    }
    GetDesktopSources(types, std::move(result));
  } else if (method_call.method_name().compare("updateDesktopSources") == 0) {
    // types: ["screen", "window"]
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Bad arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());

    const EncodableList types = findList(params, "types");
    if (types.empty()) {
      result->Error("Bad Arguments", "Types is required");
      return;
    }
    UpdateDesktopSources(types, std::move(result));
  } else if (method_call.method_name().compare("getDesktopSourceThumbnail") ==
             0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Bad arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());

    std::string sourceId = findString(params, "sourceId");
    if (sourceId.empty()) {
      result->Error("Bad Arguments", "Incorrect sourceId");
      return;
    }
    const EncodableMap thumbnailSize = findMap(params, "thumbnailSize");
    if (!thumbnailSize.empty()) {
      int width = 0;
      int height = 0;
      GetDesktopSourceThumbnail(sourceId, width, height, std::move(result));
    } else {
      result->Error("Bad Arguments", "Bad arguments received");
    }
  } else if (method_call.method_name().compare("getSources") == 0) {
    GetSources(std::move(result));
  } else if (method_call.method_name().compare("selectAudioInput") == 0) {
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    const std::string deviceId = findString(params, "deviceId");
    SelectAudioInput(deviceId, std::move(result));
#ifdef __linux__
  } else if (method_call.method_name().compare("fogataWaylandSession") == 0) {
    // Fogata: whether GTK draws on a Wayland display, where the screen is shared through
    // the system's own dialog and libwebrtc is asked for no list.
    result->Success(EncodableValue(WaylandSession()));
#endif
  } else if (method_call.method_name().compare("fogataScreenFps") == 0) {
    // Fogata: how many frames a second a screen captured through the portal sends;
    // false for any other track.
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    auto rate = track_rates_.find(findString(params, "trackId"));
    const int fps = findInt(params, "fps");
    if (rate == track_rates_.end() || fps <= 0) {
      result->Success(EncodableValue(false));
    } else {
      rate->second(fps);
      result->Success(EncodableValue(true));
    }
  } else if (method_call.method_name().compare("fogataNoiseSuppression") == 0) {
    // Fogata: the microphone's noise suppression on or off (`on`), with the table of
    // functions it runs (`api`, needed the first time); false if it cannot run.
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    result->Success(EncodableValue(
        SuppressNoise(findBoolean(params, "on"), findLongInt(params, "api"))));
#ifdef __linux__
  } else if (method_call.method_name().compare("fogataSilentInput") == 0) {
    // Fogata: the "Fogata silence" input, made or removed. Off the platform
    // thread, as making it waits up to 2 s for PipeWire's answer and a stalled
    // server froze the window as a share started; one request after the other,
    // in the order asked, so an "off" is never overtaken by the "on" before it.
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    const EncodableValue on_value = findEncodableValue(params, "on");
    const bool on = TypeIs<bool>(on_value) && GetValue<bool>(on_value);
    // Only touched here, on the platform thread.
    static std::shared_future<void> silent_turn;
    std::shared_future<void> before = silent_turn;
    auto done = std::make_shared<std::promise<void>>();
    silent_turn = done->get_future().share();
    std::shared_ptr<MethodResultProxy> answer(std::move(result));
    std::shared_ptr<AnswerGate> gate = answer_gate_;
    std::thread([before, done, on, answer, gate]() {
      if (before.valid()) before.wait();
      bool made = true;
      if (on) {
        made = SilentInput::Start();
      } else {
        SilentInput::Stop();
      }
      done->set_value();
      // Only while the plugin is there, as for fogataSoundServer below.
      std::lock_guard<std::mutex> guard(gate->lock);
      if (gate->open) answer->Success(EncodableValue(made));
    }).detach();
  } else if (method_call.method_name().compare("fogataSoundServer") == 0) {
    // Fogata: whether a sound server answers ("ready"), there is none ("none"),
    // it did not answer in time ("timeout") or this cannot tell ("unknown"),
    // before a call starts the voice engine,
    // which ends the process without one. Checked on a thread of its own, as it
    // may wait 5 s; the result answers on the platform thread (its proxy posts
    // there).
    std::shared_ptr<MethodResultProxy> answer(std::move(result));
    std::shared_ptr<AnswerGate> gate = answer_gate_;
    std::thread([answer, gate]() {
      const char* word = "unknown";
      switch (InputWatcher::CheckServer(5)) {
        case InputWatcher::Server::kReady:
          word = "ready";
          break;
        case InputWatcher::Server::kNone:
          word = "none";
          break;
        case InputWatcher::Server::kTimeout:
          word = "timeout";
          break;
        case InputWatcher::Server::kUnknown:
          break;
      }
      // Only while the plugin is there; it waits on the lock while this queues
      // the answer.
      std::lock_guard<std::mutex> guard(gate->lock);
      if (gate->open) answer->Success(EncodableValue(std::string(word)));
    }).detach();
  } else if (method_call.method_name().compare("fogataScreenSoundServer") == 0) {
    // Fogata: whether a shared screen can take the other applications' sound
    // here: "pipewire" when this build has PipeWire and its server takes a
    // connection, "none" otherwise (a shared screen then goes without sound, see
    // loopback_capturer_factory.cc), so the application can say why. On a thread
    // of its own, as connecting may wait; answered as fogataSoundServer is.
    std::shared_ptr<MethodResultProxy> answer(std::move(result));
    std::shared_ptr<AnswerGate> gate = answer_gate_;
    std::thread([answer, gate]() {
#ifdef HAVE_PIPEWIRE
      const bool reachable = PipeWireAppsCapturer::Reachable();
#else
      const bool reachable = false;
#endif
      std::lock_guard<std::mutex> guard(gate->lock);
      if (gate->open) {
        answer->Success(EncodableValue(std::string(reachable ? "pipewire" : "none")));
      }
    }).detach();
  } else if (method_call.method_name().compare("fogataSoundsTo") == 0) {
    // Fogata: every sound of the application to the output `deviceId` (as
    // getSources names it), outside a call too; the first, the system's default,
    // or one not listed leaves them on the default.
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    const std::string deviceId = findString(params, "deviceId");
    char name[RTCAudioDevice::kAdmMaxDeviceNameSize + 1] = {0};
    char guid[RTCAudioDevice::kAdmMaxGuidSize + 1] = {0};
    std::string output;
    const int count = audio_device_->PlayoutDevices();
    for (int i = 1; i < count && !deviceId.empty() && output.empty(); i++) {
      name[0] = guid[0] = 0;
      if (audio_device_->PlayoutDeviceName(static_cast<uint16_t>(i), name, guid) != 0) {
        continue;
      }
      const std::string id = SanitizeUtf8ForFlutter(guid[0] != 0 ? guid : name);
      if (id == deviceId) output = name;
    }
    OutputKeeper::SendTo(output);
    result->Success(EncodableValue(true));
  } else if (method_call.method_name().compare("fogataInputName") == 0) {
    // Fogata: the sound server's name of the input `deviceId` (as getSources names it),
    // which the application opens itself to record a voice message; empty if unknown.
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    result->Success(EncodableValue(InputName(findString(params, "deviceId"))));
#endif
  } else if (method_call.method_name().compare("selectAudioOutput") == 0) {
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    const std::string deviceId = findString(params, "deviceId");
    SelectAudioOutput(deviceId, std::move(result));
  } else if (method_call.method_name().compare("mediaStreamGetTracks") == 0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null constraints arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    const std::string streamId = findString(params, "streamId");
    MediaStreamGetTracks(streamId, std::move(result));
  } else if (method_call.method_name().compare("createOffer") == 0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null constraints arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    const std::string peerConnectionId = findString(params, "peerConnectionId");
    const EncodableMap constraints = findMap(params, "constraints");
    RTCPeerConnection* pc = PeerConnectionForId(peerConnectionId);
    if (pc == nullptr) {
      result->Error("createOfferFailed",
                    "createOffer() peerConnection is null");
      return;
    }
    CreateOffer(constraints, pc, std::move(result));
  } else if (method_call.method_name().compare("createAnswer") == 0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null constraints arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    const std::string peerConnectionId = findString(params, "peerConnectionId");
    const EncodableMap constraints = findMap(params, "constraints");
    RTCPeerConnection* pc = PeerConnectionForId(peerConnectionId);
    if (pc == nullptr) {
      result->Error("createAnswerFailed",
                    "createAnswer() peerConnection is null");
      return;
    }
    CreateAnswer(constraints, pc, std::move(result));
  } else if (method_call.method_name().compare("addStream") == 0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    const std::string streamId = findString(params, "streamId");
    const std::string peerConnectionId = findString(params, "peerConnectionId");

    scoped_refptr<RTCMediaStream> stream = MediaStreamForId(streamId);
    if (!stream) {
      result->Error("addStreamFailed", "addStream() stream not found!");
      return;
    }
    RTCPeerConnection* pc = PeerConnectionForId(peerConnectionId);
    if (pc == nullptr) {
      result->Error("addStreamFailed", "addStream() peerConnection is null");
      return;
    }
    pc->AddStream(stream);
    result->Success();
  } else if (method_call.method_name().compare("removeStream") == 0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    const std::string streamId = findString(params, "streamId");
    const std::string peerConnectionId = findString(params, "peerConnectionId");

    scoped_refptr<RTCMediaStream> stream = MediaStreamForId(streamId);
    if (!stream) {
      result->Error("removeStreamFailed", "removeStream() stream not found!");
      return;
    }
    RTCPeerConnection* pc = PeerConnectionForId(peerConnectionId);
    if (pc == nullptr) {
      result->Error("removeStreamFailed",
                    "removeStream() peerConnection is null");
      return;
    }
    pc->RemoveStream(stream);
    result->Success();
  } else if (method_call.method_name().compare("setLocalDescription") == 0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null constraints arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    const std::string peerConnectionId = findString(params, "peerConnectionId");
    const EncodableMap constraints = findMap(params, "description");
    RTCPeerConnection* pc = PeerConnectionForId(peerConnectionId);
    if (pc == nullptr) {
      result->Error("setLocalDescriptionFailed",
                    "setLocalDescription() peerConnection is null");
      return;
    }

    SdpParseError error;
    scoped_refptr<RTCSessionDescription> description =
        RTCSessionDescription::Create(findString(constraints, "type").c_str(),
                                      findString(constraints, "sdp").c_str(),
                                      &error);

    if (description.get() != nullptr) {
      SetLocalDescription(description.get(), pc, std::move(result));
    } else {
      result->Error("setLocalDescriptionFailed", "Invalid type or sdp");
    }
  } else if (method_call.method_name().compare("setRemoteDescription") == 0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null constraints arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    const std::string peerConnectionId = findString(params, "peerConnectionId");
    const EncodableMap constraints = findMap(params, "description");
    RTCPeerConnection* pc = PeerConnectionForId(peerConnectionId);
    if (pc == nullptr) {
      result->Error("setRemoteDescriptionFailed",
                    "setRemoteDescription() peerConnection is null");
      return;
    }

    SdpParseError error;
    scoped_refptr<RTCSessionDescription> description =
        RTCSessionDescription::Create(findString(constraints, "type").c_str(),
                                      findString(constraints, "sdp").c_str(),
                                      &error);

    if (description.get() != nullptr) {
      SetRemoteDescription(description.get(), pc, std::move(result));
    } else {
      result->Error("setRemoteDescriptionFailed", "Invalid type or sdp");
    }
  } else if (method_call.method_name().compare("addCandidate") == 0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null constraints arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    const std::string peerConnectionId = findString(params, "peerConnectionId");
    const EncodableMap constraints = findMap(params, "candidate");
    RTCPeerConnection* pc = PeerConnectionForId(peerConnectionId);
    if (pc == nullptr) {
      result->Error("addCandidateFailed",
                    "addCandidate() peerConnection is null");
      return;
    }

    SdpParseError error;
    std::string candidate = findString(constraints, "candidate");
    if (candidate.empty()) {
      // received the end-of-candidates
      result->Success();
      return;
    }
    int sdpMLineIndex = findInt(constraints, "sdpMLineIndex");
    scoped_refptr<RTCIceCandidate> rtc_candidate = RTCIceCandidate::Create(
        candidate.c_str(), findString(constraints, "sdpMid").c_str(),
        sdpMLineIndex == -1 ? 0 : sdpMLineIndex, &error);

    if (rtc_candidate.get() != nullptr) {
      AddIceCandidate(rtc_candidate.get(), pc, std::move(result));
    } else {
      result->Error("addCandidateFailed", "Invalid candidate");
    }
  } else if (method_call.method_name().compare("getStats") == 0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null constraints arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    const std::string peerConnectionId = findString(params, "peerConnectionId");
    const std::string track_id = findString(params, "trackId");
    RTCPeerConnection* pc = PeerConnectionForId(peerConnectionId);
    if (pc == nullptr) {
      result->Error("getStatsFailed", "getStats() peerConnection is null");
      return;
    }
    GetStats(track_id, pc, std::move(result));
  } else if (method_call.method_name().compare("createDataChannel") == 0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null constraints arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    const std::string peerConnectionId = findString(params, "peerConnectionId");

    RTCPeerConnection* pc = PeerConnectionForId(peerConnectionId);
    if (pc == nullptr) {
      result->Error("createDataChannelFailed",
                    "createDataChannel() peerConnection is null");
      return;
    }

    const std::string label = findString(params, "label");
    const EncodableMap dataChannelDict = findMap(params, "dataChannelDict");

    CreateDataChannel(peerConnectionId, label, dataChannelDict, pc,
                      std::move(result));
  } else if (method_call.method_name().compare("dataChannelSend") == 0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null constraints arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    const std::string peerConnectionId = findString(params, "peerConnectionId");
    RTCPeerConnection* pc = PeerConnectionForId(peerConnectionId);
    if (pc == nullptr) {
      result->Error("dataChannelSendFailed",
                    "dataChannelSend() peerConnection is null");
      return;
    }

    const std::string dataChannelId = findString(params, "dataChannelId");
    const std::string type = findString(params, "type");
    const EncodableValue data = findEncodableValue(params, "data");
    RTCDataChannel* data_channel = DataChannelForId(dataChannelId);
    if (data_channel == nullptr) {
      result->Error("dataChannelSendFailed",
                    "dataChannelSend() data_channel is null");
      return;
    }
    DataChannelSend(data_channel, type, data, std::move(result));
  } else if (method_call.method_name().compare(
                 "dataChannelGetBufferedAmount") == 0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null constraints arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    const std::string peerConnectionId = findString(params, "peerConnectionId");
    RTCPeerConnection* pc = PeerConnectionForId(peerConnectionId);
    if (pc == nullptr) {
      result->Error("dataChannelGetBufferedAmountFailed",
                    "dataChannelGetBufferedAmount() peerConnection is null");
      return;
    }

    const std::string dataChannelId = findString(params, "dataChannelId");
    RTCDataChannel* data_channel = DataChannelForId(dataChannelId);
    if (data_channel == nullptr) {
      result->Error("dataChannelGetBufferedAmountFailed",
                    "dataChannelGetBufferedAmount() data_channel is null");
      return;
    }
    DataChannelGetBufferedAmount(data_channel, std::move(result));
  } else if (method_call.method_name().compare("dataChannelClose") == 0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null constraints arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    const std::string peerConnectionId = findString(params, "peerConnectionId");
    RTCPeerConnection* pc = PeerConnectionForId(peerConnectionId);
    if (pc == nullptr) {
      result->Error("dataChannelCloseFailed",
                    "dataChannelClose() peerConnection is null");
      return;
    }

    const std::string dataChannelId = findString(params, "dataChannelId");
    RTCDataChannel* data_channel = DataChannelForId(dataChannelId);
    if (data_channel == nullptr) {
      result->Error("dataChannelCloseFailed",
                    "dataChannelClose() data_channel is null");
      return;
    }
    DataChannelClose(data_channel, dataChannelId, std::move(result));
  } else if (method_call.method_name().compare("streamDispose") == 0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null constraints arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    const std::string stream_id = findString(params, "streamId");
    MediaStreamDispose(stream_id, std::move(result));
  } else if (method_call.method_name().compare("mediaStreamTrackSetEnable") ==
             0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null constraints arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    const std::string track_id = findString(params, "trackId");
    const EncodableValue enable = findEncodableValue(params, "enabled");
    RTCMediaTrack* track = MediaTrackForId(track_id);
    if (track != nullptr) {
      track->set_enabled(GetValue<bool>(enable));
    }
    result->Success();
  } else if (method_call.method_name().compare("trackDispose") == 0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null constraints arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    const std::string track_id = findString(params, "trackId");
    MediaStreamTrackDispose(track_id, std::move(result));
  } else if (method_call.method_name().compare("restartIce") == 0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null constraints arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    const std::string peerConnectionId = findString(params, "peerConnectionId");
    RTCPeerConnection* pc = PeerConnectionForId(peerConnectionId);
    if (pc == nullptr) {
      result->Error("restartIceFailed", "restartIce() peerConnection is null");
      return;
    }
    pc->RestartIce();
    result->Success();
  } else if (method_call.method_name().compare("peerConnectionClose") == 0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null constraints arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    const std::string peerConnectionId = findString(params, "peerConnectionId");
    RTCPeerConnection* pc = PeerConnectionForId(peerConnectionId);
    if (pc == nullptr) {
      result->Error("peerConnectionCloseFailed",
                    "peerConnectionClose() peerConnection is null");
      return;
    }
    RTCPeerConnectionClose(pc, peerConnectionId, std::move(result));
  } else if (method_call.method_name().compare("peerConnectionDispose") == 0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null constraints arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    const std::string peerConnectionId = findString(params, "peerConnectionId");
    RTCPeerConnection* pc = PeerConnectionForId(peerConnectionId);
    // Fogata: closed first, the connection is no longer listed (close lets go of
    // it), but its observer still is, holding it and its event channel: found
    // through the observer, both go now instead of staying until the app quits.
    if (pc == nullptr) {
      if (FlutterPeerConnectionObserver* observer =
              PeerConnectionObserversForId(peerConnectionId)) {
        pc = observer->peerconnection();
      }
    }
    if (pc == nullptr) {
      result->Success();
      return;
    }
    RTCPeerConnectionDispose(pc, peerConnectionId, std::move(result));
  } else if (method_call.method_name().compare("createVideoRenderer") == 0) {
    CreateVideoRendererTexture(std::move(result));
  } else if (method_call.method_name().compare("videoRendererDispose") == 0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null constraints arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    int64_t texture_id = findLongInt(params, "textureId");
    VideoRendererDispose(texture_id, std::move(result));
  } else if (method_call.method_name().compare("videoRendererSetSrcObject") ==
             0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null constraints arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    const std::string stream_id = findString(params, "streamId");
    int64_t texture_id = findLongInt(params, "textureId");
    const std::string owner_tag = findString(params, "ownerTag");
    const std::string track_id = findString(params, "trackId");

    VideoRendererSetSrcObject(texture_id, stream_id, owner_tag, track_id);
    result->Success();
  } else if (method_call.method_name().compare(
                 "mediaStreamTrackSwitchCamera") == 0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null constraints arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    const std::string track_id = findString(params, "trackId");
    MediaStreamTrackSwitchCamera(track_id, std::move(result));
  } else if (method_call.method_name().compare("setVolume") == 0) {
    auto args = method_call.arguments();
    if (!args) {
      result->Error("Bad Arguments", "setVolume() Null arguments received");
      return;
    }

    const EncodableMap params = GetValue<EncodableMap>(*args);
    const std::string trackId = findString(params, "trackId");
    const std::optional<double> volume = maybeFindDouble(params, "volume");

    if (trackId.empty()) {
      result->Error("Bad Arguments", "setVolume() Empty track provided");
      return;
    }

    if (!volume.has_value()) {
      result->Error("Bad Arguments", "setVolume() No volume provided");
      return;
    }

    if (volume.value() < 0) {
      result->Error("Bad Arguments", "setVolume() Volume must be positive");
      return;
    }

    RTCMediaTrack* track = MediaTrackForId(trackId);
    if (nullptr == track) {
      result->Error("setVolume", "setVolume() Unable to find provided track");
      return;
    }

    std::string kind = track->kind().std_string();
    if (0 != kind.compare("audio")) {
      result->Error("setVolume",
                    "setVolume() Only audio tracks can have volume set");
      return;
    }

    auto audioTrack = static_cast<RTCAudioTrack*>(track);
    audioTrack->SetVolume(volume.value());

    result->Success();
  } else if (method_call.method_name().compare("getLocalDescription") == 0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null constraints arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    const std::string peerConnectionId = findString(params, "peerConnectionId");
    const EncodableMap constraints = findMap(params, "description");
    RTCPeerConnection* pc = PeerConnectionForId(peerConnectionId);
    if (pc == nullptr) {
      result->Error("GetLocalDescription",
                    "GetLocalDescription() peerConnection is null");
      return;
    }

    GetLocalDescription(pc, std::move(result));
  } else if (method_call.method_name().compare("getRemoteDescription") == 0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null constraints arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    const std::string peerConnectionId = findString(params, "peerConnectionId");
    const EncodableMap constraints = findMap(params, "description");
    RTCPeerConnection* pc = PeerConnectionForId(peerConnectionId);
    if (pc == nullptr) {
      result->Error("GetRemoteDescription",
                    "GetRemoteDescription() peerConnection is null");
      return;
    }

    GetRemoteDescription(pc, std::move(result));
  } else if (method_call.method_name().compare("mediaStreamAddTrack") == 0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null constraints arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());

    const std::string streamId = findString(params, "streamId");
    const std::string trackId = findString(params, "trackId");

    scoped_refptr<RTCMediaStream> stream = MediaStreamForId(streamId);
    if (stream == nullptr) {
      result->Error("MediaStreamAddTrack",
                    "MediaStreamAddTrack() stream is null");
      return;
    }

    scoped_refptr<RTCMediaTrack> track = MediaTracksForId(trackId);
    if (track == nullptr) {
      result->Error("MediaStreamAddTrack",
                    "MediaStreamAddTrack() track is null");
      return;
    }

    MediaStreamAddTrack(stream, track, std::move(result));
    std::string kind = track->kind().std_string();
    for (int i = 0; i < renders_.size(); i++) {
      FlutterVideoRenderer* renderer = renders_.at(i).get();
      if (renderer->CheckMediaStream(streamId) && 0 == kind.compare("video")) {
        renderer->SetVideoTrack(static_cast<RTCVideoTrack*>(track.get()));
      }
    }
  } else if (method_call.method_name().compare("mediaStreamRemoveTrack") == 0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null constraints arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());

    const std::string streamId = findString(params, "streamId");
    const std::string trackId = findString(params, "trackId");

    scoped_refptr<RTCMediaStream> stream = MediaStreamForId(streamId);
    if (stream == nullptr) {
      result->Error("MediaStreamRemoveTrack",
                    "MediaStreamRemoveTrack() stream is null");
      return;
    }

    scoped_refptr<RTCMediaTrack> track = MediaTracksForId(trackId);
    if (track == nullptr) {
      result->Error("MediaStreamRemoveTrack",
                    "MediaStreamRemoveTrack() track is null");
      return;
    }

    MediaStreamRemoveTrack(stream, track, std::move(result));

    for (int i = 0; i < renders_.size(); i++) {
      FlutterVideoRenderer* renderer = renders_.at(i).get();
      if (renderer->CheckVideoTrack(streamId)) {
        renderer->SetVideoTrack(nullptr);
      }
    }
  } else if (method_call.method_name().compare("addTrack") == 0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null constraints arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());

    const std::string peerConnectionId = findString(params, "peerConnectionId");
    const std::string trackId = findString(params, "trackId");
    const EncodableList streamIds = findList(params, "streamIds");

    RTCPeerConnection* pc = PeerConnectionForId(peerConnectionId);
    if (pc == nullptr) {
      result->Error("AddTrack", "AddTrack() peerConnection is null");
      return;
    }

    scoped_refptr<RTCMediaTrack> track = MediaTracksForId(trackId);
    if (track == nullptr) {
      result->Error("AddTrack", "AddTrack() track is null");
      return;
    }
    std::vector<std::string> ids;
    for (EncodableValue value : streamIds) {
      ids.push_back(GetValue<std::string>(value));
    }

    AddTrack(pc, track, ids, std::move(result));

  } else if (method_call.method_name().compare("removeTrack") == 0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null constraints arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());

    const std::string peerConnectionId = findString(params, "peerConnectionId");
    const std::string senderId = findString(params, "senderId");

    RTCPeerConnection* pc = PeerConnectionForId(peerConnectionId);
    if (pc == nullptr) {
      result->Error("removeTrack", "removeTrack() peerConnection is null");
      return;
    }

    RemoveTrack(pc, senderId, std::move(result));

  } else if (method_call.method_name().compare("addTransceiver") == 0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null constraints arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    const std::string peerConnectionId = findString(params, "peerConnectionId");
    const EncodableMap transceiverInit = findMap(params, "transceiverInit");
    const std::string mediaType = findString(params, "mediaType");
    const std::string trackId = findString(params, "trackId");

    RTCPeerConnection* pc = PeerConnectionForId(peerConnectionId);
    if (pc == nullptr) {
      result->Error("addTransceiver",
                    "addTransceiver() peerConnection is null");
      return;
    }
    AddTransceiver(pc, trackId, mediaType, transceiverInit, std::move(result));
  } else if (method_call.method_name().compare("getTransceivers") == 0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null constraints arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    const std::string peerConnectionId = findString(params, "peerConnectionId");

    RTCPeerConnection* pc = PeerConnectionForId(peerConnectionId);
    if (pc == nullptr) {
      result->Error("getTransceivers",
                    "getTransceivers() peerConnection is null");
      return;
    }

    GetTransceivers(pc, std::move(result));
  } else if (method_call.method_name().compare("getReceivers") == 0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null constraints arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    const std::string peerConnectionId = findString(params, "peerConnectionId");

    RTCPeerConnection* pc = PeerConnectionForId(peerConnectionId);
    if (pc == nullptr) {
      result->Error("getReceivers", "getReceivers() peerConnection is null");
      return;
    }

    GetReceivers(pc, std::move(result));

  } else if (method_call.method_name().compare("getSenders") == 0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null constraints arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    const std::string peerConnectionId = findString(params, "peerConnectionId");

    RTCPeerConnection* pc = PeerConnectionForId(peerConnectionId);
    if (pc == nullptr) {
      result->Error("getSenders", "getSenders() peerConnection is null");
      return;
    }

    GetSenders(pc, std::move(result));
  } else if (method_call.method_name().compare("rtpSenderSetTrack") == 0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null constraints arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    const std::string peerConnectionId = findString(params, "peerConnectionId");

    RTCPeerConnection* pc = PeerConnectionForId(peerConnectionId);
    if (pc == nullptr) {
      result->Error("rtpSenderSetTrack",
                    "rtpSenderSetTrack() peerConnection is null");
      return;
    }

    const std::string trackId = findString(params, "trackId");
    RTCMediaTrack* track = MediaTrackForId(trackId);

    const std::string rtpSenderId = findString(params, "rtpSenderId");
    if (rtpSenderId.empty()) {
      result->Error("rtpSenderSetTrack",
                    "rtpSenderSetTrack() rtpSenderId is null or empty");
      return;
    }
    RtpSenderSetTrack(pc, track, rtpSenderId, std::move(result));
  } else if (method_call.method_name().compare("rtpSenderSetStreams") == 0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null constraints arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    const std::string peerConnectionId = findString(params, "peerConnectionId");

    RTCPeerConnection* pc = PeerConnectionForId(peerConnectionId);
    if (pc == nullptr) {
      result->Error("rtpSenderSetStream",
                    "rtpSenderSetStream() peerConnection is null");
      return;
    }

    const EncodableList encodableStreamIds = findList(params, "streamIds");
    if (encodableStreamIds.empty()) {
      result->Error("rtpSenderSetStream",
                    "rtpSenderSetStream() streamId is null or empty");
      return;
    }
    std::vector<std::string> streamIds{};
    for (EncodableValue value : encodableStreamIds) {
      streamIds.push_back(GetValue<std::string>(value));
    }

    const std::string rtpSenderId = findString(params, "rtpSenderId");
    if (rtpSenderId.empty()) {
      result->Error("rtpSenderSetStream",
                    "rtpSenderSetStream() rtpSenderId is null or empty");
      return;
    }
    RtpSenderSetStream(pc, streamIds, rtpSenderId, std::move(result));
  } else if (method_call.method_name().compare("rtpSenderReplaceTrack") == 0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null constraints arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    const std::string peerConnectionId = findString(params, "peerConnectionId");

    RTCPeerConnection* pc = PeerConnectionForId(peerConnectionId);
    if (pc == nullptr) {
      result->Error("rtpSenderReplaceTrack",
                    "rtpSenderReplaceTrack() peerConnection is null");
      return;
    }

    const std::string trackId = findString(params, "trackId");
    RTCMediaTrack* track = MediaTrackForId(trackId);

    const std::string rtpSenderId = findString(params, "rtpSenderId");
    if (rtpSenderId.empty()) {
      result->Error("rtpSenderReplaceTrack",
                    "rtpSenderReplaceTrack() rtpSenderId is null or empty");
      return;
    }
    RtpSenderReplaceTrack(pc, track, rtpSenderId, std::move(result));
  } else if (method_call.method_name().compare("rtpSenderSetParameters") == 0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null constraints arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    const std::string peerConnectionId = findString(params, "peerConnectionId");

    RTCPeerConnection* pc = PeerConnectionForId(peerConnectionId);
    if (pc == nullptr) {
      result->Error("rtpSenderSetParameters",
                    "rtpSenderSetParameters() peerConnection is null");
      return;
    }

    const std::string rtpSenderId = findString(params, "rtpSenderId");
    if (rtpSenderId.empty()) {
      result->Error("rtpSenderSetParameters",
                    "rtpSenderSetParameters() rtpSenderId is null or empty");
      return;
    }

    const EncodableMap parameters = findMap(params, "parameters");
    if (0 == parameters.size()) {
      result->Error("rtpSenderSetParameters",
                    "rtpSenderSetParameters() parameters is null or empty");
      return;
    }

    RtpSenderSetParameters(pc, rtpSenderId, parameters, std::move(result));
  } else if (method_call.method_name().compare("rtpTransceiverStop") == 0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null constraints arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    const std::string peerConnectionId = findString(params, "peerConnectionId");

    RTCPeerConnection* pc = PeerConnectionForId(peerConnectionId);
    if (pc == nullptr) {
      result->Error("rtpTransceiverStop",
                    "rtpTransceiverStop() peerConnection is null");
      return;
    }

    const std::string transceiverId = findString(params, "transceiverId");
    if (transceiverId.empty()) {
      result->Error("rtpTransceiverStop",
                    "rtpTransceiverStop() transceiverId is null or empty");
      return;
    }

    RtpTransceiverStop(pc, transceiverId, std::move(result));
  } else if (method_call.method_name().compare(
                 "rtpTransceiverGetCurrentDirection") == 0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null constraints arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    const std::string peerConnectionId = findString(params, "peerConnectionId");

    RTCPeerConnection* pc = PeerConnectionForId(peerConnectionId);
    if (pc == nullptr) {
      result->Error(
          "rtpTransceiverGetCurrentDirection",
          "rtpTransceiverGetCurrentDirection() peerConnection is null");
      return;
    }

    const std::string transceiverId = findString(params, "transceiverId");
    if (transceiverId.empty()) {
      result->Error("rtpTransceiverGetCurrentDirection",
                    "rtpTransceiverGetCurrentDirection() transceiverId is "
                    "null or empty");
      return;
    }

    RtpTransceiverGetCurrentDirection(pc, transceiverId, std::move(result));
  } else if (method_call.method_name().compare("rtpTransceiverSetDirection") ==
             0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null constraints arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    const std::string peerConnectionId = findString(params, "peerConnectionId");

    RTCPeerConnection* pc = PeerConnectionForId(peerConnectionId);
    if (pc == nullptr) {
      result->Error("rtpTransceiverSetDirection",
                    "rtpTransceiverSetDirection() peerConnection is null");
      return;
    }

    const std::string transceiverId = findString(params, "transceiverId");
    if (transceiverId.empty()) {
      result->Error("rtpTransceiverSetDirection",
                    "rtpTransceiverSetDirection() transceiverId is "
                    "null or empty");
      return;
    }

    const std::string direction = findString(params, "direction");
    if (transceiverId.empty()) {
      result->Error("rtpTransceiverSetDirection",
                    "rtpTransceiverSetDirection() direction is null or empty");
      return;
    }

    RtpTransceiverSetDirection(pc, transceiverId, direction, std::move(result));
  } else if (method_call.method_name().compare("setConfiguration") == 0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null constraints arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    const std::string peerConnectionId = findString(params, "peerConnectionId");

    RTCPeerConnection* pc = PeerConnectionForId(peerConnectionId);
    if (pc == nullptr) {
      result->Error("setConfiguration",
                    "setConfiguration() peerConnection is null");
      return;
    }

    const EncodableMap configuration = findMap(params, "configuration");
    if (configuration.empty()) {
      result->Error("setConfiguration",
                    "setConfiguration() configuration is null or empty");
      return;
    }
    SetConfiguration(pc, configuration, std::move(result));
  } else if (method_call.method_name().compare("captureFrame") == 0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null constraints arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());

    const std::string path = findString(params, "path");
    if (path.empty()) {
      result->Error("captureFrame", "captureFrame() path is null or empty");
      return;
    }

    const std::string trackId = findString(params, "trackId");
    RTCMediaTrack* track = MediaTrackForId(trackId);
    if (nullptr == track) {
      result->Error("captureFrame", "captureFrame() track is null");
      return;
    }
    std::string kind = track->kind().std_string();
    if (0 != kind.compare("video")) {
      result->Error("captureFrame", "captureFrame() track not is video track");
      return;
    }
    CaptureFrame(reinterpret_cast<RTCVideoTrack*>(track), path,
                 std::move(result));

  } else if (method_call.method_name().compare("createLocalMediaStream") == 0) {
    CreateLocalMediaStream(std::move(result));
  } else if (method_call.method_name().compare("canInsertDtmf") == 0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null constraints arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    const std::string peerConnectionId = findString(params, "peerConnectionId");
    const std::string rtpSenderId = findString(params, "rtpSenderId");

    RTCPeerConnection* pc = PeerConnectionForId(peerConnectionId);
    if (pc == nullptr) {
      result->Error("canInsertDtmf", "canInsertDtmf() peerConnection is null");
      return;
    }

    auto rtpSender = GetRtpSenderById(pc, rtpSenderId);

    if (rtpSender == nullptr) {
      result->Error("sendDtmf", "sendDtmf() rtpSender is null");
      return;
    }
    auto dtmfSender = rtpSender->dtmf_sender();
    bool canInsertDtmf = dtmfSender->CanInsertDtmf();

    result->Success(EncodableValue(canInsertDtmf));
  } else if (method_call.method_name().compare("sendDtmf") == 0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null constraints arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    const std::string peerConnectionId = findString(params, "peerConnectionId");
    const std::string rtpSenderId = findString(params, "rtpSenderId");
    const std::string tone = findString(params, "tone");
    int duration = findInt(params, "duration");
    int gap = findInt(params, "gap");

    RTCPeerConnection* pc = PeerConnectionForId(peerConnectionId);
    if (pc == nullptr) {
      result->Error("sendDtmf", "sendDtmf() peerConnection is null");
      return;
    }

    auto rtpSender = GetRtpSenderById(pc, rtpSenderId);

    if (rtpSender == nullptr) {
      result->Error("sendDtmf", "sendDtmf() rtpSender is null");
      return;
    }

    auto dtmfSender = rtpSender->dtmf_sender();
    dtmfSender->InsertDtmf(tone, duration, gap);

    result->Success();
  } else if (method_call.method_name().compare("getRtpSenderCapabilities") ==
             0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());

    RTCMediaType mediaType = RTCMediaType::AUDIO;
    const std::string kind = findString(params, "kind");
    if (0 == kind.compare("video")) {
      mediaType = RTCMediaType::VIDEO;
    } else if (0 == kind.compare("audio")) {
      mediaType = RTCMediaType::AUDIO;
    } else {
      result->Error("getRtpSenderCapabilities",
                    "getRtpSenderCapabilities() kind is null or empty");
      return;
    }
    auto capabilities = factory_->GetRtpSenderCapabilities(mediaType);
    EncodableMap map;
    EncodableList codecsList;
    for (auto codec : capabilities->codecs().std_vector()) {
      EncodableMap codecMap;
      codecMap[EncodableValue("mimeType")] =
          EncodableValue(codec->mime_type().std_string());
      codecMap[EncodableValue("clockRate")] =
          EncodableValue(codec->clock_rate());
      codecMap[EncodableValue("channels")] = EncodableValue(codec->channels());
      codecMap[EncodableValue("sdpFmtpLine")] =
          EncodableValue(codec->sdp_fmtp_line().std_string());
      codecsList.push_back(EncodableValue(codecMap));
    }
    map[EncodableValue("codecs")] = EncodableValue(codecsList);
    map[EncodableValue("headerExtensions")] = EncodableValue(EncodableList());
    map[EncodableValue("fecMechanisms")] = EncodableValue(EncodableList());

    result->Success(EncodableValue(map));
  } else if (method_call.method_name().compare("getRtpReceiverCapabilities") ==
             0) {
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());

    RTCMediaType mediaType = RTCMediaType::AUDIO;
    const std::string kind = findString(params, "kind");
    if (0 == kind.compare("video")) {
      mediaType = RTCMediaType::VIDEO;
    } else if (0 == kind.compare("audio")) {
      mediaType = RTCMediaType::AUDIO;
    } else {
      result->Error("getRtpSenderCapabilities",
                    "getRtpSenderCapabilities() kind is null or empty");
      return;
    }
    auto capabilities = factory_->GetRtpReceiverCapabilities(mediaType);
    EncodableMap map;
    EncodableList codecsList;
    for (auto codec : capabilities->codecs().std_vector()) {
      EncodableMap codecMap;
      codecMap[EncodableValue("mimeType")] =
          EncodableValue(codec->mime_type().std_string());
      codecMap[EncodableValue("clockRate")] =
          EncodableValue(codec->clock_rate());
      codecMap[EncodableValue("channels")] = EncodableValue(codec->channels());
      codecMap[EncodableValue("sdpFmtpLine")] =
          EncodableValue(codec->sdp_fmtp_line().std_string());
      codecsList.push_back(EncodableValue(codecMap));
    }
    map[EncodableValue("codecs")] = EncodableValue(codecsList);
    map[EncodableValue("headerExtensions")] = EncodableValue(EncodableList());
    map[EncodableValue("fecMechanisms")] = EncodableValue(EncodableList());

    result->Success(EncodableValue(map));
  } else if (method_call.method_name().compare("setCodecPreferences") == 0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    const std::string peerConnectionId = findString(params, "peerConnectionId");
    RTCPeerConnection* pc = PeerConnectionForId(peerConnectionId);
    if (pc == nullptr) {
      result->Error("setCodecPreferences",
                    "setCodecPreferences() peerConnection is null");
      return;
    }

    const std::string transceiverId = findString(params, "transceiverId");
    if (transceiverId.empty()) {
      result->Error("setCodecPreferences",
                    "setCodecPreferences() transceiverId is null or empty");
      return;
    }

    const EncodableList codecs = findList(params, "codecs");
    if (codecs.empty()) {
      result->Error("Bad Arguments", "Codecs is required");
      return;
    }
    RtpTransceiverSetCodecPreferences(pc, transceiverId, codecs,
                                      std::move(result));
  } else if (method_call.method_name().compare("getSignalingState") == 0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null constraints arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());

    const std::string peerConnectionId = findString(params, "peerConnectionId");

    RTCPeerConnection* pc = PeerConnectionForId(peerConnectionId);
    if (pc == nullptr) {
      result->Error("getSignalingState",
                    "getSignalingState() peerConnection is null");
      return;
    }
    EncodableMap state;
    state[EncodableValue("state")] =
        signalingStateString(pc->signaling_state());
    result->Success(EncodableValue(state));
  } else if (method_call.method_name().compare("getIceGatheringState") == 0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null constraints arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());

    const std::string peerConnectionId = findString(params, "peerConnectionId");

    RTCPeerConnection* pc = PeerConnectionForId(peerConnectionId);
    if (pc == nullptr) {
      result->Error("getIceGatheringState",
                    "getIceGatheringState() peerConnection is null");
      return;
    }
    EncodableMap state;
    state[EncodableValue("state")] =
        iceGatheringStateString(pc->ice_gathering_state());
    result->Success(EncodableValue(state));
  } else if (method_call.method_name().compare("getIceConnectionState") == 0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null constraints arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());

    const std::string peerConnectionId = findString(params, "peerConnectionId");

    RTCPeerConnection* pc = PeerConnectionForId(peerConnectionId);
    if (pc == nullptr) {
      result->Error("getIceConnectionState",
                    "getIceConnectionState() peerConnection is null");
      return;
    }
    EncodableMap state;
    state[EncodableValue("state")] =
        iceConnectionStateString(pc->ice_connection_state());
    result->Success(EncodableValue(state));
  } else if (method_call.method_name().compare("getConnectionState") == 0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Null constraints arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());

    const std::string peerConnectionId = findString(params, "peerConnectionId");

    RTCPeerConnection* pc = PeerConnectionForId(peerConnectionId);
    if (pc == nullptr) {
      result->Error("getConnectionState",
                    "getConnectionState() peerConnection is null");
      return;
    }
    EncodableMap state;
    state[EncodableValue("state")] =
        peerConnectionStateString(pc->peer_connection_state());
    result->Success(EncodableValue(state));
  } else if (method_call.method_name().compare("setLogSeverity") == 0) {
    if (!method_call.arguments()) {
      result->Error("Bad Arguments", "Bad arguments received");
      return;
    }
    const EncodableMap params =
        GetValue<EncodableMap>(*method_call.arguments());
    std::string severityStr = findString(params, "severity");
    if (severityStr.empty() == false) {
      RTCLoggingSeverity severity = str2LogSeverity(severityStr);
      initLoggerCallback(severity);
    }
  } else {
    result->NotImplemented();
  }
}

void FlutterWebRTC::initLoggerCallback(RTCLoggingSeverity severity) {
  if (eventChannelProxy == nullptr) {
    eventChannelProxy = event_channel();
  }

  libwebrtc::LibWebRTCLogging::setLogSink(severity, [](const string& message) {
    EncodableMap info;
    info[EncodableValue("event")] = "onLogData";
    info[EncodableValue("data")] = message.c_string();
    eventChannelProxy->Success(EncodableValue(info), false);
  });
}

RTCLoggingSeverity FlutterWebRTC::str2LogSeverity(std::string str) {
  if (str == "verbose")
    return Verbose;
  else if (str == "info")
    return Info;
  else if (str == "warning")
    return Warning;
  else if (str == "error")
    return Error;
  else if (str == "none")
    return None;

  return None;
}

}  // namespace flutter_webrtc_plugin
