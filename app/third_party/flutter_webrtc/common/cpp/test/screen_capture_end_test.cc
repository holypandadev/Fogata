// A desktop capture's end (FlutterScreenCapture::OnStop and OnError, the plugin's own code
// in common/cpp/src/flutter_screen_capture.cc): libwebrtc tells it on its signalling thread
// (RTCDesktopCapturerImpl's BlockingCall), while the platform thread starts captures with
// their sound (getDisplayMedia, outside the portal), captures the screen again at another
// rate, and runs what was posted to it. Only the platform thread may touch the captures'
// tracks and the sound; a capture's end stops only its own sound, and a failed one tells
// Dart its track ended. Built with ThreadSanitizer by run_tests.sh: any data race fails the
// run. libwebrtc's objects are the test's own, and the plugin's base (FlutterWebRTCBase) is
// reduced to what the screen capture uses.

#include <atomic>
#include <condition_variable>
#include <cstdio>
#include <deque>
#include <map>
#include <memory>
#include <mutex>
#include <string>
#include <thread>
#include <utility>
#include <vector>

#include "flutter_screen_capture.h"
#include "rtc_rtp_capabilities.h"
#include "task_runner.h"

using namespace flutter_webrtc_plugin;

namespace {

int failures = 0;

void Check(bool ok, const std::string& what) {
  if (!ok) {
    std::printf("FAIL: %s\n", what.c_str());
    failures++;
  }
}

// The platform thread's queue, as the Linux task runner keeps it: tasks run later, in the
// order queued, with the queue let go.
class Runner : public TaskRunner {
 public:
  void EnqueueTask(TaskClosure task) override {
    std::lock_guard<std::mutex> guard(lock_);
    tasks_.push_back(std::move(task));
  }
  void RunAll() {
    while (true) {
      TaskClosure task;
      {
        std::lock_guard<std::mutex> guard(lock_);
        if (tasks_.empty()) return;
        task = std::move(tasks_.front());
        tasks_.pop_front();
      }
      task();
    }
  }

 private:
  std::mutex lock_;
  std::deque<TaskClosure> tasks_;
};

// The tracks whose end Dart was told, in order.
std::mutex ended_lock;
std::vector<std::string> ended;

class Events : public EventChannelProxy {
 public:
  void Success(const EncodableValue& event, bool) override {
    const EncodableMap& map = std::get<EncodableMap>(event);
    if (std::get<std::string>(map.at(EncodableValue("event"))) != "mediaStreamTrackEnded")
      return;
    std::lock_guard<std::mutex> guard(ended_lock);
    ended.push_back(std::get<std::string>(map.at(EncodableValue("trackId"))));
  }
};

class Result : public MethodResultProxy {
 public:
  explicit Result(EncodableValue* answer) : answer_(answer) {}
  void Success() override { *answer_ = EncodableValue(true); }
  void Success(const EncodableValue& result) override { *answer_ = result; }
  void Error(const std::string& code, const std::string& message,
             const EncodableValue&) override {
    *answer_ = EncodableValue(code + ": " + message);
  }
  void Error(const std::string& code, const std::string& message) override {
    *answer_ = EncodableValue(code + ": " + message);
  }
  void NotImplemented() override { *answer_ = EncodableValue("not implemented"); }

 private:
  EncodableValue* answer_;
};

// ----- libwebrtc's objects -----

class Source : public MediaSource {
 public:
  libwebrtc::string id() const override { return "7"; }
  libwebrtc::string name() const override { return "Screen"; }
  libwebrtc::vector<unsigned char> thumbnail() const override {
    return libwebrtc::vector<unsigned char>();
  }
  DesktopType type() const override { return kScreen; }
  bool UpdateThumbnail() override { return true; }
};

class MediaList : public RTCDesktopMediaList {
 public:
  void RegisterMediaListObserver(MediaListObserver*) override {}
  void DeRegisterMediaListObserver() override {}
  DesktopType type() const override { return kScreen; }
  int32_t UpdateSourceList(bool, bool) override { return 1; }
  int GetSourceCount() const override { return 1; }
  scoped_refptr<MediaSource> GetSource(int) override { return source_; }
  bool GetThumbnail(scoped_refptr<MediaSource>, bool) override { return false; }

