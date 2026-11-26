#include "flutter_media_stream.h"

#include "flutter_utf8_sanitize.h"
#include "fogata_default_device.h"
#include "fogata_track_owner.h"
#ifdef __linux__
#include <fcntl.h>
#include <linux/videodev2.h>
#include <sys/ioctl.h>
#include <unistd.h>

#include <cerrno>
#include <cstring>

#include "fogata_portal_camera.h"
#include "output_keeper.h"
#include "test_camera.h"
#endif

#define DEFAULT_WIDTH 1280
#define DEFAULT_HEIGHT 720
#define DEFAULT_FPS 30

namespace flutter_webrtc_plugin {

namespace {

#ifdef __linux__
// Fogata: a V4L2 string field of `size` bytes, which a driver may fill to the end.
static std::string Field(const __u8* field, size_t size) {
  const char* text = reinterpret_cast<const char*>(field);
  return std::string(text, strnlen(text, size));
}

// Fogata: whether the node `cap` describes captures video and is the camera libwebrtc
// listed as `name` with `unique_id` (its V4L2 bus_info, or empty where the driver gives
// none: then the card name is the id, as libwebrtc takes it).
static bool SameCamera(const v4l2_capability& cap, const std::string& unique_id,
                       const std::string& name) {
  const __u32 caps = (cap.capabilities & V4L2_CAP_DEVICE_CAPS) ? cap.device_caps
                                                                : cap.capabilities;
  if (!(caps & V4L2_CAP_VIDEO_CAPTURE)) return false;
  return !unique_id.empty()
             ? unique_id == Field(cap.bus_info, sizeof(cap.bus_info))
             : name == Field(cap.card, sizeof(cap.card));
}

// Fogata: whether another process holds the camera libwebrtc listed as `name` with
// `unique_id`. A camera streamed by another process refuses a second opener's
// VIDIOC_S_FMT with EBUSY (or the open itself), which libwebrtc reports only as a
// failure. Every capture node of that camera is looked at (one USB camera can have
// several, an infrared one beside it). Looks only, on this failure's path: the format
// set is the one already there; opening a node may wake its device. `why` says what
// answered, for the day's log.
static bool CameraBusy(const std::string& unique_id, const std::string& name,
                       std::string* why) {
  if (unique_id.empty() && name.empty()) return false;
  for (int n = 0; n < 64; n++) {
    const std::string path = "/dev/video" + std::to_string(n);
    v4l2_capability cap;
    memset(&cap, 0, sizeof(cap));
    const int fd = open(path.c_str(), O_RDWR | O_NONBLOCK, 0);
    if (fd < 0) {
      // Held so that it does not open: told only if it is that camera, read-only.
      if (errno != EBUSY) continue;
      const int info = open(path.c_str(), O_RDONLY | O_NONBLOCK, 0);
      if (info < 0) continue;
      const bool same = ioctl(info, VIDIOC_QUERYCAP, &cap) == 0 &&
                        SameCamera(cap, unique_id, name);
      close(info);
      if (same) {
        *why = path + ": open: Device or resource busy";
        return true;
      }
      continue;
    }
    bool busy = false;
    if (ioctl(fd, VIDIOC_QUERYCAP, &cap) == 0 && SameCamera(cap, unique_id, name)) {
      v4l2_format format;
      memset(&format, 0, sizeof(format));
      format.type = V4L2_BUF_TYPE_VIDEO_CAPTURE;
      if (ioctl(fd, VIDIOC_G_FMT, &format) == 0 &&
          ioctl(fd, VIDIOC_S_FMT, &format) < 0 && errno == EBUSY) {
        *why = path + ": VIDIOC_S_FMT: Device or resource busy";
        busy = true;
      }
    }
    close(fd);
    if (busy) return true;
  }
  return false;
}

// Fogata: how long a camera reached through the Camera portal is waited for, for its
// first picture or PipeWire's failure (fogata_portal_camera.h). A failure comes within
// milliseconds and a working camera's first picture mostly within a second; the wait
// blocks the platform thread, so it stops here and the camera is kept as before (a slow
// camera, waking from USB suspend, is not refused).
constexpr int kPortalCameraStartMs = 1000;

// Fogata: the error for a camera that did not open or start: another application's
// hold on it when it can be told (cameras reached directly only), the camera asked for
// first, then the one tried last; else `plain`.
static std::string CameraOpenError(bool direct, const std::string& asked_name,
                                   const std::string& asked_guid,
                                   const char* tried_guid, const char* tried_name,
                                   const char* plain) {
  std::string why;
  if (direct && (CameraBusy(asked_guid, asked_name, &why) ||
                 CameraBusy(tried_guid, tried_name, &why))) {
    return "NotReadableError: another application is using the camera "
           "(fogataCameraBusy): " +
           why;
  }
  return plain;
}
#endif

std::string SanitizeDeviceIdFromAudioBuffers(const char* name, const char* guid) {
  const std::string raw = (guid != nullptr && strlen(guid) > 0)
                              ? std::string(guid)
                              : std::string(name != nullptr ? name : "");
  return SanitizeUtf8ForFlutter(raw);
}

std::string SanitizeLabel(const char* name) {
  return SanitizeUtf8ForFlutter(std::string(name != nullptr ? name : ""));
}

std::string SanitizeDeviceIdFromVideoBuffers(const char* name, const char* guid) {
  const std::string raw = (guid != nullptr && strlen(guid) > 0)
                              ? std::string(guid)
                              : std::string(name != nullptr ? name : "");
  return SanitizeUtf8ForFlutter(raw);
}

// Fogata: the place of the system's default input (`capture`) or output in libwebrtc's
// list of `count` devices; 0, its first, where that is the default or it cannot be told
// (fogata_default_device.h: on Windows the list has no default first).
uint16_t DefaultAudioPlace(RTCAudioDevice* audio, bool capture, int count) {
  const std::string wanted = DefaultAudioEndpoint(capture);
  if (wanted.empty() || count <= 0)
    return 0;
  std::vector<std::string> guids;
  char name[RTCAudioDevice::kAdmMaxDeviceNameSize + 1] = {0};
  char guid[RTCAudioDevice::kAdmMaxGuidSize + 1] = {0};
  for (int i = 0; i < count; i++) {
    name[0] = guid[0] = 0;
    if (capture)
      audio->RecordingDeviceName(static_cast<uint16_t>(i), name, guid);
    else
      audio->PlayoutDeviceName(static_cast<uint16_t>(i), name, guid);
    guids.push_back(std::string(guid));
  }
  return DefaultDevicePlace(guids, wanted);
}

}  // namespace

FlutterMediaStream::FlutterMediaStream(FlutterWebRTCBase* base) : base_(base) {
}

void FlutterMediaStream::GetUserMedia(
    const EncodableMap& constraints,
    std::unique_ptr<MethodResultProxy> result) {
  std::string uuid = base_->GenerateUUID();
  scoped_refptr<RTCMediaStream> stream =
      base_->factory_->CreateStream(uuid.c_str());

  EncodableMap params;
  params[EncodableValue("streamId")] = EncodableValue(uuid);

  // Fogata: the camera first, so that a camera that cannot be used ends the
  // request before a microphone is opened for nothing.
  auto it = constraints.find(EncodableValue("video"));
  params[EncodableValue("videoTracks")] = EncodableValue(EncodableList());
  if (it != constraints.end()) {
    EncodableValue video = it->second;
    if ((TypeIs<bool>(video) && GetValue<bool>(video)) ||
        TypeIs<EncodableMap>(video)) {
      std::string error;
      if (!GetUserVideo(constraints, stream, params, &error)) {
        // As a browser names it, before the colon.
        result->Error(error.substr(0, error.find(':')), error);
        return;
      }
    }
  }

  it = constraints.find(EncodableValue("audio"));
  if (it != constraints.end()) {
    EncodableValue audio = it->second;
    if (TypeIs<bool>(audio)) {
      if (true == GetValue<bool>(audio)) {
        GetUserAudio(constraints, stream, params);
      }
    } else if (TypeIs<EncodableMap>(audio)) {
      GetUserAudio(constraints, stream, params);
    } else {
      params[EncodableValue("audioTracks")] = EncodableValue(EncodableList());
    }
  } else {
    params[EncodableValue("audioTracks")] = EncodableValue(EncodableList());
  }

  base_->local_streams_[uuid] = stream;
  result->Success(EncodableValue(params));
}

void addDefaultAudioConstraints(
    scoped_refptr<RTCMediaConstraints> audioConstraints) {
  audioConstraints->AddOptionalConstraint("googNoiseSuppression", "true");
  audioConstraints->AddOptionalConstraint("googEchoCancellation", "true");
  audioConstraints->AddOptionalConstraint("echoCancellation", "true");
  audioConstraints->AddOptionalConstraint("googEchoCancellation2", "true");
  audioConstraints->AddOptionalConstraint("googDAEchoCancellation", "true");
}

// Reads a boolean audio-processing flag from the audio constraint map.
// Supports flat W3C keys, mandatory sub-map keys, and optional list entries
// (each a single-pair map, the format used by the LiveKit SDK and others).
// Accepts bool values or "true"/"false" strings. Falls back to defaultValue
// when the key is absent (W3C default is true for AEC/NS/AGC).
static bool getAudioProcessingFlag(const EncodableMap& audioMap,
                                   const std::vector<std::string>& keys,
                                   bool defaultValue) {
  auto readBoolValue = [](const EncodableValue& v, bool def) -> bool {
    if (TypeIs<bool>(v)) return GetValue<bool>(v);
    if (TypeIs<std::string>(v)) {
      const std::string& s = GetValue<std::string>(v);
      if (s == "true") return true;
      if (s == "false") return false;
    }
    return def;
  };

  for (const std::string& key : keys) {
    // Flat W3C key at the top level.
    auto it = audioMap.find(EncodableValue(key));
    if (it != audioMap.end()) {
      return readBoolValue(it->second, defaultValue);
    }

    // Inside "mandatory" sub-map.
    auto mandatoryIt = audioMap.find(EncodableValue("mandatory"));
    if (mandatoryIt != audioMap.end() &&
        TypeIs<EncodableMap>(mandatoryIt->second)) {
      const EncodableMap& mandatory =
          GetValue<EncodableMap>(mandatoryIt->second);
      auto mit = mandatory.find(EncodableValue(key));
      if (mit != mandatory.end()) {
        return readBoolValue(mit->second, defaultValue);
      }
    }

    // Inside "optional" list — each entry is a single-pair map.
    auto optionalIt = audioMap.find(EncodableValue("optional"));
    if (optionalIt != audioMap.end() &&
        TypeIs<EncodableList>(optionalIt->second)) {
      const EncodableList& list = GetValue<EncodableList>(optionalIt->second);
      for (const EncodableValue& item : list) {
        if (!TypeIs<EncodableMap>(item)) continue;
        const EncodableMap& entry = GetValue<EncodableMap>(item);
        auto eit = entry.find(EncodableValue(key));
        if (eit != entry.end()) {
          return readBoolValue(eit->second, defaultValue);
        }
      }
    }
  }
  return defaultValue;
}

std::string getSourceIdConstraint(const EncodableMap& mediaConstraints) {
  auto it = mediaConstraints.find(EncodableValue("optional"));
  if (it != mediaConstraints.end() && TypeIs<EncodableList>(it->second)) {
    EncodableList optional = GetValue<EncodableList>(it->second);
    for (size_t i = 0, size = optional.size(); i < size; i++) {
      if (TypeIs<EncodableMap>(optional[i])) {
        EncodableMap option = GetValue<EncodableMap>(optional[i]);
        auto it2 = option.find(EncodableValue("sourceId"));
        if (it2 != option.end() && TypeIs<std::string>(it2->second)) {
          return GetValue<std::string>(it2->second);
        }
      }
    }
  }
  return "";
}

std::string getDeviceIdConstraint(const EncodableMap& mediaConstraints) {
  auto it = mediaConstraints.find(EncodableValue("deviceId"));
  if (it != mediaConstraints.end() && TypeIs<std::string>(it->second)) {
    return GetValue<std::string>(it->second);
  }
  return "";
}

void FlutterMediaStream::GetUserAudio(const EncodableMap& constraints,
                                      scoped_refptr<RTCMediaStream> stream,
                                      EncodableMap& params) {
  bool enable_audio = false;
  scoped_refptr<RTCMediaConstraints> audioConstraints;
  RTCAudioOptions audio_options;
  std::string sourceId;
  std::string deviceId;
  auto it = constraints.find(EncodableValue("audio"));
  if (it != constraints.end()) {
    EncodableValue audio = it->second;
    if (TypeIs<bool>(audio)) {
      audioConstraints = RTCMediaConstraints::Create();
      addDefaultAudioConstraints(audioConstraints);
      enable_audio = GetValue<bool>(audio);
      sourceId = "";
      deviceId = "";
      // audio: true — keep software processing on (W3C/WebRTC default).
      audio_options.echo_cancellation = true;
      audio_options.noise_suppression = true;
      audio_options.auto_gain_control = true;
      audio_options.highpass_filter = false;
    }
    if (TypeIs<EncodableMap>(audio)) {
      EncodableMap localMap = GetValue<EncodableMap>(audio);
      sourceId = getSourceIdConstraint(localMap);
      deviceId = getDeviceIdConstraint(localMap);
      audioConstraints = base_->ParseMediaConstraints(localMap);
      enable_audio = true;
      // Map W3C/goog-prefixed constraint keys to RTCAudioOptions so the
      // software AEC/NS/AGC can actually be toggled from the Dart side.
      audio_options.echo_cancellation = getAudioProcessingFlag(
          localMap, {"echoCancellation", "googEchoCancellation"}, true);
      audio_options.noise_suppression = getAudioProcessingFlag(
          localMap, {"noiseSuppression", "googNoiseSuppression"}, true);
      audio_options.auto_gain_control = getAudioProcessingFlag(
          localMap, {"autoGainControl", "googAutoGainControl"}, true);
      audio_options.highpass_filter = getAudioProcessingFlag(
          localMap, {"highpassFilter", "googHighpassFilter"}, false);
    }
  }

  // Selecting audio input device by sourceId and audio output device by
  // deviceId

  if (enable_audio) {
    char strRecordingName[256];
    char strRecordingGuid[256];
    int playout_devices = base_->audio_device_->PlayoutDevices();
    int recording_devices = base_->audio_device_->RecordingDevices();

    for (uint16_t i = 0; i < recording_devices; i++) {
      base_->audio_device_->RecordingDeviceName(i, strRecordingName,
                                                strRecordingGuid);
      if (sourceId != "" &&
          sourceId ==
              SanitizeDeviceIdFromAudioBuffers(strRecordingName,
                                               strRecordingGuid)) {
        // Fogata: kept by id, so it stays chosen when the list changes. The first
        // input of that id, as selectAudioInput takes: two of the same id (on Linux
        // two microphones with the same description) were both selected, one after
        // the other, ending on the other one.
        base_->SelectRecordingDevice(i, sourceId);
        break;
      }
    }

    if (sourceId == "") {
      // Fogata: the system's default, which on Windows need not be the first listed.
      const uint16_t place = DefaultAudioPlace(base_->audio_device_.get(),
                                               true, recording_devices);
      base_->audio_device_->RecordingDeviceName(place, strRecordingName,
                                                strRecordingGuid);
      sourceId = SanitizeDeviceIdFromAudioBuffers(strRecordingName,
                                                  strRecordingGuid);
      base_->SelectRecordingDevice(place, sourceId);
    }

    char strPlayoutName[256];
    char strPlayoutGuid[256];
    for (uint16_t i = 0; i < playout_devices; i++) {
      base_->audio_device_->PlayoutDeviceName(i, strPlayoutName,
                                              strPlayoutGuid);
      if (deviceId != "" &&
          deviceId ==
              SanitizeDeviceIdFromAudioBuffers(strPlayoutName,
                                               strPlayoutGuid)) {
        base_->audio_device_->SetPlayoutDevice(i);
      }
    }

    scoped_refptr<RTCAudioSource> source = base_->factory_->CreateAudioSource(
        "audio_input", RTCAudioSource::SourceType::kMicrophone, audio_options);
    std::string uuid = base_->GenerateUUID();
    scoped_refptr<RTCAudioTrack> track =
        base_->factory_->CreateAudioTrack(source, uuid.c_str());

    std::string track_id = track->id().std_string();

    EncodableMap track_info;
    track_info[EncodableValue("id")] = EncodableValue(track->id().std_string());
    track_info[EncodableValue("label")] =
        EncodableValue(track->id().std_string());
    track_info[EncodableValue("kind")] =
        EncodableValue(track->kind().std_string());
    track_info[EncodableValue("enabled")] = EncodableValue(track->enabled());

    EncodableMap settings;
    settings[EncodableValue("deviceId")] =
        EncodableValue(SanitizeUtf8ForFlutter(sourceId));
    settings[EncodableValue("kind")] = EncodableValue("audioinput");
    settings[EncodableValue("autoGainControl")] =
        EncodableValue(audio_options.auto_gain_control);
    settings[EncodableValue("echoCancellation")] =
        EncodableValue(audio_options.echo_cancellation);
    settings[EncodableValue("noiseSuppression")] =
        EncodableValue(audio_options.noise_suppression);
    settings[EncodableValue("channelCount")] = EncodableValue(1);
    settings[EncodableValue("latency")] = EncodableValue(0);
    track_info[EncodableValue("settings")] = EncodableValue(settings);

    EncodableList audioTracks;
    audioTracks.push_back(EncodableValue(track_info));
    params[EncodableValue("audioTracks")] = EncodableValue(audioTracks);
    stream->AddTrack(track);

    base_->local_tracks_[track->id().std_string()] = track;
  }
}

std::string getFacingMode(const EncodableMap& mediaConstraints) {
  return mediaConstraints.find(EncodableValue("facingMode")) !=
                 mediaConstraints.end()
             ? GetValue<std::string>(
                   mediaConstraints.find(EncodableValue("facingMode"))->second)
             : "";
}

EncodableValue getConstrainInt(const EncodableMap& constraints,
                               const std::string& key) {
  EncodableValue value;
  auto it = constraints.find(EncodableValue(key));
  if (it != constraints.end()) {
    if (TypeIs<int>(it->second)) {
      return it->second;
    }

    if (TypeIs<EncodableMap>(it->second)) {
      EncodableMap innerMap = GetValue<EncodableMap>(it->second);
      auto it2 = innerMap.find(EncodableValue("ideal"));
      if (it2 != innerMap.end() && TypeIs<int>(it2->second)) {
        return it2->second;
      }
    }
  }

  return EncodableValue();
}

bool FlutterMediaStream::GetUserVideo(const EncodableMap& constraints,
                                      scoped_refptr<RTCMediaStream> stream,
                                      EncodableMap& params,
                                      std::string* error) {
  EncodableMap video_constraints;
  EncodableMap video_mandatory;
  auto it = constraints.find(EncodableValue("video"));
  if (it != constraints.end() && TypeIs<EncodableMap>(it->second)) {
    video_constraints = GetValue<EncodableMap>(it->second);
    if (video_constraints.find(EncodableValue("mandatory")) !=
        video_constraints.end()) {
      video_mandatory = GetValue<EncodableMap>(
          video_constraints.find(EncodableValue("mandatory"))->second);
    }
  }

  std::string facing_mode = getFacingMode(video_constraints);
  // bool isFacing = facing_mode == "" || facing_mode != "environment";
  std::string sourceId = getSourceIdConstraint(video_constraints);

  EncodableValue widthValue = getConstrainInt(video_constraints, "width");

  if (widthValue == EncodableValue())
    widthValue = findEncodableValue(video_mandatory, "minWidth");

  if (widthValue == EncodableValue())
    widthValue = findEncodableValue(video_mandatory, "width");

  EncodableValue heightValue = getConstrainInt(video_constraints, "height");

  if (heightValue == EncodableValue())
    heightValue = findEncodableValue(video_mandatory, "minHeight");

  if (heightValue == EncodableValue())
    heightValue = findEncodableValue(video_mandatory, "height");

  EncodableValue fpsValue = getConstrainInt(video_constraints, "frameRate");

  if (fpsValue == EncodableValue())
    fpsValue = findEncodableValue(video_mandatory, "minFrameRate");

  if (fpsValue == EncodableValue())
    fpsValue = findEncodableValue(video_mandatory, "frameRate");

  scoped_refptr<RTCVideoCapturer> video_capturer;
  char strNameUTF8[256];
  char strGuidUTF8[256];
  int nb_video_devices = base_->video_device_->NumberOfDevices();

  int32_t width = toInt(widthValue, DEFAULT_WIDTH);
  int32_t height = toInt(heightValue, DEFAULT_HEIGHT);
  int32_t fps = toInt(fpsValue, DEFAULT_FPS);

#ifdef __linux__
  // Fogata: cameras through the Camera portal that were refused, or no portal
  // to reach them (and none opened directly). Tests with the made-up camera
  // never ask.
  if (!TestCamera::Wanted()) {
    const auto access = base_->video_device_->GetCameraAccess();
    if (access == RTCVideoDevice::kCameraAccessDenied) {
      *error =
          "NotAllowedError: the system refused the camera "
          "(fogataCameraDenied)";
      return false;
    }
    if (access == RTCVideoDevice::kCameraAccessUnavailable &&
        nb_video_devices == 0) {
      *error =
          "NotReadableError: no camera portal to reach the cameras "
          "(fogataCameraUnavailable)";
      return false;
    }
  }
#endif

#ifdef __linux__
  std::string asked_name, asked_guid;
#endif
  for (int i = 0; i < nb_video_devices; i++) {
    if (base_->video_device_->GetDeviceName(i, strNameUTF8, 256, strGuidUTF8,
                                            256) != 0) {
      continue;  // Gone since it was counted.
    }
    if (sourceId != "" &&
        sourceId ==
            SanitizeDeviceIdFromVideoBuffers(strNameUTF8, strGuidUTF8)) {
#ifdef __linux__
      // Fogata: the camera asked for, the one a failure is told of.
      asked_name = strNameUTF8;
      asked_guid = strGuidUTF8;
#endif
      video_capturer =
          base_->video_device_->Create(strNameUTF8, i, width, height, fps);
      break;
    }
  }

  scoped_refptr<RTCVideoSource> source;
#ifdef __linux__
  // Fogata: tests on a computer without a camera get a made-up one.
  if (nb_video_devices == 0 && TestCamera::Wanted()) {
    source = base_->factory_->CreateCustomVideoSource(
        "video_input", base_->ParseMediaConstraints(video_constraints));
    video_capturer = scoped_refptr<RTCVideoCapturer>(
        new RefCountedObject<TestCamera>(source, width, height, fps));
    sourceId = "fogata-test-camera";
  }
#endif

  if (nb_video_devices == 0 && !video_capturer.get())
    return true;

  if (!video_capturer.get()) {
    if (base_->video_device_->GetDeviceName(0, strNameUTF8, 128, strGuidUTF8,
                                            128) != 0) {
      return true;  // The only camera went away meanwhile.
    }
    sourceId = SanitizeDeviceIdFromVideoBuffers(strNameUTF8, strGuidUTF8);
    video_capturer =
        base_->video_device_->Create(strNameUTF8, 0, width, height, fps);
  }

  if (!video_capturer.get()) {
    // Fogata: listed but not opened (busy, gone, or offering no format).
    *error = "NotReadableError: the camera could not be opened";
#ifdef __linux__
    // Fogata: busy, if another application holds it (cameras reached directly).
    if (nb_video_devices > 0) {
      *error = CameraOpenError(
          !TestCamera::Wanted() &&
              base_->video_device_->GetCameraAccess() ==
                  RTCVideoDevice::kCameraAccessDirect,
          asked_name, asked_guid, strGuidUTF8, strNameUTF8, error->c_str());
    }
#endif
    return false;
  }

  // Fogata: a camera that does not start would show a black picture.
  if (!video_capturer->StartCapture()) {
    *error = "NotReadableError: the camera did not start";
#ifdef __linux__
    // Fogata: busy, if another application holds it (cameras reached directly).
    // The capturer lets go of its own descriptor first, so it is not taken for
    // the other application.
    if (nb_video_devices > 0) {
      video_capturer = scoped_refptr<RTCVideoCapturer>();
      *error = CameraOpenError(
          !TestCamera::Wanted() &&
              base_->video_device_->GetCameraAccess() ==
                  RTCVideoDevice::kCameraAccessDirect,
          asked_name, asked_guid, strGuidUTF8, strNameUTF8, error->c_str());
    }
#endif
    return false;
  }

#ifdef __linux__
  // Fogata: a camera reached through the Camera portal starts before PipeWire
  // links it, and one another application holds fails only then, which showed
  // a black picture: wait for its first picture or PipeWire's failure. A camera
  // that works answers with its first picture, a busy one within milliseconds;
  // a stream still without a picture when the wait ends is kept, as before.
  // Cameras reached directly told it as they were opened and started.
  if (!TestCamera::Wanted() &&
      base_->video_device_->GetCameraAccess() ==
          RTCVideoDevice::kCameraAccessGranted) {
    libwebrtc::string said;
    const auto start = video_capturer->WaitForStart(kPortalCameraStartMs, said);
    // Only a stream PipeWire said failed is refused; one with no picture yet is kept
    // as before this wait (its state goes to the log from WaitForStart).
    if (start == RTCVideoCapturer::kStartFailed) {
      video_capturer = scoped_refptr<RTCVideoCapturer>();
      *error = PortalCameraError(said.std_string());
      return false;
    }
  }
#endif

  const char* video_source_label = "video_input";
  if (!source.get()) {
    source = base_->factory_->CreateVideoSource(
        video_capturer, video_source_label,
        base_->ParseMediaConstraints(video_constraints));
  }

  std::string uuid = base_->GenerateUUID();
  scoped_refptr<RTCVideoTrack> track =
      base_->factory_->CreateVideoTrack(source, uuid.c_str());

  EncodableList videoTracks;
  EncodableMap info;
  info[EncodableValue("id")] = EncodableValue(track->id().std_string());
  info[EncodableValue("label")] = EncodableValue(track->id().std_string());
  info[EncodableValue("kind")] = EncodableValue(track->kind().std_string());
  info[EncodableValue("enabled")] = EncodableValue(track->enabled());

  EncodableMap settings;
  settings[EncodableValue("deviceId")] =
      EncodableValue(SanitizeUtf8ForFlutter(sourceId));
  settings[EncodableValue("kind")] = EncodableValue("videoinput");
  settings[EncodableValue("width")] = EncodableValue(width);
  settings[EncodableValue("height")] = EncodableValue(height);
  settings[EncodableValue("frameRate")] = EncodableValue(fps);
  info[EncodableValue("settings")] = EncodableValue(settings);

  videoTracks.push_back(EncodableValue(info));
  params[EncodableValue("videoTracks")] = EncodableValue(videoTracks);

  stream->AddTrack(track);

  base_->local_tracks_[track->id().std_string()] = track;
  base_->video_capturers_[track->id().std_string()] = video_capturer;
  return true;
}

void FlutterMediaStream::GetSources(std::unique_ptr<MethodResultProxy> result) {
  EncodableList sources;

  // Fogata: the system's default input and output listed first, as the application
  // takes the first listed for the default (fogata_default_device.h). A device is still
  // selected by its id, at its own place in libwebrtc's list.
  int nb_audio_devices = base_->audio_device_->RecordingDevices();
  char strNameUTF8[RTCAudioDevice::kAdmMaxDeviceNameSize + 1] = {0};
  char strGuidUTF8[RTCAudioDevice::kAdmMaxGuidSize + 1] = {0};

  for (uint16_t i : ListingOrder(
           nb_audio_devices, DefaultAudioPlace(base_->audio_device_.get(),
                                               true, nb_audio_devices))) {
    base_->audio_device_->RecordingDeviceName(i, strNameUTF8, strGuidUTF8);
    std::string device_id =
        SanitizeDeviceIdFromAudioBuffers(strNameUTF8, strGuidUTF8);
    EncodableMap audio;
    audio[EncodableValue("label")] = EncodableValue(SanitizeLabel(strNameUTF8));
    audio[EncodableValue("deviceId")] = EncodableValue(device_id);
    audio[EncodableValue("facing")] = "";
    audio[EncodableValue("kind")] = "audioinput";
    sources.push_back(EncodableValue(audio));
  }

  nb_audio_devices = base_->audio_device_->PlayoutDevices();
  for (uint16_t i : ListingOrder(
           nb_audio_devices, DefaultAudioPlace(base_->audio_device_.get(),
                                               false, nb_audio_devices))) {
    base_->audio_device_->PlayoutDeviceName(i, strNameUTF8, strGuidUTF8);
    std::string device_id =
        SanitizeDeviceIdFromAudioBuffers(strNameUTF8, strGuidUTF8);
    EncodableMap audio;
    audio[EncodableValue("label")] = EncodableValue(SanitizeLabel(strNameUTF8));
    audio[EncodableValue("deviceId")] = EncodableValue(device_id);
    audio[EncodableValue("facing")] = "";
    audio[EncodableValue("kind")] = "audiooutput";
    sources.push_back(EncodableValue(audio));
  }

  int nb_video_devices = base_->video_device_->NumberOfDevices();
  for (int i = 0; i < nb_video_devices; i++) {
    if (base_->video_device_->GetDeviceName(i, strNameUTF8, 128, strGuidUTF8,
                                            128) != 0) {
      continue;  // Fogata: gone since it was counted.
    }
    EncodableMap video;
    video[EncodableValue("label")] = EncodableValue(SanitizeLabel(strNameUTF8));
    video[EncodableValue("deviceId")] = EncodableValue(
        SanitizeDeviceIdFromVideoBuffers(strNameUTF8, strGuidUTF8));
    video[EncodableValue("facing")] = i == 1 ? "front" : "back";
    video[EncodableValue("kind")] = "videoinput";
    sources.push_back(EncodableValue(video));
  }
  EncodableMap params;
  params[EncodableValue("sources")] = EncodableValue(sources);
  result->Success(EncodableValue(params));
}

void FlutterMediaStream::SelectAudioOutput(
    const std::string& device_id,
    std::unique_ptr<MethodResultProxy> result) {
  char deviceName[256];
  char deviceGuid[256];
  int playout_devices = base_->audio_device_->PlayoutDevices();
  bool found = false;
  for (uint16_t i = 0; i < playout_devices; i++) {
    base_->audio_device_->PlayoutDeviceName(i, deviceName, deviceGuid);
    std::string cur_device_id =
        SanitizeDeviceIdFromAudioBuffers(deviceName, deviceGuid);
    if (device_id != "" && device_id == cur_device_id) {
      base_->audio_device_->SetPlayoutDevice(i);
#ifdef __linux__
      // Fogata: the application's other sounds go there too.
      OutputKeeper::SendTo(i == 0 ? std::string() : std::string(deviceName));
#endif
      found = true;
      break;
    }
  }
  if (!found) {
    result->Error("Bad Arguments",
                  "Not found device id: " + SanitizeUtf8ForFlutter(device_id));
    return;
  }
  result->Success();
}

void FlutterMediaStream::SelectAudioInput(
    const std::string& device_id,
    std::unique_ptr<MethodResultProxy> result) {
  char deviceName[256];
  char deviceGuid[256];
  int playout_devices = base_->audio_device_->RecordingDevices();
  bool found = false;
  for (uint16_t i = 0; i < playout_devices; i++) {
    base_->audio_device_->RecordingDeviceName(i, deviceName, deviceGuid);
    std::string cur_device_id =
        SanitizeDeviceIdFromAudioBuffers(deviceName, deviceGuid);
    if (device_id != "" && device_id == cur_device_id) {
      // Fogata: kept by id, so it stays chosen when the list changes.
      base_->SelectRecordingDevice(i, device_id);
      found = true;
      break;
    }
  }
  if (!found) {
    result->Error("Bad Arguments",
                  "Not found device id: " + SanitizeUtf8ForFlutter(device_id));
    return;
  }
  result->Success();
}

void FlutterMediaStream::MediaStreamGetTracks(
    const std::string& stream_id,
    std::unique_ptr<MethodResultProxy> result) {
  scoped_refptr<RTCMediaStream> stream = base_->MediaStreamForId(stream_id);

  if (stream) {
    EncodableMap params;
    EncodableList audioTracks;

    auto audio_tracks = stream->audio_tracks();
    for (auto track : audio_tracks.std_vector()) {
      base_->local_tracks_[track->id().std_string()] = track;
      EncodableMap info;
      info[EncodableValue("id")] = EncodableValue(track->id().std_string());
      info[EncodableValue("label")] = EncodableValue(track->id().std_string());
      info[EncodableValue("kind")] = EncodableValue(track->kind().std_string());
      info[EncodableValue("enabled")] = EncodableValue(track->enabled());
      info[EncodableValue("remote")] = EncodableValue(true);
      info[EncodableValue("readyState")] = "live";
      audioTracks.push_back(EncodableValue(info));
    }
    params[EncodableValue("audioTracks")] = EncodableValue(audioTracks);

    EncodableList videoTracks;
    auto video_tracks = stream->video_tracks();
    for (auto track : video_tracks.std_vector()) {
      base_->local_tracks_[track->id().std_string()] = track;
      EncodableMap info;
      info[EncodableValue("id")] = EncodableValue(track->id().std_string());
      info[EncodableValue("label")] = EncodableValue(track->id().std_string());
      info[EncodableValue("kind")] = EncodableValue(track->kind().std_string());
      info[EncodableValue("enabled")] = EncodableValue(track->enabled());
      info[EncodableValue("remote")] = EncodableValue(true);
      info[EncodableValue("readyState")] = "live";
      videoTracks.push_back(EncodableValue(info));
    }

    params[EncodableValue("videoTracks")] = EncodableValue(videoTracks);

    result->Success(EncodableValue(params));
  } else {
    result->Error("MediaStreamGetTracksFailed",
                  "MediaStreamGetTracks() media stream is null !");
  }
}

void FlutterMediaStream::MediaStreamDispose(
    const std::string& stream_id,
    std::unique_ptr<MethodResultProxy> result) {
  scoped_refptr<RTCMediaStream> stream = base_->MediaStreamForId(stream_id);

  if (!stream) {
    result->Error("MediaStreamDisposeFailed",
                  "stream [" + stream_id + "] not found!");
    return;
  }

  // Fogata: a stream made to show a received track holds that track, whose id can be one
  // of this device's own: that one (its camera, its shared screen) is not let go of with
  // the stream. Known by how the track was added (mediaStreamAddTrack with a connection
  // as its owner), not by its wrapper's pointer, which is a new one each time the
  // stream's tracks change (ReceivedInStreams, fogata_track_owner.h). Every other track
  // is let go of by id, as the package always did.
  const ReceivedInStreams& received = base_->received_in_streams_;

  vector<scoped_refptr<RTCAudioTrack>> audio_tracks = stream->audio_tracks();

  for (auto track : audio_tracks.std_vector()) {
    stream->RemoveTrack(track);
    if (received.Holds(stream_id, track->id().std_string()))
      continue;
    base_->local_tracks_.erase(track->id().std_string());
  }

  vector<scoped_refptr<RTCVideoTrack>> video_tracks = stream->video_tracks();
  for (auto track : video_tracks.std_vector()) {
    stream->RemoveTrack(track);
    if (received.Holds(stream_id, track->id().std_string()))
      continue;
    base_->local_tracks_.erase(track->id().std_string());
    auto stopper = base_->track_stoppers_.find(track->id().std_string());
    if (stopper != base_->track_stoppers_.end()) {
      stopper->second();
      base_->track_stoppers_.erase(stopper);
    }
    base_->track_rates_.erase(track->id().std_string());
    base_->track_refreshers_.erase(track->id().std_string());
    if (base_->video_capturers_.find(track->id().std_string()) !=
        base_->video_capturers_.end()) {
      auto video_capture = base_->video_capturers_[track->id().std_string()];
      if (video_capture->CaptureStarted()) {
        video_capture->StopCapture();
      }
      base_->video_capturers_.erase(track->id().std_string());
    }
  }

  base_->received_in_streams_.Forget(stream_id);
  base_->RemoveStreamForId(stream_id);
  result->Success();
}

void FlutterMediaStream::CreateLocalMediaStream(
    std::unique_ptr<MethodResultProxy> result) {
  std::string uuid = base_->GenerateUUID();
  scoped_refptr<RTCMediaStream> stream =
      base_->factory_->CreateStream(uuid.c_str());

  EncodableMap params;
  params[EncodableValue("streamId")] = EncodableValue(uuid);

  base_->local_streams_[uuid] = stream;
  result->Success(EncodableValue(params));
}

void FlutterMediaStream::MediaStreamTrackSetEnable(
    const std::string& track_id,
    std::unique_ptr<MethodResultProxy> result) {
  result->NotImplemented();
}

void FlutterMediaStream::MediaStreamTrackSwitchCamera(
    const std::string& track_id,
    std::unique_ptr<MethodResultProxy> result) {
  result->NotImplemented();
}

void FlutterMediaStream::MediaStreamTrackDispose(
    const std::string& track_id,
    std::unique_ptr<MethodResultProxy> result) {
  for (auto it : base_->local_streams_) {
    // Fogata: a stream showing a received track of the same id keeps it.
    if (base_->received_in_streams_.Holds(it.first, track_id))
      continue;
    auto stream = it.second;
    auto audio_tracks = stream->audio_tracks();
    for (auto track : audio_tracks.std_vector()) {
      if (track->id().std_string() == track_id) {
        stream->RemoveTrack(track);
      }
    }
    auto video_tracks = stream->video_tracks();
    for (auto track : video_tracks.std_vector()) {
      if (track->id().std_string() == track_id) {
        stream->RemoveTrack(track);

        if (base_->video_capturers_.find(track_id) !=
            base_->video_capturers_.end()) {
          auto video_capture = base_->video_capturers_[track_id];
          if (video_capture->CaptureStarted()) {
            video_capture->StopCapture();
          }
          base_->video_capturers_.erase(track_id);
        }
      }
    }
  }
  auto stopper = base_->track_stoppers_.find(track_id);
  if (stopper != base_->track_stoppers_.end()) {
    stopper->second();
    base_->track_stoppers_.erase(stopper);
  }
  base_->track_rates_.erase(track_id);
  base_->track_refreshers_.erase(track_id);
  base_->RemoveMediaTrackForId(track_id);
  result->Success();
}
}  // namespace flutter_webrtc_plugin
