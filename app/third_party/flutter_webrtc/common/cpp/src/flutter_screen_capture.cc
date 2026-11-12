#include "flutter_screen_capture.h"
#include "flutter_utf8_sanitize.h"
#include "task_runner.h"

#ifdef __linux__
#include "portal_screen_capturer.h"
#endif

#ifdef _WIN32
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>

#include <cstdint>
#include <cstdlib>
#endif

#include <stdexcept>

namespace flutter_webrtc_plugin {

// Fogata: whether a window source is one of this process's own windows, which are not
// offered: shared, the call's tiles would show the share inside itself, again and again.
// On Windows a window's source id is its HWND, in decimal; elsewhere nothing is left out
// here (macOS leaves them out in its own list).
static bool OwnWindow(const scoped_refptr<MediaSource>& source) {
#ifdef _WIN32
  if (source->type() != kWindow) return false;
  const std::string id = source->id().std_string();
  char* end = nullptr;
  const long long value = std::strtoll(id.c_str(), &end, 10);
  if (id.empty() || end == nullptr || *end != '\0') return false;
  DWORD pid = 0;
  GetWindowThreadProcessId(
      reinterpret_cast<HWND>(static_cast<intptr_t>(value)), &pid);
  return pid != 0 && pid == GetCurrentProcessId();
#else
  (void)source;
  return false;
#endif
}

FlutterScreenCapture::FlutterScreenCapture(FlutterWebRTCBase* base)
    : base_(base) {}

bool FlutterScreenCapture::BuildDesktopSourcesList(const EncodableList& types,
                                                   bool force_reload) {
  size_t size = types.size();
  sources_.clear();
#ifdef __linux__
  // Fogata: where GTK draws on Wayland the portal shares the screen; libwebrtc's lists
  // would be XWayland's, whose capture shows only black (desktop_session.h).
  if (PortalScreenCapturer::Available()) return true;
#endif
  for (size_t i = 0; i < size; i++) {
    std::string type_str = GetValue<std::string>(types[i]);
    DesktopType desktop_type = DesktopType::kScreen;
    if (type_str == "screen") {
      desktop_type = DesktopType::kScreen;
    } else if (type_str == "window") {
      desktop_type = DesktopType::kWindow;
    } else {
      return false;
    }
    scoped_refptr<RTCDesktopMediaList> source_list;
    auto it = medialist_.find(desktop_type);
    if (it != medialist_.end()) {
      source_list = (*it).second;
    } else {
      source_list = base_->desktop_device_->GetDesktopMediaList(desktop_type);
      source_list->RegisterMediaListObserver(this);
      medialist_[desktop_type] = source_list;
    }
#ifdef __linux__
    try {
      source_list->UpdateSourceList(force_reload, false);
    } catch (...) {
      continue;
    }
#else
    source_list->UpdateSourceList(force_reload, false);
#endif
    int count = source_list->GetSourceCount();
    for (int j = 0; j < count; j++) {
      sources_.push_back(source_list->GetSource(j));
    }
  }
  return true;
}

void FlutterScreenCapture::GetDesktopSources(
    const EncodableList& types,
    std::unique_ptr<MethodResultProxy> result) {
  if (!BuildDesktopSourcesList(types, true)) {
    result->Error("Bad Arguments", "Failed to get desktop sources");
    return;
  }

  EncodableList sources;
  for (auto source : sources_) {
    if (OwnWindow(source)) continue;
    EncodableMap info;
    info[EncodableValue("id")] = EncodableValue(source->id().std_string());
    info[EncodableValue("name")] =
        EncodableValue(SanitizeUtf8ForFlutter(source->name().std_string()));
    info[EncodableValue("type")] =
        EncodableValue(source->type() == kWindow ? "window" : "screen");
    // TODO "thumbnailSize"
    info[EncodableValue("thumbnailSize")] = EncodableMap{
        {EncodableValue("width"), EncodableValue(0)},
        {EncodableValue("height"), EncodableValue(0)},
    };
    sources.push_back(EncodableValue(info));
  }

  auto map = EncodableMap();
  map[EncodableValue("sources")] = sources;
  result->Success(EncodableValue(map));
}

void FlutterScreenCapture::UpdateDesktopSources(
    const EncodableList& types,
    std::unique_ptr<MethodResultProxy> result) {
  if (!BuildDesktopSourcesList(types, false)) {
    result->Error("Bad Arguments", "Failed to update desktop sources");
    return;
  }
  auto map = EncodableMap();
  map[EncodableValue("result")] = true;
  result->Success(EncodableValue(map));
}

void FlutterScreenCapture::OnMediaSourceAdded(
    scoped_refptr<MediaSource> source) {
  EncodableMap info;
  info[EncodableValue("event")] = "desktopSourceAdded";
  info[EncodableValue("id")] = EncodableValue(source->id().std_string());
  info[EncodableValue("name")] =
      EncodableValue(SanitizeUtf8ForFlutter(source->name().std_string()));
  info[EncodableValue("type")] =
      EncodableValue(source->type() == kWindow ? "window" : "screen");
  // TODO "thumbnailSize"
  info[EncodableValue("thumbnailSize")] = EncodableMap{
      {EncodableValue("width"), EncodableValue(0)},
      {EncodableValue("height"), EncodableValue(0)},
  };
  base_->event_channel()->Success(EncodableValue(info));
}

void FlutterScreenCapture::OnMediaSourceRemoved(
    scoped_refptr<MediaSource> source) {
  EncodableMap info;
  info[EncodableValue("event")] = "desktopSourceRemoved";
  info[EncodableValue("id")] = EncodableValue(source->id().std_string());
  base_->event_channel()->Success(EncodableValue(info));
}

void FlutterScreenCapture::OnMediaSourceNameChanged(
    scoped_refptr<MediaSource> source) {
  EncodableMap info;
  info[EncodableValue("event")] = "desktopSourceNameChanged";
  info[EncodableValue("id")] = EncodableValue(source->id().std_string());
  info[EncodableValue("name")] =
      EncodableValue(SanitizeUtf8ForFlutter(source->name().std_string()));
  base_->event_channel()->Success(EncodableValue(info));
}

void FlutterScreenCapture::OnMediaSourceThumbnailChanged(
    scoped_refptr<MediaSource> source) {
  EncodableMap info;
  info[EncodableValue("event")] = "desktopSourceThumbnailChanged";
  info[EncodableValue("id")] = EncodableValue(source->id().std_string());
  info[EncodableValue("thumbnail")] =
      EncodableValue(source->thumbnail().std_vector());
  base_->event_channel()->Success(EncodableValue(info));
}

void FlutterScreenCapture::OnStart(scoped_refptr<RTCDesktopCapturer> capturer) {
}

void FlutterScreenCapture::OnPaused(
    scoped_refptr<RTCDesktopCapturer> capturer) {}

// The capturer is held by the task, so another one made meanwhile cannot take its
// address and be taken for it.
void FlutterScreenCapture::OnStop(scoped_refptr<RTCDesktopCapturer> capturer) {
  OnPlatformThread([this, capturer]() {
    StopSoundOf(capturer.get());
    capture_tracks_.erase(capturer.get());
  });
}

void FlutterScreenCapture::OnError(scoped_refptr<RTCDesktopCapturer> capturer) {
  OnPlatformThread([this, capturer]() {
    // Fogata: no sound goes on without its picture, and Dart is told the capture
    // ended (as the portal's and macOS's are), so the share stops instead of
    // showing a frozen picture.
    StopSoundOf(capturer.get());
    auto it = capture_tracks_.find(capturer.get());
    if (it == capture_tracks_.end()) return;
    const std::string track_id = it->second;
    capture_tracks_.erase(it);
    EncodableMap event;
    event[EncodableValue("event")] = EncodableValue("mediaStreamTrackEnded");
    event[EncodableValue("trackId")] = EncodableValue(track_id);
    base_->event_channel()->Success(EncodableValue(event));
  });
}

void FlutterScreenCapture::OnPlatformThread(std::function<void()> task) {
  if (base_->task_runner_ != nullptr) {
    base_->task_runner_->EnqueueTask(std::move(task));
  } else {
    task();
  }
}

// Fogata: the sound goes with the capture it was started for. A screen captured again
// (another frame rate) starts the new capture, with its own sound, before the old one
// stops: the old one's end must leave the new sound alone.
void FlutterScreenCapture::StopSoundOf(RTCDesktopCapturer* capturer) {
  if (loopback_capturer_ && capturer == loopback_owner_) {
    loopback_capturer_->Stop();
    loopback_capturer_.reset();
    loopback_audio_source_ = nullptr;
    loopback_owner_ = nullptr;
  }
}

void FlutterScreenCapture::GetDesktopSourceThumbnail(
    std::string source_id,
    int width,
    int height,
    std::unique_ptr<MethodResultProxy> result) {
  (void)width;
  (void)height;
  scoped_refptr<MediaSource> source;
  for (auto src : sources_) {
    if (src->id().std_string() == source_id) {
      source = src;
    }
  }
  if (source.get() == nullptr) {
    result->Error("Bad Arguments", "Failed to get desktop source thumbnail");
    return;
  }
  source->UpdateThumbnail();
  result->Success(EncodableValue(source->thumbnail().std_vector()));
}

namespace {

// Fogata: the sound processing an "audio" map names (echoCancellation,
// autoGainControl, noiseSuppression), off for whatever it leaves out. The
// screen's sound never goes through the audio processing module, but WebRTC
// sets that one module, shared with the microphone, from the source of every
// channel of sound that starts sending: a screen's sound made with everything
// off turned the microphone's echo cancelling and gain control off for as
// long as it went out. The application names the microphone's own settings.
RTCAudioOptions ScreenSoundOptions(const EncodableMap& constraints) {
  const EncodableMap audio = findMap(constraints, "audio");
  RTCAudioOptions options;
  options.echo_cancellation = findBoolean(audio, "echoCancellation");
  options.auto_gain_control = findBoolean(audio, "autoGainControl");
  options.noise_suppression = findBoolean(audio, "noiseSuppression");
  options.highpass_filter = false;
  return options;
}

}  // namespace

void FlutterScreenCapture::GetDisplayMedia(
    const EncodableMap& constraints,
    std::unique_ptr<MethodResultProxy> result) {
  std::string source_id = "0";
  // DesktopType source_type = kScreen;
  double fps = 30.0;
  // Whether the OS cursor is composited into the captured frames, driven by the
  // getDisplayMedia "cursor" video constraint. Defaults to true so behaviour is
  // unchanged when the constraint is absent — that is libwebrtc's own default.
  bool show_cursor = true;

  const EncodableMap video = findMap(constraints, "video");
  if (video != EncodableMap()) {
    const EncodableMap deviceId = findMap(video, "deviceId");
    if (deviceId != EncodableMap()) {
      source_id = findString(deviceId, "exact");
      if (source_id.empty()) {
        result->Error("Bad Arguments", "Incorrect video->deviceId->exact");
        return;
      }
      if (source_id != "0") {
        // source_type = DesktopType::kWindow;
      }
    }
    const EncodableMap mandatory = findMap(video, "mandatory");
    if (mandatory != EncodableMap()) {
      double frameRate = findDouble(mandatory, "frameRate");
      if (frameRate != 0.0) {
        fps = frameRate;
      }
    }
    // Accept both the spec's string form ("always"/"never") and a plain bool.
    // Only an explicitly supplied constraint moves off the default, so callers
    // that pass no "cursor" key keep exactly the behaviour they have today.
    const std::string cursor = findString(video, "cursor");
    if (!cursor.empty()) {
      show_cursor = (cursor == "always");
    } else if (video.find(EncodableValue("cursor")) != video.end()) {
      show_cursor = findBoolean(video, "cursor");
    }
  }

#ifdef __linux__
  // Fogata: libwebrtc's own capture under Wayland opens a second, hidden
  // session, offers no windows and may deliver no frames at all.
  if (source_id == "0" && PortalScreenCapturer::Available()) {
    bool capture_audio = false;
    auto audio_it = constraints.find(EncodableValue("audio"));
    if (audio_it != constraints.end()) {
      capture_audio = TypeIs<bool>(audio_it->second)
                          ? GetValue<bool>(audio_it->second)
                          : TypeIs<EncodableMap>(audio_it->second);
    }
    GetDisplayMediaFromPortal(fps, show_cursor, capture_audio,
                              ScreenSoundOptions(constraints), std::move(result));
    return;
  }
#endif

  // Fogata: the source and its capturer first; nothing is captured, sound included,
  // for a source that cannot be.
  EncodableMap video_constraints;
  auto it = constraints.find(EncodableValue("video"));
  if (it != constraints.end() && TypeIs<EncodableMap>(it->second)) {
    video_constraints = GetValue<EncodableMap>(it->second);
  }

  scoped_refptr<MediaSource> source;
  for (auto src : sources_) {
    if (src->id().std_string() == source_id) {
      source = src;
    }
  }

#ifdef __linux__
  // If the caller didn't specify a source (source_id == "0"), fall back to
  // the first available screen. When a specific source_id was requested but
  // isn't in the (possibly stale) cached list, rebuild the list and retry
  // the match instead of silently capturing the wrong source.
  if (!source.get() && !sources_.empty() && source_id == "0") {
    source = sources_.front();
  }
  if (!source.get()) {
    EncodableList types;
    types.push_back(EncodableValue(std::string("screen")));
    BuildDesktopSourcesList(types, true);
    for (auto src : sources_) {
      if (src->id().std_string() == source_id) {
        source = src;
      }
    }
    if (!source.get() && !sources_.empty() && source_id == "0") {
      source = sources_.front();
    }
  }
#endif

  if (!source.get()) {
    result->Error("Bad Arguments", "source not found!");
    return;
  }

  scoped_refptr<RTCDesktopCapturer> desktop_capturer =
      base_->desktop_device_->CreateDesktopCapturer(source, show_cursor);

  if (!desktop_capturer.get()) {
    result->Error("Bad Arguments", "CreateDesktopCapturer failed!");
    return;
  }

  std::string uuid = base_->GenerateUUID();

  scoped_refptr<RTCMediaStream> stream =
      base_->factory_->CreateStream(uuid.c_str());

  EncodableMap params;
  params[EncodableValue("streamId")] = EncodableValue(uuid);

  // VIDEO

  desktop_capturer->RegisterDesktopCapturerObserver(this);

  const char* video_source_label = "screen_capture_input";

  scoped_refptr<RTCVideoSource> video_source =
      base_->factory_->CreateDesktopSource(
          desktop_capturer, video_source_label,
          base_->ParseMediaConstraints(video_constraints));

  // TODO: RTCVideoSource -> RTCVideoTrack

  scoped_refptr<RTCVideoTrack> track =
      base_->factory_->CreateVideoTrack(video_source, uuid.c_str());

  EncodableList videoTracks;
  EncodableMap info;
  info[EncodableValue("id")] = EncodableValue(track->id().std_string());
  info[EncodableValue("label")] = EncodableValue(track->id().std_string());
  info[EncodableValue("kind")] = EncodableValue(track->kind().std_string());
  info[EncodableValue("enabled")] = EncodableValue(track->enabled());
  videoTracks.push_back(EncodableValue(info));
  params[EncodableValue("videoTracks")] = EncodableValue(videoTracks);

  stream->AddTrack(track);

  base_->local_tracks_[track->id().std_string()] = track;

  base_->local_streams_[uuid] = stream;

  // Fogata: a capturer that cannot start (WebRTC made no capturer for the source, or it
  // could not be selected, a window that has closed) gives no stream, rather than one that
  // never has a picture; and the sound of a share already going on, captured again at
  // another rate, is left as it is: the new sound starts only once its picture has.
  if (desktop_capturer->Start(uint32_t(fps)) == RTCDesktopCapturer::CS_FAILED) {
    desktop_capturer->DeRegisterDesktopCapturerObserver();
    base_->local_tracks_.erase(track->id().std_string());
    base_->local_streams_.erase(uuid);
    result->Error("Bad Arguments", "the screen or window could not be captured");
    return;
  }
  capture_tracks_[desktop_capturer.get()] = track->id().std_string();

  // AUDIO

  bool capture_audio = false;
  {
    auto audio_it = constraints.find(EncodableValue("audio"));
    if (audio_it != constraints.end()) {
      if (TypeIs<bool>(audio_it->second)) {
        capture_audio = GetValue<bool>(audio_it->second);
      } else if (TypeIs<EncodableMap>(audio_it->second)) {
        capture_audio = true;
      }
    }
  }

  if (capture_audio) {
    // Stop any previous loopback session before starting a new one.
    if (loopback_capturer_) {
      loopback_capturer_->Stop();
      loopback_capturer_.reset();
      loopback_owner_ = nullptr;
    }

    RTCAudioOptions loopback_opts = ScreenSoundOptions(constraints);
    const std::string loopback_source_label =
      "screen_loopback_input_" + base_->GenerateUUID();
    loopback_audio_source_ = base_->factory_->CreateAudioSource(
      loopback_source_label.c_str(), RTCAudioSource::SourceType::kCustom,
        loopback_opts);

    std::string audio_uuid = base_->GenerateUUID();
    scoped_refptr<RTCAudioTrack> audio_track =
        base_->factory_->CreateAudioTrack(loopback_audio_source_,
                                          audio_uuid.c_str());

    loopback_capturer_ =
        CreateLoopbackCapturer(source_id, source->type() == kWindow);

    if (loopback_capturer_ && loopback_capturer_->Start(loopback_audio_source_)) {
      loopback_owner_ = desktop_capturer.get();
      EncodableMap audio_info;
      audio_info[EncodableValue("id")] =
          EncodableValue(audio_track->id().std_string());
      audio_info[EncodableValue("label")] =
          EncodableValue(audio_track->id().std_string());
      audio_info[EncodableValue("kind")] =
          EncodableValue(audio_track->kind().std_string());
      audio_info[EncodableValue("enabled")] =
          EncodableValue(audio_track->enabled());

      EncodableList audioTracks;
      audioTracks.push_back(EncodableValue(audio_info));
      params[EncodableValue("audioTracks")] = EncodableValue(audioTracks);

      stream->AddTrack(audio_track);
      base_->local_tracks_[audio_track->id().std_string()] = audio_track;
    } else {
      // Loopback init failed or not supported — continue without audio.
      loopback_capturer_.reset();
      loopback_audio_source_ = nullptr;
      params[EncodableValue("audioTracks")] = EncodableValue(EncodableList());
    }
  } else {
    params[EncodableValue("audioTracks")] = EncodableValue(EncodableList());
  }

  result->Success(EncodableValue(params));
}

#ifdef __linux__
void FlutterScreenCapture::GetDisplayMediaFromPortal(
    double fps,
    bool show_cursor,
    bool capture_audio,
    RTCAudioOptions sound_options,
    std::unique_ptr<MethodResultProxy> result) {
  auto capturer = std::make_shared<PortalScreenCapturer>();
  std::shared_ptr<MethodResultProxy> pending(std::move(result));
  auto track_id = std::make_shared<std::string>();
  scoped_refptr<RTCVideoSource> source = base_->factory_->CreateCustomVideoSource(
      "screen_capture_input", base_->ParseMediaConstraints(EncodableMap()));
  capturer->Start(
      show_cursor, int(fps),
      [source](int width, int height, const uint8_t* y, int stride_y,
               const uint8_t* u, int stride_u, const uint8_t* v, int stride_v) {
        source->OnCapturedFrame(RTCVideoFrame::Create(
            width, height, y, stride_y, u, stride_u, v, stride_v));
      },
      [this, capturer, source, pending, track_id, capture_audio, sound_options](
          bool started, const std::string& error) {
        if (!started) {
          pending->Error("getDisplayMedia", error);
          return;
        }
        std::string uuid = base_->GenerateUUID();
        scoped_refptr<RTCMediaStream> stream =
            base_->factory_->CreateStream(uuid.c_str());
        scoped_refptr<RTCVideoTrack> track =
            base_->factory_->CreateVideoTrack(source, uuid.c_str());
        stream->AddTrack(track);
        *track_id = track->id().std_string();
        base_->local_tracks_[*track_id] = track;
        base_->local_streams_[uuid] = stream;
        base_->track_stoppers_[*track_id] = [capturer]() { capturer->Stop(); };
        base_->track_rates_[*track_id] = [capturer](int fps) {
          capturer->SetFps(fps);
        };
        base_->track_refreshers_[*track_id] = [capturer]() {
          return capturer->Refresh();
        };

        EncodableMap info;
        info[EncodableValue("id")] = EncodableValue(*track_id);
        info[EncodableValue("label")] = EncodableValue(*track_id);
        info[EncodableValue("kind")] = EncodableValue(track->kind().std_string());
        info[EncodableValue("enabled")] = EncodableValue(track->enabled());
        EncodableList video_tracks;
        video_tracks.push_back(EncodableValue(info));
        EncodableMap params;
        params[EncodableValue("streamId")] = EncodableValue(uuid);
        params[EncodableValue("videoTracks")] = EncodableValue(video_tracks);
        // The sound of the other applications, while this screen is shared. Not
        // for a window: on Linux the sound cannot be narrowed to the window's own
        // application, and every other one's would go to the whole room.
        EncodableList audio_tracks;
        if (capture_audio && capturer->SharesScreen()) {
          if (loopback_capturer_) {
            loopback_capturer_->Stop();
            loopback_capturer_.reset();
            loopback_owner_ = nullptr;
          }
          scoped_refptr<RTCAudioSource> sound = base_->factory_->CreateAudioSource(
              ("screen_loopback_input_" + base_->GenerateUUID()).c_str(),
              RTCAudioSource::SourceType::kCustom, sound_options);
          std::shared_ptr<LoopbackCapturer> loopback(
              CreateLoopbackCapturer("0", false).release());
          if (loopback && loopback->Start(sound)) {
            scoped_refptr<RTCAudioTrack> audio = base_->factory_->CreateAudioTrack(
                sound, base_->GenerateUUID().c_str());
            stream->AddTrack(audio);
            const std::string audio_id = audio->id().std_string();
            base_->local_tracks_[audio_id] = audio;
            base_->track_stoppers_[audio_id] = [loopback]() { loopback->Stop(); };
            auto video_stop = base_->track_stoppers_[*track_id];
            base_->track_stoppers_[*track_id] = [video_stop, loopback]() {
              video_stop();
              loopback->Stop();
            };
            EncodableMap audio_info;
            audio_info[EncodableValue("id")] = EncodableValue(audio_id);
            audio_info[EncodableValue("label")] = EncodableValue(audio_id);
            audio_info[EncodableValue("kind")] =
                EncodableValue(audio->kind().std_string());
            audio_info[EncodableValue("enabled")] = EncodableValue(audio->enabled());
            audio_tracks.push_back(EncodableValue(audio_info));
          }
        }
        params[EncodableValue("audioTracks")] = EncodableValue(audio_tracks);
        pending->Success(EncodableValue(params));
      },
      [this, track_id]() {
        EncodableMap event;
        event[EncodableValue("event")] = EncodableValue("mediaStreamTrackEnded");
        event[EncodableValue("trackId")] = EncodableValue(*track_id);
        base_->event_channel()->Success(EncodableValue(event));
      });
}
#endif

}  // namespace flutter_webrtc_plugin