 private:
  scoped_refptr<MediaSource> source_ = new RefCountedObject<Source>();
};

class Capturer : public RTCDesktopCapturer {
 public:
  void RegisterDesktopCapturerObserver(DesktopCapturerObserver* observer) override {
    observer_ = observer;
  }
  void DeRegisterDesktopCapturerObserver() override { observer_ = nullptr; }
  CaptureState Start(uint32_t) override { return CS_RUNNING; }
  CaptureState Start(uint32_t, uint32_t, uint32_t, uint32_t, uint32_t) override {
    return CS_RUNNING;
  }
  void Stop() override {}
  bool IsRunning() override { return true; }
  scoped_refptr<MediaSource> source() override { return nullptr; }
  bool IsCaptureBorderHidden() override { return false; }
  DesktopCapturerObserver* observer() const { return observer_; }

 private:
  DesktopCapturerObserver* observer_ = nullptr;
};

// Every capturer made, in order, by the platform thread.
std::vector<scoped_refptr<RTCDesktopCapturer>> capturers;

class Device : public RTCDesktopDevice {
 public:
  scoped_refptr<RTCDesktopCapturer> CreateDesktopCapturer(scoped_refptr<MediaSource>,
                                                          bool) override {
    scoped_refptr<RTCDesktopCapturer> capturer = new RefCountedObject<Capturer>();
    capturers.push_back(capturer);
    return capturer;
  }
  scoped_refptr<RTCDesktopMediaList> GetDesktopMediaList(DesktopType) override {
    return new RefCountedObject<MediaList>();
  }
  scoped_refptr<RTCDesktopCapturer> CreateDesktopCapturerWithOptions(
      scoped_refptr<MediaSource> source, const RTCDesktopCapturerOptions&) override {
    return CreateDesktopCapturer(source, true);
  }
};

template <typename Base>
class Track : public Base {
 public:
  explicit Track(std::string id) : id_(std::move(id)) {}
  RTCMediaTrack::RTCTrackState state() const override { return RTCMediaTrack::kLive; }
  const libwebrtc::string kind() const override { return "track"; }
  const libwebrtc::string id() const override { return id_; }
  bool enabled() const override { return true; }
  bool set_enabled(bool) override { return true; }

 private:
  std::string id_;
};

class VideoTrack : public Track<RTCVideoTrack> {
 public:
  using Track::Track;
  void AddRenderer(RTCVideoRenderer<scoped_refptr<RTCVideoFrame>>*) override {}
  void RemoveRenderer(RTCVideoRenderer<scoped_refptr<RTCVideoFrame>>*) override {}
};

class AudioTrack : public Track<RTCAudioTrack> {
 public:
  using Track::Track;
  void SetVolume(double) override {}
  void AddSink(AudioTrackSink*) override {}
  void RemoveSink(AudioTrackSink*) override {}
};

class VideoSource : public RTCVideoSource {
 public:
  void OnCapturedFrame(scoped_refptr<RTCVideoFrame>) override {}
  SourceType GetSourceType() const override { return SourceType::kCustom; }
  void AddProcessor(RTCVideoProcessor*) override {}
  void RemoveProcessor(RTCVideoProcessor*) override {}
};

class AudioSource : public RTCAudioSource {
 public:
  void CaptureFrame(const void*, int, int, size_t, size_t) override {}
  SourceType GetSourceType() const override { return kCustom; }
};

class Stream : public RTCMediaStream {
 public:
  bool AddTrack(scoped_refptr<RTCAudioTrack>) override { return true; }
  bool AddTrack(scoped_refptr<RTCVideoTrack>) override { return true; }
  bool RemoveTrack(scoped_refptr<RTCAudioTrack>) override { return true; }
  bool RemoveTrack(scoped_refptr<RTCVideoTrack>) override { return true; }
  libwebrtc::vector<scoped_refptr<RTCAudioTrack>> audio_tracks() override {
    return libwebrtc::vector<scoped_refptr<RTCAudioTrack>>();
  }
  libwebrtc::vector<scoped_refptr<RTCVideoTrack>> video_tracks() override {
    return libwebrtc::vector<scoped_refptr<RTCVideoTrack>>();
  }
  libwebrtc::vector<scoped_refptr<RTCMediaTrack>> tracks() override {
    return libwebrtc::vector<scoped_refptr<RTCMediaTrack>>();
  }
  scoped_refptr<RTCAudioTrack> FindAudioTrack(const libwebrtc::string) override {
    return nullptr;
  }
  scoped_refptr<RTCVideoTrack> FindVideoTrack(const libwebrtc::string) override {
    return nullptr;
  }
  const libwebrtc::string label() override { return "stream"; }
  const libwebrtc::string id() override { return "stream"; }
};

class Factory : public RTCPeerConnectionFactory {
 public:
  explicit Factory(scoped_refptr<RTCDesktopDevice> device) : device_(device) {}
  bool Initialize() override { return true; }
  bool Terminate() override { return true; }
  scoped_refptr<RTCPeerConnection> Create(const RTCConfiguration&,
                                          scoped_refptr<RTCMediaConstraints>) override {
    return nullptr;
  }
  void Delete(scoped_refptr<RTCPeerConnection>) override {}
  scoped_refptr<RTCAudioDevice> GetAudioDevice() override { return nullptr; }
  scoped_refptr<RTCAudioProcessing> GetAudioProcessing() override { return nullptr; }
  scoped_refptr<RTCVideoDevice> GetVideoDevice() override { return nullptr; }
  scoped_refptr<RTCDesktopDevice> GetDesktopDevice() override { return device_; }
  scoped_refptr<RTCAudioSource> CreateAudioSource(const libwebrtc::string,
                                                  RTCAudioSource::SourceType,
                                                  RTCAudioOptions) override {
    return new RefCountedObject<AudioSource>();
  }
  scoped_refptr<RTCVideoSource> CreateVideoSource(
      scoped_refptr<RTCVideoCapturer>, const libwebrtc::string,
      scoped_refptr<RTCMediaConstraints>) override {
    return new RefCountedObject<VideoSource>();
  }
  scoped_refptr<RTCVideoSource> CreateCustomVideoSource(
      libwebrtc::string, scoped_refptr<RTCMediaConstraints>) override {
    return new RefCountedObject<VideoSource>();
  }
  scoped_refptr<RTCVideoSource> CreateDesktopSource(
      scoped_refptr<RTCDesktopCapturer>, const libwebrtc::string,
      scoped_refptr<RTCMediaConstraints>) override {
    return new RefCountedObject<VideoSource>();
  }
  scoped_refptr<RTCAudioTrack> CreateAudioTrack(scoped_refptr<RTCAudioSource>,
                                                const libwebrtc::string id) override {
    return new RefCountedObject<AudioTrack>(id.std_string());
  }
  scoped_refptr<RTCVideoTrack> CreateVideoTrack(scoped_refptr<RTCVideoSource>,
                                                const libwebrtc::string id) override {
    return new RefCountedObject<VideoTrack>(id.std_string());
  }
  scoped_refptr<RTCMediaStream> CreateStream(const libwebrtc::string) override {
    return new RefCountedObject<Stream>();
  }
  scoped_refptr<RTCRtpCapabilities> GetRtpSenderCapabilities(RTCMediaType) override {
    return nullptr;
  }
  scoped_refptr<RTCRtpCapabilities> GetRtpReceiverCapabilities(RTCMediaType) override {
    return nullptr;
  }

 private:
  scoped_refptr<RTCDesktopDevice> device_;
};

// ----- The sound of a shared screen -----

// Whether each loopback capture made runs, in the order made; only the platform thread
// starts and stops them.
std::vector<bool> sounds;

class Sound : public LoopbackCapturer {
 public:
  bool Start(scoped_refptr<RTCAudioSource>) override {
    index_ = int(sounds.size());
    sounds.push_back(true);
    return true;
  }
  void Stop() override {
    if (index_ >= 0) sounds[size_t(index_)] = false;
  }

 private:
  int index_ = -1;
};

class TestBase : public FlutterWebRTCBase {
 public:
  explicit TestBase(TaskRunner* runner) : FlutterWebRTCBase(nullptr, nullptr, runner) {
    desktop_device_ = new RefCountedObject<Device>();
    factory_ = new RefCountedObject<Factory>(desktop_device_);
    event_channel_ = std::make_unique<Events>();
  }
};

// getDisplayMedia for the screen of the test's list, at `fps`, with its sound; the video
// track's id, or "" when it failed.
std::string Share(FlutterScreenCapture& capture, double fps) {
  EncodableMap video;
  video[EncodableValue("deviceId")] = EncodableValue(EncodableMap{
      {EncodableValue("exact"), EncodableValue(std::string("7"))}});
  video[EncodableValue("mandatory")] = EncodableValue(EncodableMap{
      {EncodableValue("frameRate"), EncodableValue(fps)}});
  EncodableMap audio;
  audio[EncodableValue("echoCancellation")] = EncodableValue(true);
  audio[EncodableValue("autoGainControl")] = EncodableValue(true);
  EncodableMap constraints;
  constraints[EncodableValue("video")] = EncodableValue(video);
  constraints[EncodableValue("audio")] = EncodableValue(audio);
  EncodableValue answer;
  capture.GetDisplayMedia(constraints, std::make_unique<Result>(&answer));
  if (!std::holds_alternative<EncodableMap>(answer)) return "";
  const EncodableList tracks =
      findList(std::get<EncodableMap>(answer), "videoTracks");
  if (tracks.empty()) return "";
  return findString(std::get<EncodableMap>(tracks[0]), "id");
}

DesktopCapturerObserver* Observer(const scoped_refptr<RTCDesktopCapturer>& capturer) {
  return static_cast<Capturer*>(capturer.get())->observer();
}

std::vector<std::string> Ended() {
  std::lock_guard<std::mutex> guard(ended_lock);
  return ended;
}

// A screen captured again at another rate keeps its sound when the capture before it
// ends; a failed capture stops its own sound and tells Dart its track ended, once.
void Recapture() {
  Runner runner;
  TestBase base(&runner);
  FlutterScreenCapture capture(&base);
  const std::string first = Share(capture, 30);
  Check(!first.empty(), "a screen shared");
  Check(sounds.size() == 1 && sounds[0], "with its sound");
  const std::string second = Share(capture, 60);
  Check(!second.empty() && second != first, "shared again at another rate");
  Check(sounds.size() == 2 && !sounds[0] && sounds[1], "the sound goes with the new capture");
  scoped_refptr<RTCDesktopCapturer> old = capturers[0];
  scoped_refptr<RTCDesktopCapturer> now = capturers[1];
  // libwebrtc's signalling thread.
  std::thread([old] { Observer(old)->OnStop(old); }).join();
  Check(sounds[1], "nothing changes before the platform thread runs");
  runner.RunAll();
  Check(sounds[1], "the old capture's end leaves the new sound");
  Check(Ended().empty(), "a capture stopped is not told as ended");
  std::thread([now] { Observer(now)->OnError(now); }).join();
  runner.RunAll();
  Check(!sounds[1], "a failed capture stops its sound");
  Check(Ended() == std::vector<std::string>{second}, "a failed capture's track ended");
  std::thread([now] { Observer(now)->OnError(now); }).join();
  runner.RunAll();
  Check(Ended().size() == 1, "told once");
}

// Captures started one after another on the platform thread while libwebrtc's signalling
// thread ends earlier ones, stopped or failed, as they come.
void Races() {
  Runner runner;
  TestBase base(&runner);
  FlutterScreenCapture capture(&base);
  const size_t first = capturers.size();
  const size_t ended_before = Ended().size();
  std::mutex lock;
  std::condition_variable more;
  std::deque<std::pair<scoped_refptr<RTCDesktopCapturer>, bool>> to_end;
  bool done = false;
  std::thread signalling([&] {
    while (true) {
      std::unique_lock<std::mutex> guard(lock);
      more.wait(guard, [&] { return done || !to_end.empty(); });
      if (to_end.empty()) return;
      auto [capturer, failed] = to_end.front();
      to_end.pop_front();
      guard.unlock();
      if (failed) {
        Observer(capturer)->OnError(capturer);
      } else {
        Observer(capturer)->OnStop(capturer);
      }
    }
  });
  constexpr int kCaptures = 300;
  int failed = 0;
  std::map<std::string, bool> tracks;
  for (int i = 0; i < kCaptures; i++) {
    const std::string track = Share(capture, i % 2 == 0 ? 30 : 15);
    Check(!track.empty(), "shared, round " + std::to_string(i));
    const bool fails = i % 3 != 0;
    failed += fails ? 1 : 0;
    tracks[track] = fails;
    {
      std::lock_guard<std::mutex> guard(lock);
      to_end.emplace_back(capturers.back(), fails);
    }
    more.notify_one();
    runner.RunAll();
  }
  {
    std::lock_guard<std::mutex> guard(lock);
    done = true;
  }
  more.notify_one();
  signalling.join();
  runner.RunAll();
  Check(capturers.size() - first == size_t(kCaptures), "a capturer for each share");
  const std::vector<std::string> told = Ended();
  Check(int(told.size() - ended_before) == failed, "each failed capture told once");
  for (size_t i = ended_before; i < told.size(); i++) {
    Check(tracks.count(told[i]) == 1 && tracks[told[i]],
          "only failed captures told: " + told[i]);
  }
  bool any = false;
  for (bool running : sounds) any = any || running;
  Check(!any, "no sound left once every capture ended");
}

}  // namespace

// The plugin's base, reduced to what the screen capture uses.
namespace flutter_webrtc_plugin {

FlutterWebRTCBase::FlutterWebRTCBase(BinaryMessenger* messenger,
                                     TextureRegistrar* textures,
                                     TaskRunner* task_runner)
    : messenger_(messenger), task_runner_(task_runner), textures_(textures) {}

FlutterWebRTCBase::~FlutterWebRTCBase() {}

void FlutterWebRTCBase::EnsureWebRTCInitialized(bool, bool) {}

scoped_refptr<RTCMediaTrack> FlutterWebRTCBase::MediaTrackForId(const std::string&) {
  return nullptr;
}

std::string FlutterWebRTCBase::GenerateUUID() {
  static std::atomic<int> next{0};
  return "id-" + std::to_string(++next);
}

scoped_refptr<RTCMediaConstraints> FlutterWebRTCBase::ParseMediaConstraints(
    const EncodableMap&) {
  return nullptr;
}

EventChannelProxy* FlutterWebRTCBase::event_channel() {
  return event_channel_.get();
}

// The loopback capture of a shared screen (linux/loopback_capturer_factory.cc): the
// test's, which says whether it runs.
std::unique_ptr<LoopbackCapturer> CreateLoopbackCapturer(const std::string&, bool) {
  return std::make_unique<Sound>();
}

}  // namespace flutter_webrtc_plugin

int main() {
  Recapture();
  Races();
  if (failures == 0) std::printf("screen_capture_end_test: ok\n");
  return failures == 0 ? 0 : 1;
}
